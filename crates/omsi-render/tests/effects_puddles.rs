//! Tests moved out of `src/effects/puddles.rs`.

use omsi_render::testing::internals::effects::puddles::*;
use omsi_render::testing::internals::ShadingPath;
use omsi_render::testing::internals::*;

#[test]
fn reflection_preserves_clearance_at_any_height_grade_and_floating_origin() {
    for height in [-125.0, 0.09, 12.0, 501.0] {
        for normal in [Vec3::Z, Vec3::new(0.1, -0.04, 1.0).normalize()] {
            let ground = DVec3::new(-2165.0, 736.0, height);
            let origin = DVec3::new(-2200.0, 700.0, (height / 100.0).floor() * 100.0);
            let plane = reflection_plane(ground, normal, origin);
            let point = (ground - origin).as_vec3() + normal * 0.4;
            let reflected = point - 2.0 * normal * (normal.dot(point) - plane.w);
            assert!((normal.dot(point) - plane.w - 0.4).abs() < 1e-4);
            assert!((normal.dot(reflected) - plane.w + 0.4).abs() < 1e-4);
            assert!(((point + reflected) * 0.5 - (ground - origin).as_vec3()).length() < 1e-4);
        }
    }
}

#[test]
#[ignore = "requires a graphics adapter; checks transparent window reflection depth"]
fn glass_hit_depth_preserves_receiver_depth_across_resizes() {
    let mut r = test_renderer(4, ShadingPath::Hdr);
    let mut scene = r.new_scene();
    let back = test_material(
        &r,
        &mut scene,
        [1.0; 4],
        false,
        1.0,
        MaterialExtra::default(),
    );
    let pane = test_material(
        &r,
        &mut scene,
        [0.5, 0.5, 0.5, 0.15],
        false,
        0.0,
        MaterialExtra {
            glass: true,
            ..Default::default()
        },
    );
    for (y, half, material) in [(10.0, 10.0, back), (5.0, 2.0, pane)] {
        test_quad(
            &r,
            &mut scene,
            [
                [-half, y, -half],
                [half, y, -half],
                [half, y, half],
                [-half, y, half],
            ],
            -Vec3::Y,
            material,
        );
    }
    let camera = Camera {
        position: DVec3::ZERO,
        yaw: 0.0,
        pitch: 0.0,
        roll: 0.0,
        fov_deg: 90.0,
        near: 0.1,
        far: 100.0,
    };
    let mut lighting = Lighting {
        path: ShadingPath::Hdr,
        wetness: 1.0,
        shadows: false,
        ..Default::default()
    };
    fn depth_at_centre(r: &Renderer, texture: &wgpu::Texture, size: u32) -> f32 {
        let bytes = r
            .read_texture(texture, wgpu::TextureAspect::DepthOnly)
            .unwrap();
        let offset = ((size / 2 * size + size / 2) * 4) as usize;
        f32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
    }
    for inside in [false, true] {
        lighting.inside = inside.then_some((DVec3::ZERO, 0.0, [12.0, 12.0, 12.0, 0.0, 0.0, 0.0]));
        for size in [64, 48, 64] {
            r.render_to_image(&mut scene, size, size, &camera, &lighting)
                .unwrap();
            let targets = r.targets.hdr_targets[&(size, size)]
                .puddles
                .as_ref()
                .unwrap();
            assert_eq!(
                targets.source_depth,
                *r.targets.ao.as_ref().unwrap().depth_view.texture()
            );
            // Read both textures below through an immutable renderer borrow.
            let receiver = depth_at_centre(&r, &targets.source_depth, size);
            let hit = depth_at_centre(&r, &targets.hit_depth, size);
            assert!(
                (receiver - 0.009009).abs() < 1e-5,
                "opaque receiver: {receiver}"
            );
            let expected_hit = if inside { receiver } else { 0.019019 };
            assert!((hit - expected_hit).abs() < 1e-5, "glass hit: {hit}");
        }
    }
}

fn test_renderer(msaa: u32, path: ShadingPath) -> Renderer {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    pollster::block_on(Renderer::new_with(
        &instance,
        None,
        Some(wgpu::TextureFormat::Rgba8UnormSrgb),
        RenderOptions {
            msaa,
            ssao: false,
            fxaa: false,
            shadow_size: 1024,
            render_scale: 1.0,
            path,
            ..Default::default()
        },
    ))
    .expect("test renderer")
}

