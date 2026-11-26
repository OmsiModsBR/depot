//! What the shared core hands to Vanilla's renderer, and what Vanilla's device gets of it
//! (spec 001, FR-023): the same `FramePlan` always gives the same `SceneFrame`, every field of
//! the plan reaches it, what the options leave out is left out, and the constants the Rust
//! and the shader sides both hold agree.
//!
//! `NullBackend` takes only the simple `DrawBatch` of the crate's backend trait; the device's
//! scene entry point has no recording stand-in, so the contract is checked on the `SceneFrame`
//! the device takes, which is all the device reads of a picture.

use omsi_render::{FramePlan, PlanBatch, PlanKind, RenderOptions, Renderer, Scene};
use omsi_render_vanilla::{DrawKind, SceneBatch};
use openomsi_game::testing::output::vanilla::{
    scene_batches, scene_frame, ui_overlays, FrameParts, VanillaEffects,
};

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

const KINDS: [(PlanKind, DrawKind); 6] = [
    (PlanKind::Opaque, DrawKind::Opaque),
    (PlanKind::AlphaTest, DrawKind::AlphaTest),
    (PlanKind::Blend, DrawKind::Blend),
    (PlanKind::BlendNoWrite, DrawKind::BlendNoWrite),
    (PlanKind::SurfaceDepth, DrawKind::SurfaceDepth),
    (PlanKind::TerrainPaint, DrawKind::TerrainPaint),
];

fn vec4(seed: f32) -> [f32; 4] {
    [seed, seed + 0.1, seed + 0.2, seed + 0.3]
}

/// A plan whose every field has its own value, so a field that is dropped or crossed shows.
fn plan() -> FramePlan {
    let batch = |k: usize, kind: PlanKind| PlanBatch {
        kind,
        cull: k % 2 == 0,
        surface: k % 3 == 0,
        mesh: 10 + k,
        first: 100 * k as u32,
        count: 3 * (k as u32 + 1),
        material: 20 + k,
        instances: (k as u32)..(k as u32 + 4),
    };
    FramePlan {
        width: 1280,
        height: 720,
        target: None,
        render_origin: glam::DVec3::new(100.0, 200.0, 0.0),
        view_proj: std::array::from_fn(|c| vec4(c as f32)),
        cam_pos: vec4(1.0),
        sun_dir: vec4(2.0),
        sun_color: vec4(3.0),
        ambient: vec4(4.0),
        fog: vec4(5.0),
        sky_color: vec4(6.0),
        cam_right: vec4(7.0),
        cam_up: vec4(8.0),
        world_origin: vec4(9.0),
        wetness: 0.6,
        rain_reflection: Some((7, std::array::from_fn(|c| vec4(30.0 + c as f32)))),
        light_grid: vec4(10.0),
        light_map: vec4(11.0),
        clear: vec4(12.0),
        rain: 0.4,
        sky: vec4(13.0),
        clouds: vec4(14.0),
        sun_glow: true,
        moon: vec4(15.0),
        stars: [vec4(16.0), vec4(17.0), vec4(18.0)],
        lamp_cap: 5,
        camera: Vec::new(),
        list: vec![4, 3, 2, 1, 0],
        main: KINDS
            .iter()
            .enumerate()
            .map(|(k, (kind, _))| batch(k, *kind))
            .collect(),
        cab: vec![batch(7, PlanKind::Blend), batch(8, PlanKind::Opaque)],
        overlays: vec![(3, [1.0, 2.0, 3.0, 4.0]), (5, [5.0, 6.0, 7.0, 8.0])],
    }
}

fn translate<'a>(
    plan: &'a FramePlan,
    scene: &'a Scene,
    effects: &VanillaEffects,
    main: &'a [SceneBatch],
    cab: &'a [SceneBatch],
) -> omsi_render_vanilla::SceneFrame<'a> {
    scene_frame(
        plan,
        scene,
        FrameParts {
            main,
            cab,
            lights: &[],
        },
        effects,
        0.25,
    )
}

