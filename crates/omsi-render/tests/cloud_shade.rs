//! The clouds' shade map (sky/cloud_shade.rs): when it is drawn again, and what it does to the
//! Enhanced picture - the sun reaches the scenery through the gaps in the clouds and not
//! through the clouds, wherever the camera stands.

use glam::{DVec3, Mat4, Vec2, Vec3};
use omsi_geometry::MeshData;
use omsi_render::testing::internals::sky::cloud_shade::{ShadeInputs, ShadeState};
use omsi_render::testing::internals::sky::{CLOUD_LAYER_BASE, CLOUD_LAYER_TOP};
use omsi_render::{AlphaMode, Camera, Lighting, RenderOptions, Renderer, Scene, ShadingPath};
use omsi_texture::Image;

fn inputs() -> ShadeInputs {
    ShadeInputs {
        light: Vec3::new(0.3, 0.2, 0.9).normalize(),
        cover: 0.55,
        offset: Vec2::ZERO,
        base: CLOUD_LAYER_BASE,
        top: CLOUD_LAYER_TOP,
        camera: DVec3::new(12_000.0, 34_000.0, 5.0),
        camera_z: 5.0,
    }
}

#[test]
fn the_map_is_drawn_once_and_again_only_when_what_it_shows_changes() {
    let mut state = ShadeState::default();
    let now = inputs();
    assert!(state.due(&now), "nothing drawn yet");
    state.drawn(&now, 0.6);
    assert!(!state.due(&now));
    // a few metres of driving, a tenth of a degree of sun, a hair of cover: not worth a draw
    let mut near = now;
    near.camera.x += 30.0;
    near.light = Vec3::new(0.3, 0.2, 0.9).normalize() + Vec3::new(0.001, 0.0, 0.0);
    near.light = near.light.normalize();
    near.cover += 0.002;
    assert!(!state.due(&near));
    // the sun moved a degree, the cover moved, the camera went an eighth of the map
    let mut sun = now;
    sun.light = Vec3::new(0.3, 0.25, 0.9).normalize();
    assert!(state.due(&sun));
    let mut cover = now;
    cover.cover = 0.6;
    assert!(state.due(&cover));
    let mut away = now;
    away.camera.x += 2_000.0;
    assert!(state.due(&away));
    // the clouds drifting by a kilometre
    let mut drift = now;
    drift.offset = Vec2::new(0.5, 0.0);
    assert!(state.due(&drift));
    // no clouds, or the sun under the horizon: nothing to draw, nothing to read
    let mut clear = now;
    clear.cover = 0.0;
    assert!(!state.due(&clear));
    assert_eq!(state.uniform(&clear).0[3], 0.0);
    let mut night = now;
    night.light = Vec3::new(0.3, 0.2, -0.1).normalize();
    assert!(!state.due(&night));
    assert_eq!(state.uniform(&night).0[3], 0.0);
}

#[test]
fn the_map_slides_with_the_drift_between_two_draws() {
    let mut state = ShadeState::default();
    let now = inputs();
    state.drawn(&now, 0.6);
    let (shade, layer) = state.uniform(&now);
    assert_eq!(shade[3], 1.0);
    assert!((shade[2] - 0.6).abs() < 1e-6, "the border's mean");
    assert_eq!((layer[0], layer[1]), (CLOUD_LAYER_BASE, CLOUD_LAYER_TOP));
    assert_eq!((layer[2], layer[3]), (0.0, 0.0));
    let mut later = now;
    later.offset = Vec2::new(0.1, -0.2);
    let (_, layer) = state.uniform(&later);
    assert!((layer[2] - 250.0).abs() < 1e-3 && (layer[3] + 500.0).abs() < 1e-3);
}

#[test]
fn the_shadow_edge_widens_with_the_height_it_falls_from() {
    let low = ShadeInputs {
        base: 270.0,
        top: 700.0,
        ..inputs()
    };
    let high = inputs();
    assert!(low.blur_sigma() < high.blur_sigma());
    assert!(low.blur_sigma() >= 0.5);
}

