//! The lamps in the Enhanced picture: their colour as the content gives it, and a far lamp's
//! glow round, not square.

use glam::{DVec3, Mat4, Vec2, Vec3};
use omsi_geometry::MeshData;
use omsi_render::testing::linear_light_colour;
use omsi_render::{
    AlphaMode, Camera, Corona, Lighting, MaterialExtra, PointLight, RenderOptions, Renderer,
};
use omsi_texture::Image;

fn luma(c: [f32; 3]) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

#[test]
fn a_light_colour_is_decoded_at_its_own_luminance() {
    // white and grey stay as they are
    for c in [[1.0; 3], [0.5; 3]] {
        let l = linear_light_colour(c);
        assert!(
            l.iter().zip(c).all(|(a, b)| (a - b).abs() < 1e-5),
            "{c:?} -> {l:?}"
        );
    }
    // an amber lamp (255, 200, 120) keeps its strength and gains its saturation back
    let amber = [1.0, 200.0 / 255.0, 120.0 / 255.0];
    let l = linear_light_colour(amber);
    assert!((luma(l) - luma(amber)).abs() < 1e-5, "{l:?}");
    assert!(l[2] / l[0] < 0.25 && l[1] / l[0] < 0.65, "{l:?}");
}

/// A render of `scene` in the Enhanced picture at night, as 8-bit RGBA.
fn render(
    renderer: &mut Renderer,
    scene: &mut omsi_render::Scene,
    size: u32,
    camera: &Camera,
) -> Vec<u8> {
    let lighting = Lighting {
        enhanced: true,
        shadows: false,
        fog_density: 0.0,
        night: 1.0,
        sun_intensity: 0.0,
        sun_dir: Vec3::new(0.0, 0.6, -0.8),
        ..Default::default()
    };
    renderer
        .render_to_image(scene, size, size, camera, &lighting)
        .unwrap()
}

