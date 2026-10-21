//! Characterization of the draw order of blended slots (issue #41), on synthetic meshes.
//!
//! One model of two slots: a window (slot 0, alpha mode 2, half transparent, listed first)
//! and an opaque panel 4 mm behind it (slot 1). The picture's middle pixel says whether the
//! panel can be seen through the window. The four cases below record what the renderer
//! does today; the one that reproduces the report (the model drawn in model order, the
//! window writing depth) is marked, and is the one a fix flips.
//!
//! Needs a graphics adapter (a software one is enough):
//!
//! ```text
//! cargo test -p omsi-render --test draw_order -- --ignored --nocapture
//! ```

use glam::{DVec3, Mat4, Vec2, Vec3};
use omsi_geometry::MeshData;
use omsi_render::{AlphaMode, Camera, Lighting, RenderOptions, Renderer};

const SIZE: u32 = 64;

/// Middle pixel of the picture for a window over a panel.
/// `ordered`: the instance is drawn in model order (what a bus with a blended slot listed
/// before an opaque one gets); `window_writes_depth`: the window leaves `[matl_noZwrite]` out.
fn middle(ordered: bool, window_writes_depth: bool, gap: f32) -> [u8; 3] {
    let camera = Camera {
        position: DVec3::ZERO,
        yaw: 0.0,
        pitch: 0.0,
        roll: 0.0,
        fov_deg: 60.0,
        near: 0.1,
        far: 100.0,
    };
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let mut renderer = pollster::block_on(Renderer::new_with(
        &instance,
        None,
        Some(wgpu::TextureFormat::Rgba8UnormSrgb),
        RenderOptions {
            msaa: 1,
            ssao: false,
            shadow_size: 1024,
            fxaa: false,
            render_scale: 1.0,
            ..Default::default()
        },
    ))
    .expect("test renderer");
    let mut scene = renderer.new_scene();
    let quad = |y: f32, half: f32| -> [Vec3; 4] {
        [
            Vec3::new(-half, y, -half),
            Vec3::new(half, y, -half),
            Vec3::new(half, y, half),
            Vec3::new(-half, y, half),
        ]
    };
    // a far wall so that nothing but the model decides the middle pixel
    let wall = renderer.add_mesh(
        &mut scene,
        &MeshData {
            positions: quad(30.0, 60.0).to_vec(),
            normals: vec![-Vec3::Y; 4],
            uvs: vec![Vec2::ZERO; 4],
            ranges: vec![(0, 6, 0)],
            indices: vec![0, 1, 2, 0, 2, 3],
            one_sided: false,
        },
    );
    let black = renderer.add_material(
        &mut scene,
        None,
        AlphaMode::Opaque,
        [0.0, 0.0, 0.0, 1.0],
        true,
    );
    renderer.add_instance(&mut scene, wall, DVec3::ZERO, Mat4::IDENTITY, vec![black]);
    // slot 0: the window, 8 m away; slot 1: the opaque panel `gap` metres behind it
    let model = renderer.add_mesh(
        &mut scene,
        &MeshData {
            positions: [quad(8.0, 4.0), quad(8.0 + gap, 4.0)].concat(),
            normals: vec![-Vec3::Y; 8],
            uvs: vec![Vec2::ZERO; 8],
            ranges: vec![(0, 6, 0), (6, 6, 1)],
            indices: vec![0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7],
            one_sided: false,
        },
    );
    let half = renderer.add_texture(
        &mut scene,
        &omsi_texture::Image {
            width: 1,
            height: 1,
            rgba: vec![255, 255, 255, 128],
            has_alpha: true,
        },
        false,
    );
    let window = renderer.add_material(
        &mut scene,
        Some(half),
        AlphaMode::Blend,
        [0.0, 0.0, 1.0, 1.0],
        true,
    );
    let panel = renderer.add_material(
        &mut scene,
        None,
        AlphaMode::Opaque,
        [1.0, 0.0, 0.0, 1.0],
        true,
    );
    scene.materials[window].writes_depth = window_writes_depth;
    scene.materials[window].no_z_write = !window_writes_depth;
    let inst = renderer.add_instance(
        &mut scene,
        model,
        DVec3::ZERO,
        Mat4::IDENTITY,
        vec![window, panel],
    );
    renderer.set_ordered(&mut scene, inst, ordered);
    let lighting = Lighting {
        shadows: false,
        fog_density: 0.0,
        ..Default::default()
    };
    let rgba = renderer
        .render_to_image(&mut scene, SIZE, SIZE, &camera, &lighting)
        .unwrap();
    let at = ((SIZE / 2 * SIZE + SIZE / 2) * 4) as usize;
    [rgba[at], rgba[at + 1], rgba[at + 2]]
}

/// Red comes from the panel, blue from the window: the panel is seen through the window.
fn panel_seen(c: [u8; 3]) -> bool {
    c[0] > 60
}

const GAP: f32 = 0.004;

#[test]
#[ignore = "needs a graphics adapter; run with --ignored"]
fn opaque_parts_are_drawn_first_when_the_model_is_not_ordered() {
    for writes in [true, false] {
        let c = middle(false, writes, GAP);
        assert!(
            panel_seen(c),
            "panel behind the window (writes depth {writes}): {c:?}"
        );
        assert!(
            c[2] > 20,
            "the window tints it (writes depth {writes}): {c:?}"
        );
    }
}

#[test]
#[ignore = "needs a graphics adapter; run with --ignored"]
fn ordered_model_with_a_window_that_leaves_depth_alone_paints_the_panel_over_it_today() {
    // the panel is listed after the window and the window wrote no depth, so the panel is
    // drawn over the window's colour: seen, but without the window's tint
    let c = middle(true, false, GAP);
    assert!(panel_seen(c) && c[2] < 20, "{c:?}");
}

/// Issue #41 as it stands: in model order the window, listed first, writes its depth and
/// the panel 4 mm behind it fails the depth test. When #41 is fixed this assertion flips
/// (and the one with a wide gap stays as it is).
#[test]
#[ignore = "needs a graphics adapter; run with --ignored"]
fn issue_41_ordered_window_that_writes_depth_hides_a_panel_4_mm_behind_today() {
    let c = middle(true, true, GAP);
    assert!(!panel_seen(c), "the panel is hidden today: {c:?}");
}

#[test]
#[ignore = "needs a graphics adapter; run with --ignored"]
fn ordered_window_that_writes_depth_hides_a_panel_further_behind_too_today() {
    let c = middle(true, true, 1.0);
    assert!(
        !panel_seen(c),
        "a metre behind the window it is hidden as well: {c:?}"
    );
}