fn luma(p: &[u8]) -> f32 {
    0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32
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

/// A grey field 20 km across at the ground, and the camera 1 km over it looking down.
fn field(renderer: &mut Renderer) -> Scene {
    let mut scene = renderer.new_scene();
    let sky = renderer.add_texture(&mut scene, &Image::solid([90, 140, 220, 255]), false);
    renderer.set_sky_textures(&mut scene, [sky, sky, sky]);
    let grey = renderer.add_material(
        &mut scene,
        None,
        AlphaMode::Opaque,
        [0.5, 0.5, 0.5, 1.0],
        false,
    );
    let floor = MeshData {
        positions: vec![
            Vec3::new(-10_000.0, -10_000.0, 0.0),
            Vec3::new(10_000.0, -10_000.0, 0.0),
            Vec3::new(10_000.0, 10_000.0, 0.0),
            Vec3::new(-10_000.0, 10_000.0, 0.0),
        ],
        normals: vec![Vec3::Z; 4],
        uvs: vec![Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
        indices: vec![0, 1, 2, 0, 2, 3],
        ranges: vec![(0, 6, 0)],
        one_sided: false,
    };
    let mesh = renderer.add_mesh(&mut scene, &floor);
    renderer.add_surface_instance(&mut scene, mesh, DVec3::ZERO, Mat4::IDENTITY, vec![grey]);
    scene
}

fn render(renderer: &mut Renderer, scene: &mut Scene, cover: f32, offset: [f32; 2]) -> Vec<f32> {
    const SIZE: u32 = 48;
    let camera = Camera {
        position: DVec3::new(0.0, 0.0, 1000.0),
        yaw: 0.0,
        pitch: -89.0,
        roll: 0.0,
        fov_deg: 100.0,
        near: 1.0,
        far: 20_000.0,
    };
    let lighting = Lighting {
        path: ShadingPath::Hdr,
        shadows: false,
        fog_density: 0.0,
        sun_intensity: 1.0,
        sun_dir: Vec3::new(0.3, 0.2, 0.9).normalize(),
        cloud_density: cover,
        cloud_offset: offset,
        ..Default::default()
    };
    let img = renderer
        .render_to_image(scene, SIZE, SIZE, &camera, &lighting)
        .unwrap();
    img.chunks(4).map(luma).collect()
}

fn spread(l: &[f32]) -> (f32, f32) {
    let lo = l.iter().cloned().fold(f32::MAX, f32::min);
    let hi = l.iter().cloned().fold(0.0, f32::max);
    (lo, hi)
}

#[test]
#[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
fn the_sun_reaches_the_field_through_the_gaps_in_the_clouds() {
    let mut renderer = renderer();
    let mut scene = field(&mut renderer);
    // a clear sky lights the field evenly
    let clear = render(&mut renderer, &mut scene, 0.0, [0.0, 0.0]);
    let (lo, hi) = spread(&clear);
    assert!(hi > 30.0, "the field is lit: {hi}");
    assert!(hi / lo < 1.15, "an even field: {lo}..{hi}");
    // clouds put lit patches and shaded ones side by side: under the camera's own cloud the
    // picture is no darker everywhere (the sun used to be taken from the whole scene)
    let cloudy = render(&mut renderer, &mut scene, 0.55, [0.0, 0.0]);
    let (lo, hi) = spread(&cloudy);
    assert!(hi / lo > 1.5, "lit and shaded patches: {lo}..{hi}");
    assert!(hi > 0.8 * spread(&clear).1, "the gaps keep the sun: {hi}");
}

#[test]
#[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
fn the_shade_moves_with_the_clouds_and_stays_put_without_them() {
    let mut renderer = renderer();
    let mut scene = field(&mut renderer);
    let a = render(&mut renderer, &mut scene, 0.55, [0.0, 0.0]);
    let again = render(&mut renderer, &mut scene, 0.55, [0.0, 0.0]);
    assert_eq!(a, again, "the same sky, the same picture");
    // a drift of 100 m (0.04 of 2500 m): the pattern shifts a little, it does not jump
    let drifted = render(&mut renderer, &mut scene, 0.55, [0.04, 0.0]);
    let diff: f32 = a
        .iter()
        .zip(&drifted)
        .map(|(x, y)| (x - y).abs())
        .sum::<f32>()
        / a.len() as f32;
    let mean: f32 = a.iter().sum::<f32>() / a.len() as f32;
    assert!(diff > 0.0 && diff < 0.5 * mean, "moved by {diff} of {mean}");
}