/// A 2 x 2 m board facing the camera 3 m ahead (the camera at the origin, looking north).
fn board() -> MeshData {
    MeshData {
        positions: vec![
            Vec3::new(-1.0, 3.0, -1.0),
            Vec3::new(1.0, 3.0, -1.0),
            Vec3::new(1.0, 3.0, 1.0),
            Vec3::new(-1.0, 3.0, 1.0),
        ],
        normals: vec![-Vec3::Y; 4],
        uvs: vec![Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
        indices: vec![0, 1, 2, 0, 2, 3],
        ranges: vec![(0, 6, 0)],
        one_sided: false,
    }
}

fn looking_north(fov_deg: f32) -> Camera {
    Camera {
        position: DVec3::ZERO,
        yaw: 0.0,
        pitch: 0.0,
        roll: 0.0,
        fov_deg,
        near: 0.1,
        far: 100.0,
    }
}

fn renderer() -> Renderer {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    pollster::block_on(Renderer::new_with(
        &instance,
        None,
        Some(wgpu::TextureFormat::Rgba8UnormSrgb),
        RenderOptions {
            msaa: 1,
            ssao: false,
            fxaa: false,
            render_scale: 1.0,
            ..Default::default()
        },
    ))
    .expect("test renderer")
}

#[test]
#[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
fn an_amber_lamp_lights_the_ground_amber() {
    const SIZE: u32 = 32;
    let mut renderer = renderer();
    let mut scene = renderer.new_scene();
    let grey = [0.5, 0.5, 0.5, 1.0];
    let paint = renderer.add_material(&mut scene, None, AlphaMode::Opaque, grey, false);
    let floor = MeshData {
        positions: vec![
            Vec3::new(-20.0, -20.0, 0.0),
            Vec3::new(20.0, -20.0, 0.0),
            Vec3::new(20.0, 20.0, 0.0),
            Vec3::new(-20.0, 20.0, 0.0),
        ],
        normals: vec![Vec3::Z; 4],
        uvs: vec![Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
        indices: vec![0, 1, 2, 0, 2, 3],
        ranges: vec![(0, 6, 0)],
        one_sided: false,
    };
    let mesh = renderer.add_mesh(&mut scene, &floor);
    renderer.add_surface_instance(&mut scene, mesh, DVec3::ZERO, Mat4::IDENTITY, vec![paint]);
    // a sodium-like street lamp, (255, 200, 120) in the content, 4 m over the ground
    let amber = [1.0, 200.0 / 255.0, 120.0 / 255.0];
    scene.lights.push(PointLight {
        position: DVec3::new(0.0, 0.0, 4.0),
        radius: 25.0,
        color: amber,
        intensity: 1.0,
        core: 3.0,
        ..Default::default()
    });
    let camera = Camera {
        position: DVec3::new(0.0, -0.5, 6.0),
        yaw: 0.0,
        pitch: -80.0,
        roll: 0.0,
        fov_deg: 30.0,
        near: 0.1,
        far: 100.0,
    };
    let img = render(&mut renderer, &mut scene, SIZE, &camera);
    let at = ((SIZE / 2 * SIZE + SIZE / 2) * 4) as usize;
    let (r, g, b) = (img[at] as f32, img[at + 1] as f32, img[at + 2] as f32);
    eprintln!("lit ground: {r} {g} {b}");
    assert!(r > 40.0, "the lamp does not light the ground: {r} {g} {b}");
    // on the screen at least as saturated as the content's numbers: blue under half of red
    // (taken as linear, the lamp came out a pale cream of about 0.7)
    assert!(b / r < 0.55, "the lamp's light is washed out: {r} {g} {b}");
}

#[test]
#[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
fn a_far_lamp_glows_round_not_square() {
    far_lamp_is_round(None);
}

#[test]
#[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
fn a_far_lamp_with_its_own_bitmap_glows_round_not_square() {
    // a light's own bitmap as its last mip levels show it on a small sprite: the glow
    // averaged over the whole square, flat to the corners
    far_lamp_is_round(Some(&Image {
        width: 16,
        height: 16,
        rgba: [255u8; 16 * 16 * 4].to_vec(),
        has_alpha: false,
    }));
}

/// A lamp 300 m ahead, seen through a long lens (its sprite a few pixels across), with the
/// standard glow or with `bitmap`, glows as a round point.
fn far_lamp_is_round(bitmap: Option<&Image>) {
    const SIZE: u32 = 96;
    let mut renderer = renderer();
    if let Some(img) = bitmap {
        renderer.set_corona_texture(1, img);
    }
    let mut scene = renderer.new_scene();
    scene.coronas.push(Corona {
        texture: u16::from(bitmap.is_some()),
        position: DVec3::new(0.0, 300.0, 0.0),
        size: 0.5,
        color: [1.0, 1.0, 1.0],
        brightness: 2.0,
        rotating: 2,
        z_offset: 0.0,
        ..Default::default()
    });
    let camera = Camera {
        position: DVec3::ZERO,
        yaw: 0.0,
        pitch: 0.0,
        roll: 0.0,
        fov_deg: 3.0,
        near: 0.1,
        far: 1000.0,
    };
    let img = render(&mut renderer, &mut scene, SIZE, &camera);
    let px = |x: u32, y: u32| -> f32 {
        let i = ((y * SIZE + x) * 4) as usize;
        (img[i] as f32 + img[i + 1] as f32 + img[i + 2] as f32) / 3.0
    };
    let c = SIZE / 2;
    let mut rows = String::new();
    for y in c - 8..c + 8 {
        for x in c - 8..c + 8 {
            rows.push_str(&format!("{:4}", px(x, y) as i32));
        }
        rows.push('\n');
    }
    eprintln!("{rows}");
    // the brightest pixel, and how far out along the axes and the diagonals the glow reaches
    let peak = (c - 8..c + 8)
        .flat_map(|y| (c - 8..c + 8).map(move |x| (x, y)))
        .map(|(x, y)| px(x, y))
        .fold(0.0, f32::max);
    // (what lies round it: the night sky)
    let bg = px(c - 12, c - 12);
    assert!(peak > bg + 40.0, "no lamp to be seen: {peak} over {bg}");
    let reach = |dx: i32, dy: i32| -> i32 {
        (1..8)
            .take_while(|k| {
                let x = (c as i32 + dx * k) as u32;
                let y = (c as i32 + dy * k) as u32;
                px(x, y) - bg > (peak - bg) * 0.1
            })
            .count() as i32
    };
    let axis = reach(1, 0)
        .max(reach(-1, 0))
        .max(reach(0, 1))
        .max(reach(0, -1));
    let diagonal = reach(1, 1)
        .max(reach(-1, -1))
        .max(reach(1, -1))
        .max(reach(-1, 1));
    // round: a diagonal step is 1.41 pixels, so a disc reaches fewer of them than of the
    // axis' steps; a lit square reaches as many
    assert!(
        diagonal < axis || axis == 0,
        "the far lamp's glow is a square: {axis} pixels along the axes, {diagonal} along the diagonals"
    );
}

#[test]
#[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
fn a_light_mapped_display_text_shows_at_night() {
    const SIZE: u32 = 32;
    let mut renderer = renderer();
    let mut scene = renderer.new_scene();
    // a red destination text on a display slot, lit all over by a white light map, beside
    // the same red made self-lit ([matl_allcolor] emissive 1): switched on at night, the
    // light map shows the texture's own colour as the self-lit one does
    let red = renderer.add_texture(&mut scene, &Image::solid([200, 40, 30, 255]), false);
    let white = renderer.add_texture(&mut scene, &Image::solid([255, 255, 255, 255]), false);
    let text = renderer.add_material_extra(
        &mut scene,
        Some(red),
        AlphaMode::Opaque,
        [1.0; 4],
        false,
        None,
        None,
        Some(white),
        None,
        [0.0; 3],
        MaterialExtra {
            display: true,
            screen: true,
            // (no ambient colour: the light map's own light alone, whatever the metering
            // makes of the night's sky light on an empty test scene)
            ambient: Some([0.0; 3]),
            ..Default::default()
        },
    );
    let self_lit = renderer.add_material_extra(
        &mut scene,
        Some(red),
        AlphaMode::Opaque,
        [1.0; 4],
        false,
        None,
        None,
        None,
        None,
        [1.0; 3],
        MaterialExtra::default(),
    );
    // three boards side by side: the text with its light map switched on, the same text
    // switched off, the self-lit red
    let mesh = renderer.add_mesh(&mut scene, &board());
    let at = |x: f32| Mat4::from_translation(Vec3::new(x, 0.0, 0.0));
    let on = renderer.add_surface_instance(&mut scene, mesh, DVec3::ZERO, at(-2.0), vec![text]);
    renderer.set_slot_light(&mut scene, on, &[1.0]);
    let off = renderer.add_surface_instance(&mut scene, mesh, DVec3::ZERO, at(0.0), vec![text]);
    renderer.set_slot_light(&mut scene, off, &[0.0]);
    renderer.add_surface_instance(&mut scene, mesh, DVec3::ZERO, at(2.0), vec![self_lit]);
    let img = render(&mut renderer, &mut scene, SIZE, &looking_north(90.0));
    let red_at = |x: f32| img[((SIZE / 2 * SIZE + (x * SIZE as f32) as u32) * 4) as usize] as f32;
    // (the boards' middles: 2 m apart at 3 m, with a 90 degree view)
    let (lit, dark, own) = (
        red_at(0.5 - 1.0 / 3.0),
        red_at(0.5),
        red_at(0.5 + 1.0 / 3.0),
    );
    eprintln!("red of the lit text {lit}, switched off {dark}, of the self-lit board {own}");
    assert!(own > 60.0, "the self-lit board is dark: {own}");
    assert!(
        lit > dark + 20.0 && lit > own * 0.7,
        "the light map hardly lights the text: {lit} on, {dark} off, {own} self-lit"
    );
}
