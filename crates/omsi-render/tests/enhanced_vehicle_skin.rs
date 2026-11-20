//! On the camera's own vehicle, an up-facing part of the outer skin under the roof's top -
//! the shoulder rounding into the roof, the cap over the destination display - is outside in
//! the Enhanced picture: it takes the sky's light and reflection. Taken as cab (dry, lit by
//! the cab's grey light), its envmap faded into a flat grey wherever it faced the sky. A
//! seat or a floor under the roof stays cab.

use glam::{DVec3, Mat4, Vec2, Vec3};
use omsi_geometry::MeshData;
use omsi_render::{
    AlphaMode, Camera, Lighting, MaterialExtra, RenderOptions, Renderer, ShadingPath,
};
use omsi_texture::Image;

const SIZE: u32 = 32;
/// The vehicle: its origin, turned 90 degrees so that its right side faces the camera
/// (-y), with a 2.5 x 12 x 3.2 m `[boundingbox]` and its roof at 3.4 m.
const ORIGIN: DVec3 = DVec3::new(0.0, 4.1, 0.0);
const ROOF: f32 = 3.4;

/// How much the Enhanced pass takes a vehicle's patch at `centre` facing `normal` as cab
/// (`OMSI_DEBUG_ENHANCED=13`: red), seen from `eye` straight along +y.
fn cab(renderer: &mut Renderer, centre: Vec3, normal: Vec3, eye: DVec3) -> f32 {
    let mut scene = renderer.new_scene();
    let grey = renderer.add_texture(&mut scene, &Image::solid([128, 128, 128, 255]), false);
    let paint = renderer.add_material_extra(
        &mut scene,
        Some(grey),
        AlphaMode::Opaque,
        [0.5, 0.5, 0.5, 1.0],
        false,
        None,
        None,
        None,
        None,
        [0.0; 3],
        MaterialExtra::default(),
    );
    let n = normal.normalize();
    let t1 = Vec3::X * 0.1;
    let t2 = n.cross(Vec3::X).normalize() * 0.1;
    let patch = MeshData {
        positions: vec![
            centre - t1 - t2,
            centre + t1 - t2,
            centre + t1 + t2,
            centre - t1 + t2,
        ],
        normals: vec![n; 4],
        uvs: vec![Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
        indices: vec![0, 1, 2, 0, 2, 3],
        ranges: vec![(0, 6, 0)],
        one_sided: false,
    };
    let mesh = renderer.add_mesh(&mut scene, &patch);
    let part = renderer.add_instance(&mut scene, mesh, DVec3::ZERO, Mat4::IDENTITY, vec![paint]);
    renderer.set_roof(&mut scene, part, Some(ROOF));
    let camera = Camera {
        position: eye,
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
        inside: Some((ORIGIN, 90.0, [2.5, 12.0, 3.2, 0.0, 0.0, 1.6])),
        ..Default::default()
    };
    let img = renderer
        .render_to_image(&mut scene, SIZE, SIZE, &camera, &lighting)
        .unwrap();
    let c = SIZE / 2;
    let reds: Vec<f32> = (c - 2..c + 2)
        .flat_map(|y| (c - 2..c + 2).map(move |x| (x, y)))
        .map(|(x, y)| img[((y * SIZE + x) * 4) as usize] as f32)
        .collect();
    reds.iter().sum::<f32>() / reds.len() as f32
}

#[test]
#[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
fn the_skin_facing_the_sky_is_outside_and_a_seat_is_cab() {
    std::env::set_var("OMSI_DEBUG_ENHANCED", "13");
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
    // the shoulder: 0.15 m inside the right side, 0.5 m under the roof, leaning out a little
    let shoulder = cab(
        &mut renderer,
        Vec3::new(0.0, 4.1 - 1.1, 2.9),
        Vec3::new(0.0, -0.3, 0.95),
        DVec3::new(0.0, 2.0, 2.9),
    );
    // a seat cushion in the middle of the saloon
    let seat = cab(
        &mut renderer,
        Vec3::new(0.0, 4.1, 1.0),
        Vec3::new(0.0, -0.5, 0.87),
        DVec3::new(0.0, 3.1, 1.0),
    );
    eprintln!("cab on the screen: shoulder {shoulder}, seat {seat}");
    assert!(
        shoulder < 30.0,
        "the shoulder under the roof is taken as cab: {shoulder}"
    );
    assert!(seat > 200.0, "the seat is not taken as cab: {seat}");
}
