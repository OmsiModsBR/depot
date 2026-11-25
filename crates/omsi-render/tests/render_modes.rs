//! What each render mode decides for a frame (`interface::RenderMode`), as data: no graphics
//! adapter is needed. The two modes answer the steps the renderer used to settle by testing
//! which shading path was on.

use glam::{DVec3, Vec3};
use omsi_render::testing::modes::{
    draws_light, fog_cull, lamp_shadows, light_radius, pipe_set, shading,
};
use omsi_render::testing::{
    BuiltPasses, FrameFeatures, FrameInputs, FrameSwitches, ModeView, PipeSet, ViewKind,
};
use omsi_render::{LightMode, PointLight, RenderOptions, ShadingPath};

fn view(with_overlays: bool, xr_view: bool, mirror_enhanced: bool, built: bool) -> ModeView {
    ModeView {
        with_overlays,
        xr_view,
        mirror_enhanced,
        built,
    }
}

fn features(path: ShadingPath, view: ViewKind) -> FrameFeatures {
    FrameFeatures::decide(
        path,
        &RenderOptions {
            msaa: 1,
            ..Default::default()
        },
        &FrameInputs {
            view,
            lead_view: true,
            full_size: (1280, 720),
            picture_size: (1280, 720),
            rain: 0.0,
            wetness: 0.0,
            snow: 0.0,
            has_glass_slot: false,
            has_puddle_material: false,
            built: BuiltPasses {
                own_mode: true,
                reflections: true,
                puddles: true,
                ssao: true,
            },
            switches: FrameSwitches::default(),
        },
    )
}

#[test]
fn the_plain_path_is_a_vanilla_plus_frame_in_every_view() {
    for v in [
        view(true, false, false, true),
        view(false, false, true, true),
        view(true, true, false, true),
    ] {
        let s = shading(ShadingPath::Linear, &v);
        assert!(!s.own_post && !s.own_shading);
    }
}

#[test]
fn enhanced_shades_the_window_and_the_eyes_and_the_mirrors_plainly() {
    let window = shading(ShadingPath::Hdr, &view(true, false, false, true));
    assert!(window.own_post && window.own_shading);
    let eye = shading(ShadingPath::Hdr, &view(false, true, false, true));
    assert!(eye.own_post && eye.own_shading);
    // a mirror: finished by the post passes, shaded plainly, unless asked otherwise
    let mirror = shading(ShadingPath::Hdr, &view(false, false, false, true));
    assert!(mirror.own_post && !mirror.own_shading);
    let mirror_enhanced = shading(ShadingPath::Hdr, &view(false, false, true, true));
    assert!(mirror_enhanced.own_shading);
}

#[test]
fn enhanced_without_its_pipelines_draws_nothing_of_its_own() {
    let s = shading(ShadingPath::Hdr, &view(true, false, true, false));
    assert!(!s.own_post && !s.own_shading);
}

#[test]
fn the_pipelines_follow_the_frame() {
    let window = features(ShadingPath::Hdr, ViewKind::Window);
    assert_eq!(pipe_set(ShadingPath::Hdr, &window), PipeSet::Own);
    let mirror = features(ShadingPath::Hdr, ViewKind::Mirror);
    assert_eq!(pipe_set(ShadingPath::Hdr, &mirror), PipeSet::Plain);
    let plain = features(ShadingPath::Linear, ViewKind::Window);
    assert_eq!(pipe_set(ShadingPath::Linear, &plain), PipeSet::Plain);
    // the rain's reflections draw the plain shading in high range, in either mode
    let mut reflecting = plain;
    reflecting.reflection_frame = true;
    assert_eq!(
        pipe_set(ShadingPath::Linear, &reflecting),
        PipeSet::Reflections
    );
    let mut reflecting = mirror;
    reflecting.reflection_frame = true;
    assert_eq!(
        pipe_set(ShadingPath::Hdr, &reflecting),
        PipeSet::Reflections
    );
}

