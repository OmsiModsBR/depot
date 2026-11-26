//! Enhanced: a pane dims the sun's light instead of letting it through in full, by its alpha
//! and its colour, and does so only for what lies behind it as seen from the sun (a roof over
//! a window is lit as any roof).
//!
//! The scene, from above with the sun 45 degrees up in the east: an opaque panel and two grey
//! panes of alpha 0.4 and 0.8 hang 2.5 m over the ground, so each throws its shadow 2.5 m to the
//! west; the ground under those shadows is read.

use glam::{DVec3, Mat4, Vec2, Vec3};
use omsi_geometry::MeshData;
use omsi_render::{AlphaMode, Camera, Lighting, RenderOptions, Renderer, Scene, ShadingPath};

const SIZE: u32 = 128;
/// The height of the panels over the ground (m); with the sun at 45 degrees the shadow lies
/// this far to the west.
const HEIGHT: f32 = 2.5;

fn quad(min: Vec3, max: Vec3) -> MeshData {
    MeshData {
        positions: vec![
            Vec3::new(min.x, min.y, min.z),
            Vec3::new(max.x, min.y, min.z),
            Vec3::new(max.x, max.y, min.z),
            Vec3::new(min.x, max.y, min.z),
        ],
        normals: vec![Vec3::Z; 4],
        uvs: vec![Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
        indices: vec![0, 1, 2, 0, 2, 3],
        ranges: vec![(0, 6, 0)],
        one_sided: false,
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
            shadow_size: 1024,
            render_scale: 1.0,
            ..Default::default()
        },
    ))
    .expect("test renderer")
}

fn camera() -> Camera {
    Camera {
        position: DVec3::new(-2.5, 0.0, 13.0),
        yaw: 0.0,
        pitch: -90.0,
        roll: 0.0,
        fov_deg: 70.0,
        near: 0.1,
        far: 100.0,
    }
}

/// The mean of the green channel over a few pixels round the world point.
fn brightness(img: &[u8], at: Vec3) -> f32 {
    let clip = camera().view_proj(1.0, DVec3::ZERO) * at.extend(1.0);
    let x = ((clip.x / clip.w * 0.5 + 0.5) * SIZE as f32) as i32;
    let y = ((0.5 - clip.y / clip.w * 0.5) * SIZE as f32) as i32;
    let mut sum = 0.0;
    for dy in -2..=2 {
        for dx in -2..=2 {
            let (px, py) = ((x + dx) as u32, (y + dy) as u32);
            sum += img[((py * SIZE + px) * 4 + 1) as usize] as f32;
        }
    }
    sum / 25.0
}

fn picture(renderer: &mut Renderer, scene: &mut Scene) -> Vec<u8> {
    let lighting = Lighting {
        path: ShadingPath::Hdr,
        shadows: true,
        fog_density: 0.0,
        sun_dir: Vec3::new(1.0, 0.0, 1.0).normalize(),
        ..Default::default()
    };
    renderer
        .render_to_image(scene, SIZE, SIZE, &camera(), &lighting)
        .unwrap()
}

fn ground(renderer: &mut Renderer, scene: &mut Scene) {
    let grey = renderer.add_material(scene, None, AlphaMode::Opaque, [0.5; 4], false);
    let mesh = renderer.add_mesh(
        scene,
        &quad(Vec3::new(-60.0, -60.0, 0.0), Vec3::new(60.0, 60.0, 0.0)),
    );
    renderer.add_instance(scene, mesh, DVec3::ZERO, Mat4::IDENTITY, vec![grey]);
}

fn hang(renderer: &mut Renderer, scene: &mut Scene, material: usize, y: (f32, f32), height: f32) {
    let mesh = renderer.add_mesh(
        scene,
        &quad(Vec3::new(-0.75, y.0, height), Vec3::new(0.75, y.1, height)),
    );
    renderer.add_instance(scene, mesh, DVec3::ZERO, Mat4::IDENTITY, vec![material]);
}

/// The ground under the shadow of the opaque panel, of a pane of alpha 0.4, of a pane of alpha
/// 0.8, and in the open.
fn ground_under(renderer: &mut Renderer) -> [f32; 4] {
    let mut scene = renderer.new_scene();
    ground(renderer, &mut scene);
    let panel = renderer.add_material(&mut scene, None, AlphaMode::Opaque, [0.3; 4], false);
    let thin = renderer.add_material(
        &mut scene,
        None,
        AlphaMode::Blend,
        [0.3, 0.3, 0.3, 0.4],
        false,
    );
    let dense = renderer.add_material(
        &mut scene,
        None,
        AlphaMode::Blend,
        [0.3, 0.3, 0.3, 0.8],
        false,
    );
    hang(renderer, &mut scene, panel, (-7.0, -3.5), HEIGHT);
    hang(renderer, &mut scene, thin, (-1.75, 1.75), HEIGHT);
    hang(renderer, &mut scene, dense, (3.5, 7.0), HEIGHT);
    let img = picture(renderer, &mut scene);
    let shadow_x = -2.5;
    [
        brightness(&img, Vec3::new(shadow_x, -5.25, 0.0)),
        brightness(&img, Vec3::new(shadow_x, 0.0, 0.0)),
        brightness(&img, Vec3::new(shadow_x, 5.25, 0.0)),
        brightness(&img, Vec3::new(shadow_x, 8.5, 0.0)),
    ]
}

#[test]
#[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
fn a_pane_dims_the_sun_between_the_panels_shadow_and_the_lit_ground() {
    let mut renderer = renderer();
    let [panel, thin, dense, open] = ground_under(&mut renderer);
    eprintln!("ground under: panel {panel}, pane 0.4 {thin}, pane 0.8 {dense}, open {open}");
    let span = open - panel;
    assert!(span > 30.0, "no sun shadow to compare against: {span}");
    assert!(
        thin < open - 0.1 * span,
        "a pane lets the sun through in full: {thin} against {open} in the open"
    );
    assert!(
        thin > panel + 0.1 * span,
        "a pane shades as much as a panel: {thin} against {panel} under the panel"
    );
    assert!(
        dense < thin - 0.05 * span,
        "a denser pane lets as much through as a thinner one: {dense} against {thin}"
    );
    assert!(dense > panel + 0.05 * span);
}

/// A panel above a pane, the pane under it as seen from the sun, is not dimmed by it.
#[test]
#[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
fn a_surface_over_a_pane_is_lit_in_full() {
    let mut renderer = renderer();
    let mut scene = renderer.new_scene();
    ground(&mut renderer, &mut scene);
    let pane = renderer.add_material(
        &mut scene,
        None,
        AlphaMode::Blend,
        [0.3, 0.3, 0.3, 0.8],
        false,
    );
    let roof = renderer.add_material(&mut scene, None, AlphaMode::Opaque, [0.6; 4], false);
    hang(&mut renderer, &mut scene, pane, (-1.75, 1.75), HEIGHT);
    // one roof straight over the pane, one over open ground
    hang(&mut renderer, &mut scene, roof, (-1.75, 1.75), 4.5);
    hang(&mut renderer, &mut scene, roof, (2.5, 4.5), 4.5);
    let img = picture(&mut renderer, &mut scene);
    let over_pane = brightness(&img, Vec3::new(0.0, 0.0, 4.5));
    let over_ground = brightness(&img, Vec3::new(0.0, 3.5, 4.5));
    eprintln!("roof over the pane {over_pane}, over the ground {over_ground}");
    assert!(
        (over_pane - over_ground).abs() < 4.0,
        "the pane under a roof dims it: {over_pane} against {over_ground}"
    );
}
