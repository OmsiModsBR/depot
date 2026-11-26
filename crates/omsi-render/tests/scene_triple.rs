//! Tests moved out of `src/scene/triple.rs`.

use omsi_render::testing::internals::*;
fn camera() -> Camera {
    Camera {
        position: DVec3::new(1e6, 2e6, 3.0),
        yaw: 17.0,
        pitch: 12.0,
        roll: 7.0,
        fov_deg: 60.0,
        near: 0.1,
        far: 5000.0,
    }
}
#[test]
fn temporary_zoom_keeps_calibration_and_supports_close_views() {
    let rig = TripleScreen::default();
    assert_eq!(rig.zoomed(5760, 1080, 1.0), rig);
    let zoom = rig.zoomed(5760, 1080, 0.1);
    assert_eq!(zoom.fov_deg, 8.0);
    assert_eq!(rig.fov_deg, 0.0);
    assert!((zoom.views(&camera(), 5760, 1080)[1].camera.fov_deg - 8.0).abs() < 1e-4);
}

#[test]
fn fov_override_changes_all_views_without_breaking_joins() {
    let cam = camera();
    let base = TripleScreen::default();
    let wide = TripleScreen {
        fov_deg: 80.0,
        ..base
    };
    let views = wide.views(&cam, 5760, 1080);
    assert!((views[1].camera.fov_deg - 80.0).abs() < 1e-4);
    let (_, before, _) = base.cursor_ray(&cam, (1920.0, 540.0), (5760, 1080));
    let (_, after, _) = wide.cursor_ray(&cam, (1920.0, 540.0), (5760, 1080));
    assert!(before.distance(after) > 0.1);
    let ray = |i: usize, x| {
        let v = &views[i];
        let vp = v.projection * Mat4::look_to_rh(Vec3::ZERO, v.camera.forward(), v.camera.up());
        vp.inverse()
            .project_point3(Vec3::new(x, 0.0, 0.0))
            .normalize()
    };
    assert!(ray(0, 1.0).distance(ray(1, -1.0)) < 1e-5);
    assert!(ray(1, 1.0).distance(ray(2, -1.0)) < 1e-5);
}
#[test]
fn joins_are_continuous_with_pitch_and_roll() {
    for angle in [0.0, 30.0, 45.0, 70.0, 90.0] {
        let rig = TripleScreen {
            left_angle_deg: angle,
            right_angle_deg: angle,
            ..Default::default()
        };
        let cam = camera();
        let views = rig.views(&cam, 5760, 1080);
        let ray = |i: usize, x: f32, y: f32| {
            let v = &views[i];
            let vp = v.projection * Mat4::look_to_rh(Vec3::ZERO, v.camera.forward(), v.camera.up());
            vp.inverse()
                .project_point3(Vec3::new(x, y, 0.0))
                .normalize()
        };
        for y in [-1.0, 0.0, 1.0] {
            assert!(
                ray(0, 1.0, y).distance(ray(1, -1.0, y)) < 1e-5,
                "left {angle}"
            );
            assert!(
                ray(1, 1.0, y).distance(ray(2, -1.0, y)) < 1e-5,
                "right {angle}"
            );
        }
    }
}
#[test]
fn covers_every_pixel_and_reverses_depth() {
    let cam = camera();
    let rig = TripleScreen::default();
    let views = rig.views(&cam, 5761, 1080);
    // equal panels side by side, the last one cropped by the window's edge
    assert!(views.iter().all(|v| v.viewport[2] == 1921));
    assert_eq!(views[0].viewport[0], 0);
    for i in 0..2 {
        assert_eq!(
            views[i + 1].viewport[0],
            views[i].viewport[0] + views[i].viewport[2]
        );
    }
    let end = views[2].viewport[0] + views[2].viewport[2];
    assert!(end >= 5761 && end < 5761 + 3);
    for v in &views {
        assert!(
            (v.projection
                .project_point3(Vec3::new(0.0, 0.0, -cam.near))
                .z
                - 1.0)
                .abs()
                < 1e-5
        );
        assert!(
            v.projection
                .project_point3(Vec3::new(0.0, 0.0, -cam.far))
                .z
                .abs()
                < 1e-5
        );
        assert_eq!(v.camera.position, cam.position);
    }
}
#[test]
fn view_extent_covers_the_side_panels() {
    let cam = Camera {
        roll: 0.0,
        pitch: 0.0,
        ..camera()
    };
    let rig = TripleScreen::default();
    let (tx, ty) = rig.view_extent(&cam, 5760, 1080);
    // the centre panel alone: 300 mm either side at 650 mm
    assert!(tx > 300.0 / 650.0 * 2.0, "{tx}");
    assert!(ty >= rig.views(&cam, 5760, 1080)[1].projection.y_axis.y.recip() - 1e-4);
    // a flat row: the outer edges at 900 mm either side
    let flat = TripleScreen {
        left_angle_deg: 0.0,
        right_angle_deg: 0.0,
        ..rig
    };
    assert!((flat.view_extent(&cam, 5760, 1080).0 - 900.0 / 650.0).abs() < 1e-3);
}