#[test]
fn only_an_enhanced_frame_casts_lamp_shadows() {
    let window = features(ShadingPath::Hdr, ViewKind::Window);
    assert!(lamp_shadows(ShadingPath::Hdr, &window));
    let mirror = features(ShadingPath::Hdr, ViewKind::Mirror);
    assert!(!lamp_shadows(ShadingPath::Hdr, &mirror));
    let plain = features(ShadingPath::Linear, ViewKind::Window);
    assert!(!lamp_shadows(ShadingPath::Linear, &plain));
}

#[test]
fn a_lamp_is_drawn_by_the_mode_that_knows_its_kind() {
    let spot = PointLight {
        radius: 60.0,
        intensity: 20.0,
        direction: Vec3::new(0.0, 2.0, -0.6),
        cone: [0.97, 0.82],
        core: 1.0,
        beam: 24.0,
        mode: LightMode::Enhanced,
        position: DVec3::ZERO,
        ..Default::default()
    };
    // a spot is Enhanced's: the plain path passes it by
    let enhanced = features(ShadingPath::Hdr, ViewKind::Window);
    let plain = features(ShadingPath::Linear, ViewKind::Window);
    assert!(draws_light(ShadingPath::Hdr, &spot, &enhanced));
    assert!(!draws_light(ShadingPath::Linear, &spot, &plain));
    // a plain lamp is drawn by both; a headlight's stand-in only by the plain path
    let lamp = PointLight {
        radius: 30.0,
        intensity: 1.0,
        ..Default::default()
    };
    let stand_in = PointLight {
        radius: 18.0,
        intensity: 0.8,
        mode: LightMode::Vanilla,
        ..Default::default()
    };
    assert!(draws_light(ShadingPath::Hdr, &lamp, &enhanced));
    assert!(draws_light(ShadingPath::Linear, &lamp, &plain));
    assert!(draws_light(ShadingPath::Linear, &stand_in, &plain));
    assert!(!draws_light(ShadingPath::Hdr, &stand_in, &enhanced));
    // a lamp with no intensity gives no light
    let dark = PointLight {
        intensity: 0.0,
        ..lamp
    };
    assert!(!draws_light(ShadingPath::Hdr, &dark, &enhanced));
    // (and the vanilla shader reads its radius as 0)
    assert_eq!(light_radius(ShadingPath::Linear, &spot, &plain), 0.0);
}

#[test]
fn each_mode_says_how_far_the_fog_lets_the_picture_see() {
    let window_enhanced = features(ShadingPath::Hdr, ViewKind::Window);
    let window_plain = features(ShadingPath::Linear, ViewKind::Window);
    let mirror = features(ShadingPath::Hdr, ViewKind::Mirror);
    // the plain fog: 99 % gone at 4.6 / density
    let (far, own) = fog_cull(ShadingPath::Linear, &window_plain, 0.01, 5000.0);
    assert!(!own && (far - 460.0).abs() < 1e-2);
    // none at all: the camera's range
    let (far, own) = fog_cull(ShadingPath::Linear, &window_plain, 0.0, 5000.0);
    assert!(!own && far == 5000.0);
    // Enhanced has no fog below the weather's 1e-4 (the clear air is the sky model's)...
    let (far, own) = fog_cull(ShadingPath::Hdr, &window_enhanced, 5e-5, 5000.0);
    assert!(own && far == 5000.0);
    // ...and above it the layer thins with height, so a camera at the base sees as far as the
    // plain density says
    let (far, own) = fog_cull(ShadingPath::Hdr, &window_enhanced, 0.01, 5000.0);
    assert!(own && far >= 460.0 && far <= 5000.0);
    // a mirror shaded plainly has the plain fog, whatever the mode
    let (far, own) = fog_cull(ShadingPath::Hdr, &mirror, 0.01, 5000.0);
    assert!(!own && (far - 460.0).abs() < 1e-2);
}
