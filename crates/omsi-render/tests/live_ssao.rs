//! Ambient occlusion switched on or off in a running renderer (`Renderer::set_ssao`) draws
//! the picture a renderer started with that choice draws (spec 001, FR-020e).

use glam::{DVec3, Mat4, Vec2, Vec3};
use omsi_geometry::MeshData;
use omsi_render::{AlphaMode, Camera, Lighting, RenderOptions, Renderer, ShadingPath};

const SIZE: u32 = 96;
/// Pictures drawn before the comparison, the switch made after the first.
const FRAMES: usize = 3;

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

/// A groove (a wall standing on the floor with a shelf over its foot, the occlusion deep in
/// the corner) drawn `FRAMES` times, starting with occlusion `start`; after the first picture
/// it is set to `then`. The last picture as grey levels.
fn render(path: ShadingPath, start: bool, then: bool) -> Vec<i32> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let mut renderer = pollster::block_on(Renderer::new_with(
        &instance,
        None,
        Some(wgpu::TextureFormat::Rgba8UnormSrgb),
        RenderOptions {
            msaa: 1,
            ssao: start,
            fxaa: false,
            render_scale: 1.0,
            ..Default::default()
        },
    ))
    .expect("test renderer");
    let mut scene = renderer.new_scene();
    let paint = renderer.add_material(&mut scene, None, AlphaMode::Opaque, [0.4; 4], false);
    let x8 = Vec3::new(8.0, 0.0, 0.0);
    for (a, v, n) in [
        (
            Vec3::new(-4.0, -4.0, 0.0),
            Vec3::new(0.0, 5.0, 0.0),
            Vec3::Z,
        ),
        (
            Vec3::new(-4.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 3.0),
            -Vec3::Y,
        ),
        (
            Vec3::new(-4.0, 0.6, 0.6),
            Vec3::new(0.0, 0.4, 0.0),
            -Vec3::Z,
        ),
        (
            Vec3::new(-4.0, 0.6, 0.6),
            Vec3::new(0.0, 0.0, 0.05),
            -Vec3::Y,
        ),
    ] {
        let mesh = renderer.add_mesh(&mut scene, &quad(a, x8, v, n));
        renderer.add_surface_instance(&mut scene, mesh, DVec3::ZERO, Mat4::IDENTITY, vec![paint]);
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
        path,
        shadows: false,
        fog_density: 0.0,
        ..Default::default()
    };
    let mut img = Vec::new();
    for frame in 0..FRAMES {
        if frame == 1 {
            renderer.set_ssao(then);
        }
        img = renderer
            .render_to_image(&mut scene, SIZE, SIZE, &camera, &lighting)
            .unwrap();
    }
    img.chunks(4)
        .map(|p| (p[0] as i32 + p[1] as i32 + p[2] as i32) / 3)
        .collect()
}

fn largest_gap(a: &[i32], b: &[i32]) -> i32 {
    a.iter().zip(b).map(|(a, b)| (a - b).abs()).max().unwrap()
}

#[test]
#[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
fn ambient_occlusion_switched_in_a_running_renderer_draws_as_a_fresh_start() {
    for path in [ShadingPath::Linear, ShadingPath::Hdr] {
        let on = render(path, true, true);
        let off = render(path, false, false);
        // (the groove must actually be shaded, or the test says nothing)
        let shade = largest_gap(&on, &off);
        assert!(
            shade >= 5,
            "no ambient occlusion in the groove ({path:?}): {shade}"
        );
        // off -> on
        let switched_on = render(path, false, true);
        assert!(
            largest_gap(&switched_on, &on) <= 2,
            "switched on, it differs from a start with it on ({path:?})"
        );
        // on -> off
        let switched_off = render(path, true, false);
        assert!(
            largest_gap(&switched_off, &off) <= 2,
            "switched off, it differs from a start without it ({path:?})"
        );
    }
}
