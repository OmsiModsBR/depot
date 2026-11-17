//! A `[matl_envmap]`'s photo gives its structure to the whole reflection in the Enhanced
//! picture, at every height the reflection points to - not only near the horizon. Kept to a
//! band round the horizon, a tilted pane seen from above (a bus's destination display glass)
//! showed the photo inside an ellipse only, the rest of it plain sky.

use glam::{DVec3, Mat4, Vec2, Vec3};
use omsi_geometry::MeshData;
use omsi_render::{
    AlphaMode, Camera, Lighting, MaterialExtra, RenderOptions, Renderer, ShadingPath,
};
use omsi_texture::Image;

const SIZE: u32 = 32;

/// An envmap photo laid out as OMSI's are (upside down: the street at the top, the sky at
/// the bottom), here one half black and the other white.
fn photo(street_white: bool) -> Image {
    let rgba = (0..64u32)
        .flat_map(|y| {
            let v = if (y < 32) == street_white { 255u8 } else { 0 };
            (0..64).flat_map(move |_| [v, v, v, 255])
        })
        .collect();
    Image {
        width: 64,
        height: 64,
        rgba,
        has_alpha: false,
    }
}

/// The picture of a black chrome board (a `[matl_envmap_mask]` that says it all mirrors)
/// tilted 45 degrees, so that it mirrors the view
/// straight down (all it shows is its reflection), mean green over its centre.
fn reflection(renderer: &mut Renderer, street_white: bool) -> f32 {
    let mut scene = renderer.new_scene();
    let grey = renderer.add_texture(&mut scene, &Image::solid([0, 0, 0, 255]), false);
    let env = renderer.add_texture(&mut scene, &photo(street_white), false);
    let mask = renderer.add_texture(&mut scene, &Image::solid([255, 255, 255, 255]), false);
    let paint = renderer.add_material_extra(
        &mut scene,
        Some(grey),
        AlphaMode::Opaque,
        [0.5, 0.5, 0.5, 1.0],
        false,
        None,
        None,
        None,
        Some((env, 1.0)),
        [0.0; 3],
        MaterialExtra {
            env_mask: Some(mask),
            ..Default::default()
        },
    );
    let n = Vec3::new(0.0, -1.0, -1.0).normalize();
    let board = MeshData {
        positions: vec![
            Vec3::new(-1.0, 4.0, -1.0),
            Vec3::new(1.0, 4.0, -1.0),
            Vec3::new(1.0, 2.0, 1.0),
            Vec3::new(-1.0, 2.0, 1.0),
        ],
        normals: vec![n; 4],
        uvs: vec![Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
        indices: vec![0, 1, 2, 0, 2, 3],
        ranges: vec![(0, 6, 0)],
        one_sided: false,
    };
    let mesh = renderer.add_mesh(&mut scene, &board);
    renderer.add_surface_instance(&mut scene, mesh, DVec3::ZERO, Mat4::IDENTITY, vec![paint]);
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
        .render_to_image(&mut scene, SIZE, SIZE, &camera, &lighting)
        .unwrap();
    let c = SIZE / 2;
    let greens: Vec<f32> = (c - 4..c + 4)
        .flat_map(|y| (c - 4..c + 4).map(move |x| (x, y)))
        .map(|(x, y)| img[((y * SIZE + x) * 4 + 1) as usize] as f32)
        .collect();
    greens.iter().sum::<f32>() / greens.len() as f32
}

#[test]
#[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
fn the_envmap_photo_shows_in_a_reflection_pointing_down() {
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
    let white = reflection(&mut renderer, true);
    let black = reflection(&mut renderer, false);
    eprintln!("reflection straight down: {white} with a white street in the photo, {black} with a black one");
    assert!(
        white > black * 1.5 + 5.0,
        "the photo does not show in the reflection: {white} white, {black} black"
    );
}
