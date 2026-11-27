//! The cost of the clouds' shade map on the software rasteriser (lavapipe), for the numbers
//! of the performance gate: the same Enhanced picture of a cloudy day with the sun still and
//! with the sun moving (the map drawn again every frame), averaged over a few frames.
//! Run with `-- --ignored --nocapture`; the times are CPU-rasteriser times, to be compared
//! with each other and not with a GPU's.

use glam::{DVec3, Mat4, Vec2, Vec3};
use omsi_geometry::MeshData;
use omsi_render::{AlphaMode, Camera, Lighting, RenderOptions, Renderer, ShadingPath};
use omsi_texture::Image;
use std::time::Instant;

#[test]
#[ignore = "benchmark: requires a graphics adapter; run with --ignored --nocapture"]
fn enhanced_frame_time_under_clouds() {
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
    let mut scene = renderer.new_scene();
    let sky = renderer.add_texture(&mut scene, &Image::solid([90, 140, 220, 255]), false);
    renderer.set_sky_textures(&mut scene, [sky, sky, sky]);
    let grey = renderer.add_material(&mut scene, None, AlphaMode::Opaque, [0.5; 4], false);
    let floor = MeshData {
        positions: vec![
            Vec3::new(-2000.0, -2000.0, 0.0),
            Vec3::new(2000.0, -2000.0, 0.0),
            Vec3::new(2000.0, 2000.0, 0.0),
            Vec3::new(-2000.0, 2000.0, 0.0),
        ],
        normals: vec![Vec3::Z; 4],
        uvs: vec![Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
        indices: vec![0, 1, 2, 0, 2, 3],
        ranges: vec![(0, 6, 0)],
        one_sided: false,
    };
    let mesh = renderer.add_mesh(&mut scene, &floor);
    renderer.add_surface_instance(&mut scene, mesh, DVec3::ZERO, Mat4::IDENTITY, vec![grey]);
    let camera = Camera {
        position: DVec3::new(0.0, 0.0, 2.0),
        yaw: 0.0,
        pitch: -15.0,
        roll: 0.0,
        fov_deg: 70.0,
        near: 0.3,
        far: 4000.0,
    };
    let lighting = |elevation_deg: f32| Lighting {
        path: ShadingPath::Hdr,
        shadows: true,
        sun_intensity: 1.0,
        sun_dir: Vec3::new(
            0.4 * elevation_deg.to_radians().cos(),
            0.2,
            elevation_deg.to_radians().sin(),
        )
        .normalize(),
        cloud_density: 0.55,
        ..Default::default()
    };
    let mut time = |label: &str, moving: bool| {
        // (one picture first: the shaders, the sky cube and the first map)
        let _ = renderer.render_to_image(&mut scene, 256, 144, &camera, &lighting(50.0));
        let start = Instant::now();
        let frames = 5;
        for i in 0..frames {
            let el = if moving { 50.0 + 0.5 * (i + 1) as f32 } else { 50.0 };
            let _ = renderer.render_to_image(&mut scene, 256, 144, &camera, &lighting(el));
        }
        let ms = start.elapsed().as_secs_f64() * 1000.0 / frames as f64;
        println!("BENCH {label}: {ms:.1} ms a picture");
    };
    time("sun still", false);
    time("sun moving (map redrawn every picture)", true);
}