#[test]
fn bezel_hides_a_gap_and_eye_height_shifts_horizon() {
    let cam = camera();
    let rig = TripleScreen {
        bezel_mm: 20.0,
        eye_height_mm: 50.0,
        ..Default::default()
    };
    let (_, left, _) = rig.cursor_ray(&cam, (1919.99, 540.0), (5760, 1080));
    let (_, centre, _) = rig.cursor_ray(&cam, (1920.0, 540.0), (5760, 1080));
    assert!(left.distance(centre) > 0.01);
    let v = rig.views(&cam, 5760, 1080);
    assert!(v[1].projection.z_axis.y < 0.0);
}

#[test]
#[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
fn gpu_renders_each_panel_and_composites_hud_with_msaa() {
    use omsi_geometry::MeshData;
    use omsi_render::{AlphaMode, Lighting, MaterialExtra, RenderOptions, Renderer};
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let cam = Camera {
        position: DVec3::ZERO,
        yaw: 0.0,
        pitch: 0.0,
        roll: 0.0,
        ..camera()
    };
    let rig = TripleScreen::default();
    for (msaa, render_scale) in [(1, 1.0), (4, 1.0), (4, 0.65)] {
        let mut renderer = pollster::block_on(Renderer::new_with(
            &instance,
            None,
            Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            RenderOptions {
                msaa,
                ssao: true,
                shadow_size: 1024,
                fxaa: true,
                render_scale,
                ..Default::default()
            },
        ))
        .unwrap();
        let mut scene = renderer.new_scene();
        let colours = [
            [1.0, 0.0, 0.0, 1.0],
            [0.0, 1.0, 0.0, 1.0],
            [0.0, 0.0, 1.0, 1.0],
        ];
        for i in 0..3 {
            // Put a coloured marker exactly on the pixel ray of that panel.
            let (_, ray, _) = rig.cursor_ray(&cam, (i as f32 * 64.0 + 32.0, 32.0), (192, 64));
            let right = Vec3::new(ray.y, -ray.x, 0.0).normalize();
            let up = right.cross(ray);
            let centre = ray * 10.0;
            let quad = MeshData {
                positions: vec![
                    centre - right - up,
                    centre + right - up,
                    centre + right + up,
                    centre - right + up,
                ],
                normals: vec![-ray; 4],
                uvs: vec![glam::Vec2::ZERO; 4],
                ranges: vec![(0, 6, 0)],
                indices: vec![0, 1, 2, 0, 2, 3],
                one_sided: false,
            };
            let mesh = renderer.add_mesh(&mut scene, &quad);
            let material =
                renderer.add_material(&mut scene, None, AlphaMode::Opaque, colours[i], true);
            renderer.add_instance(
                &mut scene,
                mesh,
                DVec3::ZERO,
                Mat4::IDENTITY,
                vec![material],
            );
            let film = renderer.add_material_extra(
                &mut scene,
                None,
                AlphaMode::Blend,
                [1.0, 1.0, 1.0, 0.08],
                false,
                None,
                None,
                None,
                None,
                [0.0; 3],
                MaterialExtra {
                    rain_film: true,
                    ..Default::default()
                },
            );
            renderer.add_instance(
                &mut scene,
                mesh,
                DVec3::ZERO,
                Mat4::from_scale(Vec3::splat(0.5)),
                vec![film],
            );
        }
        let white = renderer.add_texture(
            &mut scene,
            &omsi_texture::Image {
                width: 1,
                height: 1,
                rgba: vec![255; 4],
                has_alpha: false,
            },
            false,
        );
        scene.overlays.push((white, [0.0, 0.0, 192.0, 4.0]));
        for enhanced in [false, true] {
            let lighting = Lighting {
                path: if enhanced {
                    omsi_render::testing::internals::ShadingPath::Hdr
                } else {
                    omsi_render::testing::internals::ShadingPath::Linear
                },
                shadows: true,
                fog_density: 0.0,
                rain: 0.5,
                wetness: 0.5,
                ..Default::default()
            };
            let rgba = renderer
                .render_triple_to_image(&mut scene, 192, 64, &cam, &lighting, &rig)
                .unwrap();
            for i in 0..3 {
                let at = ((32 * 192 + i * 64 + 32) * 4) as usize;
                let pixel = &rgba[at..at + 3];
                assert!(
                    pixel[i] > 80 && pixel[i] > pixel[(i + 1) % 3] * 2,
                    "panel {i}, msaa {msaa}, enhanced {enhanced}: {pixel:?}"
                );
                let at = ((2 * 192 + i * 64 + 32) * 4) as usize;
                assert!(
                    rgba[at..at + 3].iter().all(|c| *c > 240),
                    "HUD must cover every panel at full resolution"
                );
            }
            assert_eq!(scene.overlays.len(), 1);
            assert!(renderer.env_heading.get().is_none());
            assert!(renderer.culling.cull_drawn.borrow().is_empty());
            assert_ne!(
                renderer.culling.triple_culling[0].drawn,
                renderer.culling.triple_culling[1].drawn
            );
            assert_ne!(
                renderer.culling.triple_culling[2].drawn,
                renderer.culling.triple_culling[1].drawn
            );
            // Rain must sample this panel's current scene, including when
            // the three panels reuse the same scratch texture dimensions.
            let behind = renderer.targets.glass_picture.as_ref().unwrap().texture();
            let data = renderer
                .read_texture(behind, wgpu::TextureAspect::All)
                .unwrap();
            let at = ((behind.height() / 2 * behind.width() + behind.width() / 2)
                * if enhanced { 8 } else { 4 }) as usize;
            if enhanced {
                let channel =
                    |i: usize| u16::from_le_bytes([data[at + i * 2], data[at + i * 2 + 1]]);
                assert!(channel(2) > channel(0) && channel(2) > channel(1));
            } else {
                assert!(data[at + 2] > data[at] && data[at + 2] > data[at + 1]);
            }
            scene.overlays[0].1 = [64.0, 0.0, 128.0, 4.0];
            let centred = renderer
                .render_triple_to_image(&mut scene, 193, 64, &cam, &lighting, &rig)
                .unwrap();
            for i in 0..3 {
                let at = ((2 * 193 + i * 64 + 32) * 4) as usize;
                assert_eq!(centred[at..at + 3].iter().all(|c| *c > 240), i == 1);
            }
            scene.overlays.clear();
            let clear = renderer
                .render_triple_to_image(&mut scene, 192, 64, &cam, &lighting, &rig)
                .unwrap();
            let at = ((2 * 192 + 96) * 4) as usize;
            assert!(!clear[at..at + 3].iter().all(|c| *c > 240));
            scene.overlays.push((white, [0.0, 0.0, 192.0, 4.0]));
        }
    }
}
