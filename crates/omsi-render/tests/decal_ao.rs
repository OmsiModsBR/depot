//! A blended layer printed on an opaque surface - a `[useTextTexture]` destination or fleet
//! number on its sign, a sticker - takes the screen-space ambient occlusion of the surface it
//! lies on (Vanilla+ and Enhanced). Left out of it as every blended surface was, its
//! letters stood out bright where the sign around them lay in the dashboard's shade.

use glam::{DVec3, Mat4, Vec2, Vec3};
use omsi_geometry::MeshData;
use omsi_render::{AlphaMode, Camera, Lighting, RenderOptions, Renderer};

/// A quad from `a` along `u` and `v`, facing `n`.
fn quad(a: Vec3, u: Vec3, v: Vec3, n: Vec3) -> MeshData {
    MeshData {
        positions: vec![a, a + u, a + u + v, a + v],
        normals: vec![n; 4],
        uvs: vec![Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
        indices: vec![0, 1, 2, 0, 2, 3],
        ranges: vec![(0, 6, 0)],
        one_sided: false,
    }
}

const SIZE: usize = 96;

/// A groove: a wall standing on the floor with a shelf over its foot, the AO deep in the
/// corner; with `layer`, the right half of the wall under the shelf wears a blended layer of
/// the wall's own grey 5 mm in front of it. The picture as grey levels.
fn render(ssao: bool, layer: bool, enhanced: bool) -> Vec<i32> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let mut renderer = pollster::block_on(Renderer::new_with(
        &instance,
        None,
        Some(wgpu::TextureFormat::Rgba8UnormSrgb),
        RenderOptions {
            msaa: 1,
            ssao,
            fxaa: false,
            render_scale: 1.0,
            ..Default::default()
        },
    ))
    .expect("test renderer");
    let mut scene = renderer.new_scene();
    // (grey: white came out saturated, its shade clipped away)
    let grey = [0.4, 0.4, 0.4, 1.0];
    let paint = renderer.add_material(&mut scene, None, AlphaMode::Opaque, grey, false);
    let blend = renderer.add_material(&mut scene, None, AlphaMode::Blend, grey, false);
    let x8 = Vec3::new(8.0, 0.0, 0.0);
    let mut meshes = vec![
        // the floor, the wall, the shelf's underside and its front edge
        (
            quad(
                Vec3::new(-4.0, -4.0, 0.0),
                x8,
                Vec3::new(0.0, 5.0, 0.0),
                Vec3::Z,
            ),
            paint,
        ),
        (
            quad(
                Vec3::new(-4.0, 1.0, 0.0),
                x8,
                Vec3::new(0.0, 0.0, 3.0),
                -Vec3::Y,
            ),
            paint,
        ),
        (
            quad(
                Vec3::new(-4.0, 0.6, 0.6),
                x8,
                Vec3::new(0.0, 0.4, 0.0),
                -Vec3::Z,
            ),
            paint,
        ),
        (
            quad(
                Vec3::new(-4.0, 0.6, 0.6),
                x8,
                Vec3::new(0.0, 0.0, 0.05),
                -Vec3::Y,
            ),
            paint,
        ),
    ];
    if layer {
        meshes.push((
            quad(
                Vec3::new(0.0, 0.995, 0.0),
                Vec3::new(4.0, 0.0, 0.0),
                Vec3::new(0.0, 0.0, 0.6),
                -Vec3::Y,
            ),
            blend,
        ));
    }
    for (data, mat) in meshes {
        let mesh = renderer.add_mesh(&mut scene, &data);
        renderer.add_surface_instance(&mut scene, mesh, DVec3::ZERO, Mat4::IDENTITY, vec![mat]);
    }
    let camera = Camera {
        position: DVec3::new(0.0, -1.0, 0.3),
        yaw: 0.0,
        pitch: 0.0,
        roll: 0.0,
        fov_deg: 70.0,
        near: 0.1,
        far: 100.0,
    };
    let lighting = Lighting {
        enhanced,
        shadows: false,
        fog_density: 0.0,
        ..Default::default()
    };
    let img = renderer
        .render_to_image(&mut scene, SIZE as u32, SIZE as u32, &camera, &lighting)
        .unwrap();
    img.chunks(4)
        .map(|p| (p[0] as i32 + p[1] as i32 + p[2] as i32) / 3)
        .collect()
}

#[test]
#[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
fn a_blended_layer_on_a_surface_takes_its_ambient_occlusion() {
    // the picture's right half, where the layer lies (its edges left out)
    let region = |img: &[i32]| -> Vec<i32> {
        (0..SIZE)
            .flat_map(|y| (SIZE / 2 + 4..SIZE - 4).map(move |x| (x, y)))
            .map(|(x, y)| img[y * SIZE + x])
            .collect()
    };
    let largest_gap = |a: &[i32], b: &[i32]| -> i32 {
        a.iter().zip(b).map(|(a, b)| (a - b).abs()).max().unwrap()
    };
    for enhanced in [false, true] {
        let bare = region(&render(true, false, enhanced));
        // (the groove must actually be shaded, or the test says nothing)
        let shade = largest_gap(&bare, &region(&render(false, false, enhanced)));
        let covered = region(&render(true, true, enhanced));
        let off = largest_gap(&bare, &covered);
        assert!(
            shade >= 5,
            "no ambient occlusion in the groove (enhanced {enhanced}): {shade}"
        );
        assert!(
            off * 2 < shade,
            "the layer is not shaded as the wall under it (enhanced {enhanced}): off by {off}, \
             the shade is {shade}"
        );
    }
}