#[test]
fn every_batch_of_the_plan_reaches_the_device_with_its_kind_and_its_range() {
    let plan = plan();
    let mut main = Vec::new();
    scene_batches(&plan.main, &mut main);
    assert_eq!(main.len(), plan.main.len());
    for (k, (got, want)) in main.iter().zip(&plan.main).enumerate() {
        assert_eq!(got.kind, KINDS[k].1, "batch {k}");
        assert_eq!(got.cull, want.cull, "batch {k}");
        assert_eq!(got.surface, want.surface, "batch {k}");
        assert_eq!(got.mesh, want.mesh, "batch {k}");
        assert_eq!(got.first, want.first, "batch {k}");
        assert_eq!(got.count, want.count, "batch {k}");
        assert_eq!(got.material, want.material, "batch {k}");
        assert_eq!(got.instances, want.instances, "batch {k}");
    }
}

#[test]
fn the_plans_camera_light_lamps_and_sky_reach_the_frame() {
    let renderer = noop_renderer();
    let scene = renderer.new_scene();
    let plan = plan();
    let (mut main, mut cab) = (Vec::new(), Vec::new());
    scene_batches(&plan.main, &mut main);
    scene_batches(&plan.cab, &mut cab);
    let frame = translate(&plan, &scene, &VanillaEffects::default(), &main, &cab);
    assert_eq!(frame.view_proj, plan.view_proj);
    assert_eq!(frame.list, plan.list.as_slice());
    assert_eq!(frame.main.len(), plan.main.len());
    assert_eq!(frame.cab.len(), plan.cab.len());
    let light = frame.light;
    assert_eq!(light.cam_pos, plan.cam_pos);
    assert_eq!(light.sun_dir, plan.sun_dir);
    assert_eq!(light.sun_color, plan.sun_color);
    assert_eq!(light.ambient, plan.ambient);
    assert_eq!(light.fog, plan.fog);
    assert_eq!(light.sky_color, plan.sky_color);
    assert_eq!(light.cam_right, plan.cam_right);
    assert_eq!(light.cam_up, plan.cam_up);
    assert_eq!(
        light.weather,
        [
            plan.wetness,
            0.0,
            plan.world_origin[0],
            plan.world_origin[1]
        ]
    );
    assert_eq!(frame.lamps.place, plan.light_grid);
    assert_eq!(frame.lamps.tile_map, plan.light_map);
    assert_eq!(frame.lamps.cell_cap, plan.lamp_cap as usize);
    assert_eq!(frame.sky.params, plan.sky);
    assert_eq!(frame.sky.clouds.params, plan.clouds);
    assert_eq!(frame.sky.moon_dir, plan.moon);
    assert_eq!(frame.sky.star_frame, plan.stars);
    assert_eq!(frame.rain.intensity, plan.rain);
    assert_eq!(frame.rain.seconds, 0.25);
    assert_eq!(frame.rain_reflection, plan.rain_reflection);
}

#[test]
fn the_same_plan_always_gives_the_same_frame() {
    let renderer = noop_renderer();
    let scene = renderer.new_scene();
    let plan = plan();
    let (mut main, mut cab) = (Vec::new(), Vec::new());
    scene_batches(&plan.main, &mut main);
    scene_batches(&plan.cab, &mut cab);
    let effects = VanillaEffects::default();
    let first = format!("{:?}", translate(&plan, &scene, &effects, &main, &cab));
    let second = format!(
        "{:?}",
        translate(&plan.clone(), &scene, &effects, &main, &cab)
    );
    assert_eq!(first, second);
}

