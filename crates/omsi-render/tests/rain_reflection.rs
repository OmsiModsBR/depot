//! The rain reflection's picture for a second renderer: what its mirrored camera leaves out
//! (the ground, and the people when asked) and how the window's frame plan hands the
//! reflection on. Runs on wgpu's noop device: no graphics adapter is needed.

use std::sync::{Arc, Mutex};

use glam::{DVec3, Mat4, Vec2, Vec3};
use omsi_geometry::MeshData;
use omsi_render::{AlphaMode, Camera, FramePlan, Lighting, RenderOptions, Renderer, Scene};

fn noop_renderer() -> Renderer {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = wgpu::Backends::NOOP;
    descriptor.backend_options.noop = wgpu::NoopBackendOptions { enable: true };
    let instance = wgpu::Instance::new(descriptor);
    pollster::block_on(Renderer::new_with(
        &instance,
        None,
        Some(wgpu::TextureFormat::Rgba8UnormSrgb),
        RenderOptions {
            msaa: 1,
            shadow_size: 1024,
            ..Default::default()
        },
    ))
    .expect("noop renderer")
}

/// A triangle standing up in front of a camera at the origin looking north (+y).
fn triangle() -> MeshData {
    MeshData {
        positions: vec![
            Vec3::new(-1.0, 0.0, -1.0),
            Vec3::new(1.0, 0.0, -1.0),
            Vec3::new(0.0, 0.0, 1.0),
        ],
        normals: vec![-Vec3::Y; 3],
        uvs: vec![Vec2::ZERO; 3],
        indices: vec![0, 1, 2],
        ranges: vec![(0, 3, 0)],
        ..Default::default()
    }
}

const CAMERA: Camera = Camera {
    position: DVec3::ZERO,
    yaw: 0.0,
    pitch: 0.0,
    roll: 0.0,
    fov_deg: 60.0,
    near: 0.1,
    far: 1000.0,
};

/// A scene of a house (an ordinary instance), a road (a surface) and a person, each its
/// own mesh, 10 m ahead; and a renderer whose plans are caught.
struct World {
    renderer: Renderer,
    scene: Scene,
    plans: Arc<Mutex<Vec<FramePlan>>>,
    house: usize,
    road: usize,
    person: usize,
}

fn world() -> World {
    let mut renderer = noop_renderer();
    let plans: Arc<Mutex<Vec<FramePlan>>> = Arc::default();
    let got = plans.clone();
    renderer.set_plan_sink(Some(Box::new(move |plan: &FramePlan, _: &Scene| {
        got.lock().unwrap().push(plan.clone());
    })));
    let mut scene = renderer.new_scene();
    let mat = renderer.add_material(&mut scene, None, AlphaMode::Opaque, [1.0; 4], true);
    let at = DVec3::new(0.0, 10.0, 0.0);
    let house = renderer.add_mesh(&mut scene, &triangle());
    renderer.add_instance(&mut scene, house, at, Mat4::IDENTITY, vec![mat]);
    let road = renderer.add_mesh(&mut scene, &triangle());
    renderer.add_surface_instance(&mut scene, road, at, Mat4::IDENTITY, vec![mat]);
    let person = renderer.add_mesh(&mut scene, &triangle());
    let i = renderer.add_instance(&mut scene, person, at, Mat4::IDENTITY, vec![mat]);
    renderer.set_human(&mut scene, i, true);
    World {
        renderer,
        scene,
        plans,
        house,
        road,
        person,
    }
}

impl World {
    /// The meshes the picture drawn into a texture with `lighting` draws, and its plan.
    fn reflection(&mut self, lighting: &Lighting) -> Vec<usize> {
        let tex = self.renderer.add_render_texture(&mut self.scene, 128, 256);
        self.renderer
            .render_to_texture(&mut self.scene, tex, &CAMERA, lighting, 1.6);
        let plans = self.plans.lock().unwrap();
        let plan = plans.last().expect("the texture's picture has a plan");
        assert_eq!(plan.target, Some(tex));
        plan.main.iter().map(|b| b.mesh).collect()
    }
}

#[test]
fn the_rain_reflection_leaves_the_ground_out_and_the_people_when_asked() {
    let mut w = world();
    let all = w.reflection(&Lighting::default());
    for mesh in [w.house, w.road, w.person] {
        assert!(
            all.contains(&mesh),
            "{mesh} drawn in a plain picture: {all:?}"
        );
    }
    let no_ground = w.reflection(&Lighting {
        ground_hidden: true,
        ..Default::default()
    });
    assert!(no_ground.contains(&w.house) && no_ground.contains(&w.person));
    assert!(
        !no_ground.contains(&w.road),
        "the road is left out: {no_ground:?}"
    );
    let no_people = w.reflection(&Lighting {
        ground_hidden: true,
        humans_hidden: true,
        ..Default::default()
    });
    assert!(no_people.contains(&w.house));
    assert!(
        !no_people.contains(&w.person),
        "the person is left out: {no_people:?}"
    );
}

#[test]
fn the_windows_plan_carries_the_rain_reflection_set_for_the_frame() {
    let mut w = world();
    let target = w.renderer.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: 64,
            height: 32,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    w.renderer
        .render(&mut w.scene, &view, 64, 32, &CAMERA, &Lighting::default());
    assert_eq!(
        w.plans.lock().unwrap().last().unwrap().rain_reflection,
        None
    );

    let tex = w.renderer.add_render_texture(&mut w.scene, 128, 256);
    let mirrored = Camera {
        position: DVec3::new(0.0, 0.0, -3.0),
        pitch: 10.0,
        ..CAMERA
    };
    w.renderer.set_rain_reflection(Some((tex, mirrored, 2.0)));
    w.renderer
        .render(&mut w.scene, &view, 64, 32, &CAMERA, &Lighting::default());
    let plan = w.plans.lock().unwrap().last().unwrap().clone();
    let want = mirrored
        .view_proj(2.0, plan.render_origin)
        .to_cols_array_2d();
    assert_eq!(plan.rain_reflection, Some((tex, want)));

    w.renderer.set_rain_reflection(None);
    w.renderer
        .render(&mut w.scene, &view, 64, 32, &CAMERA, &Lighting::default());
    assert_eq!(
        w.plans.lock().unwrap().last().unwrap().rain_reflection,
        None
    );
}