fn test_material(
    r: &Renderer,
    scene: &mut Scene,
    colour: [f32; 4],
    unlit: bool,
    moisture: f32,
    extra: MaterialExtra,
) -> MaterialId {
    let alpha = if extra.glass || extra.rain_film {
        AlphaMode::Blend
    } else {
        AlphaMode::Opaque
    };
    r.add_material_inner(
        scene, None, alpha, colour, unlit, None, None, None, None, None, [0.0; 3], moisture, extra,
    )
}

fn test_quad(
    r: &Renderer,
    scene: &mut Scene,
    positions: [[f32; 3]; 4],
    normal: Vec3,
    material: MaterialId,
) -> usize {
    let mesh = r.add_mesh(
        scene,
        &MeshData {
            positions: positions.map(Vec3::from).to_vec(),
            normals: vec![normal; 4],
            uvs: vec![
                glam::Vec2::ZERO,
                glam::Vec2::X,
                glam::Vec2::ONE,
                glam::Vec2::Y,
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
            ranges: vec![(0, 6, 0)],
            one_sided: false,
        },
    );
    r.add_instance(scene, mesh, DVec3::ZERO, Mat4::IDENTITY, vec![material])
}

fn refraction_source(r: &Renderer, own_shading: bool) -> Vec<f32> {
    let texture = r.targets.glass_picture.as_ref().unwrap().texture();
    let bytes = r.read_texture(texture, wgpu::TextureAspect::All).unwrap();
    if texture.format() != HDR_FORMAT {
        return bytes.into_iter().map(|v| v as f32 / 255.0).collect();
    }
    let pre = if own_shading {
        r.adaptation.exposure.unwrap().exp()
    } else {
        1.0
    };
    bytes
        .chunks_exact(2)
        .enumerate()
        .map(|(i, b)| {
            half_to_f32(u16::from_le_bytes(b.try_into().unwrap()))
                / if i % 4 == 3 { 1.0 } else { pre }
        })
        .collect()
}

fn image_difference(a: &[u8], b: &[u8]) -> u64 {
    a.iter().zip(b).map(|(a, b)| a.abs_diff(*b) as u64).sum()
}

#[test]
#[ignore = "requires a graphics adapter; renders wet roads through bus glass"]
fn wet_scene_reflections_survive_bus_glass_in_every_graphics_mode() {
    for (msaa, path) in [
        (1, ShadingPath::Linear),
        (1, ShadingPath::Hdr),
        (4, ShadingPath::Linear),
        (4, ShadingPath::Hdr),
    ] {
        let mut r = test_renderer(msaa, path);
        let mut scene = r.new_scene();
        let road = test_material(
            &r,
            &mut scene,
            [0.25, 0.25, 0.25, 1.0],
            false,
            1.0,
            MaterialExtra::default(),
        );
        let red = test_material(
            &r,
            &mut scene,
            [1.0, 0.01, 0.01, 1.0],
            true,
            0.0,
            MaterialExtra::default(),
        );
        let pane = test_material(
            &r,
            &mut scene,
            [0.25, 0.3, 0.35, 0.15],
            false,
            0.0,
            MaterialExtra {
                glass: true,
                ..Default::default()
            },
        );
        let film = test_material(
            &r,
            &mut scene,
            [1.0; 4],
            false,
            0.0,
            MaterialExtra {
                rain_film: true,
                no_z_write: true,
                ..Default::default()
            },
        );
        test_quad(
            &r,
            &mut scene,
            [
                [-20.0, 0.0, 0.0],
                [20.0, 0.0, 0.0],
                [20.0, 30.0, 0.0],
                [-20.0, 30.0, 0.0],
            ],
            Vec3::Z,
            road,
        );
        test_quad(
            &r,
            &mut scene,
            [
                [-3.0, 10.0, 0.0],
                [3.0, 10.0, 0.0],
                [3.0, 10.0, 5.0],
                [-3.0, 10.0, 5.0],
            ],
            -Vec3::Y,
            red,
        );
        let window = test_quad(
            &r,
            &mut scene,
            [
                [-3.0, 1.0, -1.0],
                [3.0, 1.0, -1.0],
                [3.0, 1.0, 6.0],
                [-3.0, 1.0, 6.0],
            ],
            -Vec3::Y,
            pane,
        );
        let rain = test_quad(
            &r,
            &mut scene,
            [
                [-3.0, 0.99, -1.0],
                [3.0, 0.99, -1.0],
                [3.0, 0.99, 6.0],
                [-3.0, 0.99, 6.0],
            ],
            -Vec3::Y,
            film,
        );
        let camera = Camera {
            position: DVec3::new(0.0, 0.0, 2.0),
            yaw: 0.0,
            pitch: -10.0,
            roll: 0.0,
            fov_deg: 70.0,
            near: 0.1,
            far: 100.0,
        };
        {
            let lighting = Lighting {
                path,
                wetness: 1.0,
                rain: 0.8,
                shadows: false,
                inside: Some((camera.position, 0.0, [4.0, 4.0, 4.0, 0.0, 0.0, 0.0])),
                ..Default::default()
            };
            let mut reflection = Vec::new();
            for glass in 0..3 {
                r.set_params(&mut scene, window, &[], glass > 0, &[]);
                r.set_params(&mut scene, rain, &[], glass == 2, &[]);
                r.options.reflections = false;
                let without = r
                    .render_to_image(&mut scene, 128, 128, &camera, &lighting)
                    .unwrap();
                r.options.reflections = true;
                let with = r
                    .render_to_image(&mut scene, 128, 128, &camera, &lighting)
                    .unwrap();
                // Only the road: the red wall is above this region.
                let red_change: i64 = with[128 * 70 * 4..]
                    .chunks_exact(4)
                    .zip(without[128 * 70 * 4..].chunks_exact(4))
                    .map(|(a, b)| (a[0] as i64 - a[1] as i64) - (b[0] as i64 - b[1] as i64))
                    .sum();
                assert!(
                    red_change > 1000,
                    "missing reflection: {red_change}; MSAA={msaa}, path={path:?}, glass={glass}"
                );
                reflection.push(red_change);
            }
            assert!(
                reflection[1..].iter().all(|v| *v > reflection[0] / 4),
                "glass hid the reflection: {reflection:?}"
            );

            // The road stays still while drops move. Settle the sky after changing weather.
            let steady = Lighting {
                rain: 0.0,
                ..lighting
            };
            r.set_params(&mut scene, window, &[], true, &[]);
            r.set_params(&mut scene, rain, &[0.0], true, &[]);
            for _ in 0..3 {
                r.render_to_image(&mut scene, 128, 128, &camera, &steady)
                    .unwrap();
            }
            let clean = r
                .render_to_image(&mut scene, 128, 128, &camera, &steady)
                .unwrap();
            let behind = refraction_source(&r, path == ShadingPath::Hdr);
            r.set_params(&mut scene, rain, &[1.0], true, &[]);
            for _ in 0..4 {
                let wet = r
                    .render_to_image(&mut scene, 128, 128, &camera, &steady)
                    .unwrap();
                assert!(
                    image_difference(&wet, &clean) > 300,
                    "rain film was not exercised"
                );
                let current = refraction_source(&r, path == ShadingPath::Hdr);
                let drift = current
                    .iter()
                    .zip(&behind)
                    .map(|(a, b)| (a - b).abs())
                    .sum::<f32>()
                    / current.len() as f32;
                assert!(
                    drift < 0.002,
                    "rain/wiper feedback: {drift}; MSAA={msaa}, path={path:?}"
                );
            }
            r.set_params(&mut scene, rain, &[0.0], true, &[]);
            let wiped = r
                .render_to_image(&mut scene, 128, 128, &camera, &steady)
                .unwrap();
            assert!(
                image_difference(&wiped, &clean) < 128 * 128,
                "wiper left old film colour"
            );
        }
    }
}

#[test]
fn reflection_depth_uses_panes_not_rain_decals_or_terrain() {
    let mut pane = <MaterialUniform as bytemuck::Zeroable>::zeroed();
    pane.params[0] = 2.0;
    pane.bump[2] = 1.0;
    pane.emissive[3] = 1.0;
    assert!(reflection_glass(&pane));
    for (field, value) in [(0, 2.0), (1, 1.0), (2, 1.0), (3, 1.0)] {
        let mut other = pane;
        match field {
            0 => other.emissive[3] = value,
            1 => other.bump[3] = value,
            2 => other.extra[0] = value,
            _ => other.params[1] = value,
        }
        assert!(!reflection_glass(&other));
    }
    pane.emissive[3] = 0.0;
    assert!(!reflection_glass(&pane));
    pane.params2[1] = 0.4;
    assert!(reflection_glass(&pane));
    pane.bump[2] = 0.0;
    assert!(!reflection_glass(&pane));
}

#[test]
fn ray_budget_stays_bounded_at_large_and_odd_sizes() {
    for (w, h) in [
        (1, 1),
        (1279, 719),
        (1920, 1080),
        (3840, 2160),
        (7680, 4320),
        (2160, 3840),
    ] {
        let (tw, th) = trace_size(w, h);
        assert!(tw >= 1 && th >= 1 && tw <= w && th <= h);
        assert!(tw * th <= MAX_TRACE_PIXELS as u32);
        if w >= 2 && h >= 2 {
            assert!(tw <= w / 2 && th <= h / 2);
        }
    }
}
