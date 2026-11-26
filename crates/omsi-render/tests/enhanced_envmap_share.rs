//! The `[matl_envmap]` factor of a material without a mask or a transmap says how much it
//! mirrors in the Enhanced picture, as it does in the vanilla picture: a factor of 1 mirrors
//! far more than one of 0.1, and one of 0.02 still shows. (Every such body took a fixed 4 %
//! coat whatever its factor, and the factor only made the weak ones rough, which blurred
//! them into nothing.)

use glam::{DVec3, Mat4, Vec2, Vec3};
use omsi_geometry::MeshData;
use omsi_render::{
    AlphaMode, Camera, Lighting, MaterialExtra, RenderOptions, Renderer, ShadingPath,
};
use omsi_texture::Image;

const WIDTH: u32 = 128;
const HEIGHT: u32 = 32;

/// The envmap factors of the four black boards, left to right (0: no envmap).
const FACTORS: [f32; 4] = [0.0, 0.02, 0.1, 1.0];

/// Mean green over the centre of each of four black boards side by side in one picture
/// (one exposure for all), each with `[matl_envmap] … factor` and neither mask nor transmap.
fn reflections(renderer: &mut Renderer) -> [f32; 4] {
    let mut scene = renderer.new_scene();
    let sky = renderer.add_texture(&mut scene, &Image::solid([90, 140, 220, 255]), false);
    renderer.set_sky_textures(&mut scene, [sky, sky, sky]);
    let black = renderer.add_texture(&mut scene, &Image::solid([0, 0, 0, 255]), false);
    let env = renderer.add_texture(&mut scene, &Image::solid([200, 200, 200, 255]), false);
    let board = MeshData {
        positions: vec![
            Vec3::new(-0.4, 0.0, -0.4),
            Vec3::new(0.4, 0.0, -0.4),
            Vec3::new(0.4, 0.0, 0.4),
            Vec3::new(-0.4, 0.0, 0.4),
        ],
        normals: vec![-Vec3::Y; 4],
        uvs: vec![Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
        indices: vec![0, 1, 2, 0, 2, 3],
        ranges: vec![(0, 6, 0)],
        one_sided: false,
    };
    let mesh = renderer.add_mesh(&mut scene, &board);
    for (i, factor) in FACTORS.into_iter().enumerate() {
        let paint = renderer.add_material_extra(
            &mut scene,
            Some(black),
            AlphaMode::Opaque,
            [0.5, 0.5, 0.5, 1.0],
            false,
            None,
            None,
            None,
            (factor > 0.0).then_some((env, factor)),
            [0.0; 3],
            MaterialExtra::default(),
        );
        let x = -1.5 + i as f64;
        renderer.add_surface_instance(
            &mut scene,
            mesh,
            DVec3::new(x, 3.0, 0.0),
            Mat4::IDENTITY,
            vec![paint],
        );
    }
    let camera = Camera {
        position: DVec3::ZERO,
        yaw: 0.0,
        pitch: 0.0,
        roll: 0.0,
        fov_deg: 20.0,
        near: 0.1,
        far: 100.0,
    };
    let lighting = Lighting {
        path: ShadingPath::Hdr,
        shadows: false,
        fog_density: 0.0,
        ..Default::default()
    };
    let img = renderer
        .render_to_image(&mut scene, WIDTH, HEIGHT, &camera, &lighting)
        .unwrap();
    // each board's centre on the screen, by the same projection
    let half_w = (20f64.to_radians() / 2.0).tan() * WIDTH as f64 / HEIGHT as f64;
    let y = HEIGHT / 2;
    std::array::from_fn(|i| {
        let x = -1.5 + i as f64;
        let px = ((0.5 + x / 3.0 / half_w / 2.0) * WIDTH as f64) as u32;
        let greens: Vec<f32> = (y - 2..y + 2)
            .flat_map(|py| (px - 2..px + 2).map(move |qx| (qx, py)))
            .map(|(qx, py)| img[((py * WIDTH + qx) * 4 + 1) as usize] as f32)
            .collect();
        greens.iter().sum::<f32>() / greens.len() as f32
    })
}

#[test]
#[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
fn the_envmap_factor_sets_how_much_the_paint_mirrors() {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let mut renderer = pollster::block_on(Renderer::new_with(
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
    .expect("test renderer");
    let [none, faint, weak, full] = reflections(&mut renderer);
    eprintln!("reflection by factor: none {none}, 0.02 {faint}, 0.1 {weak}, 1 {full}");
    // (on the screen, against the board without an envmap: proportional to the factor,
    // as the vanilla picture lays it, give or take the tone mapping)
    let [faint, weak, full] = [faint - none, weak - none, full - none];
    assert!(
        faint > 1.0,
        "a factor of 0.02 does not show: {faint} over no envmap"
    );
    assert!(
        weak > faint * 3.0,
        "a factor of 0.1 mirrors not much more than one of 0.02: {weak} against {faint}"
    );
    assert!(
        full > weak * 3.0,
        "a factor of 1 mirrors not much more than one of 0.1: {full} against {weak}"
    );
}
