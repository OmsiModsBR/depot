//! What each render mode decides for a frame (`interface::RenderMode`), as data: no graphics
//! adapter is needed. The two modes answer the steps the renderer used to settle by testing
//! which shading path was on.

use glam::{DVec3, Vec3};
use omsi_render::testing::modes::{draws_light, lamp_shadows, light_radius, pipe_set, shading};
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
                enhanced: true,
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
    // (and the vanilla shader reads its radius as 0)
    assert_eq!(light_radius(ShadingPath::Linear, &spot, &plain), 0.0);
}