#[test]
fn what_the_options_leave_out_is_left_out_of_the_frame() {
    let renderer = noop_renderer();
    let scene = renderer.new_scene();
    let plan = plan();
    let (mut main, mut cab) = (Vec::new(), Vec::new());
    scene_batches(&plan.main, &mut main);
    scene_batches(&plan.cab, &mut cab);
    let all = VanillaEffects::default();
    let on = translate(&plan, &scene, &all, &main, &cab);
    assert_eq!(on.rain.intensity, plan.rain);
    assert!(!on.sky.no_cloud_layer && !on.sky.no_stars);

    let none = VanillaEffects {
        glass_rain: false,
        coronas: false,
        smoke: false,
        stars: false,
        cloud_picture: false,
        ..all
    };
    let off = translate(&plan, &scene, &none, &main, &cab);
    assert_eq!(off.rain.intensity, 0.0, "no drops gather");
    assert_eq!(off.rain.seconds, 0.25, "and the time still runs");
    assert!(off.rain.wiper.is_none());
    assert!(off.coronas.sprites.is_empty() && off.coronas.runs.is_empty());
    assert!(off.smoke.sprites.is_empty());
    assert!(off.sky.no_cloud_layer && off.sky.no_stars);
    // (what the plan carries of the light is not an effect)
    assert_eq!(off.sky.params, plan.sky);
    assert_eq!(off.light.ambient, plan.ambient);
}

#[test]
fn the_huds_pictures_keep_their_order_and_rects() {
    let renderer = noop_renderer();
    let scene = renderer.new_scene();
    let plan = plan();
    let mut overlays = Vec::new();
    ui_overlays(&plan, &scene, &mut overlays);
    assert_eq!(overlays.len(), plan.overlays.len());
    for (got, &(texture, rect)) in overlays.iter().zip(&plan.overlays) {
        assert_eq!(got.texture, texture);
        assert_eq!(got.rect, rect);
        assert!(!got.premultiplied && !got.transposed);
    }
}

/// The number a `name = value;` line of a shader gives, in the file `path` under `crates/`.
fn shader_constant(path: &str, name: &str) -> f64 {
    let full = format!("{}/../{path}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&full).unwrap_or_else(|e| panic!("{full}: {e}"));
    let line = text
        .lines()
        .find(|l| {
            let l = l.trim_start();
            l.contains(&format!(" {name}")) && l.contains('=') && l.contains("const")
        })
        .unwrap_or_else(|| panic!("no constant {name} in {path}"));
    let value = line.split('=').nth(1).expect("a value").trim();
    value
        .trim_end_matches(';')
        .trim_end_matches("f32")
        .trim()
        .parse()
        .unwrap_or_else(|e| panic!("{name} in {path}: {value:?}: {e}"))
}

const HLSL_SKY: &str = "omsi-render-vanilla/src/implementation/d3d9/shaders/sky.hlsl";
const WGSL_SKY: &str = "omsi-render/src/shaders/sky/sky.wgsl";

#[test]
fn the_sizes_of_the_suns_and_moons_pictures_agree_between_the_two_shader_sides() {
    for name in ["SUN_HALF", "MOON_RADIUS"] {
        assert_eq!(
            shader_constant(HLSL_SKY, name),
            shader_constant(WGSL_SKY, name),
            "{name}"
        );
    }
}

#[test]
fn the_pattern_period_of_the_wet_road_is_the_period_the_plan_is_taken_modulo() {
    // (the plan's `world_origin` is the map position modulo this many metres)
    assert_eq!(
        shader_constant(
            "omsi-render-vanilla/src/implementation/d3d9/shaders/wet_road.hlsl",
            "PERIOD"
        ),
        1000.0
    );
    let camera_uniform = std::fs::read_to_string(format!(
        "{}/../omsi-render/src/renderer/camera_uniform.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("camera_uniform.rs");
    assert!(
        camera_uniform.contains("ro.x.rem_euclid(1000.0)")
            && camera_uniform.contains("ro.y.rem_euclid(1000.0)"),
        "the plan's world origin is no longer taken modulo 1000 m"
    );
}
