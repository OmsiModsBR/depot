//! What a picture draws, for every view and graphics path (`FrameFeatures::decide`): a pure
//! decision, so no graphics adapter is needed.

use omsi_render::testing::{BuiltPasses, FrameFeatures, FrameInputs, FrameSwitches, ViewKind};
use omsi_render::{RenderOptions, ShadingPath};

const WINDOW: (u32, u32) = (1280, 720);

fn options() -> RenderOptions {
    RenderOptions {
        msaa: 1,
        fxaa: true,
        ssao: true,
        reflections: true,
        ..Default::default()
    }
}

fn inputs(view: ViewKind) -> FrameInputs {
    FrameInputs {
        view,
        lead_view: true,
        full_size: WINDOW,
        picture_size: WINDOW,
        rain: 0.0,
        wetness: 0.0,
        snow: 0.0,
        has_glass_slot: true,
        has_puddle_material: true,
        built: BuiltPasses {
            own_mode: true,
            reflections: true,
            puddles: true,
            ssao: true,
        },
        switches: FrameSwitches::default(),
    }
}

fn decide(path: ShadingPath, options: &RenderOptions, i: &FrameInputs) -> FrameFeatures {
    FrameFeatures::decide(path, options, i)
}

#[test]
fn the_window_in_the_plain_path_smooths_its_edges_after_the_scene() {
    let f = decide(ShadingPath::Linear, &options(), &inputs(ViewKind::Window));
    assert!(f.with_overlays && f.fxaa_without_msaa && f.scaled);
    assert!(f.ao_on && f.prepass_on);
    assert!(!f.own_shading && !f.masked_frame && !f.xr_view);
}

#[test]
fn multisampling_or_a_smaller_picture_leaves_the_fxaa_out() {
    let msaa = RenderOptions {
        msaa: 4,
        ..options()
    };
    let f = decide(ShadingPath::Linear, &msaa, &inputs(ViewKind::Window));
    assert!(!f.fxaa_without_msaa && !f.scaled);
    // a render scale below 100 %: the scene goes through a scaled target, the edges are
    // left to the scaling
    let mut small = inputs(ViewKind::Window);
    small.picture_size = (960, 540);
    let f = decide(ShadingPath::Linear, &options(), &small);
    assert!(f.scaled && !f.fxaa_without_msaa);
}

#[test]
fn the_window_in_enhanced_is_drawn_by_the_enhanced_path() {
    let f = decide(ShadingPath::Hdr, &options(), &inputs(ViewKind::Window));
    assert!(f.own_shading && f.masked_frame && f.prepass_on && f.ao_on);
    // (Enhanced has its FXAA in its post passes)
    assert!(!f.fxaa_without_msaa && !f.scaled);
}

#[test]
fn enhanced_without_its_pipelines_draws_the_plain_path() {
    let mut i = inputs(ViewKind::Window);
    i.built.own_mode = false;
    let f = decide(ShadingPath::Hdr, &options(), &i);
    assert!(!f.own_shading && f.fxaa_without_msaa);
}

#[test]
fn a_mirror_in_enhanced_uses_the_base_shading() {
    let f = decide(ShadingPath::Hdr, &options(), &inputs(ViewKind::Mirror));
    assert!(!f.own_shading && !f.masked_frame);
    assert!(!f.with_overlays && !f.ao_on && !f.prepass_on && !f.xr_view);
    // OMSI_MIRROR_ENHANCED draws them enhanced again
    let f = decide(
        ShadingPath::Hdr,
        &RenderOptions {
            mirror_enhanced: true,
            ..options()
        },
        &inputs(ViewKind::Mirror),
    );
    assert!(f.own_shading && f.masked_frame);
    // (still without the occlusion and the overlays of the window's picture)
    assert!(!f.ao_on && !f.with_overlays);
}

#[test]
fn a_mirror_in_the_plain_path_is_just_drawn() {
    let f = decide(ShadingPath::Linear, &options(), &inputs(ViewKind::Mirror));
    assert!(!f.fxaa_without_msaa && !f.scaled && !f.glass_on && !f.ao_on && !f.prepass_on);
}

#[test]
fn the_headsets_eyes_are_the_real_picture_without_the_window_s_overlays() {
    let f = decide(ShadingPath::Hdr, &options(), &inputs(ViewKind::XrEye));
    assert!(f.own_shading && f.xr_view && !f.with_overlays);
    // (the depth prepass keeps the costly shading to the visible surface; no occlusion)
    assert!(f.prepass_on && !f.ao_on);
    let f = decide(ShadingPath::Linear, &options(), &inputs(ViewKind::XrEye));
    assert!(!f.own_shading && !f.prepass_on);
}

#[test]
fn a_triple_screen_panel_is_the_windows_picture_with_a_projection() {
    let f = decide(ShadingPath::Hdr, &options(), &inputs(ViewKind::TriplePanel));
    assert!(f.with_overlays && f.xr_view && f.own_shading && f.ao_on);
}

#[test]
fn only_the_leading_view_is_marked() {
    let mut i = inputs(ViewKind::XrEye);
    i.lead_view = false;
    assert!(!decide(ShadingPath::Hdr, &options(), &i).lead_view);
    i.lead_view = true;
    assert!(decide(ShadingPath::Hdr, &options(), &i).lead_view);
}

#[test]
fn rain_on_the_glass_is_drawn_for_the_window_only() {
    let mut i = inputs(ViewKind::Window);
    i.rain = 0.5;
    let plain = decide(ShadingPath::Linear, &options(), &i);
    assert!(plain.glass_on && plain.prepass_on && plain.scaled);
    // (Enhanced samples its own targets: no scaled target for the glass)
    let enhanced = decide(ShadingPath::Hdr, &options(), &i);
    assert!(enhanced.glass_on && !enhanced.scaled);
    i.has_glass_slot = false;
    assert!(!decide(ShadingPath::Hdr, &options(), &i).glass_on);
    let mut mirror = inputs(ViewKind::Mirror);
    mirror.rain = 0.5;
    assert!(!decide(ShadingPath::Hdr, &options(), &mirror).glass_on);
    // a wet street alone is enough, a dry one with no rain is not
    let mut wet = inputs(ViewKind::Window);
    wet.wetness = 0.5;
    assert!(decide(ShadingPath::Linear, &options(), &wet).glass_on);
    wet.wetness = 0.01;
    assert!(!decide(ShadingPath::Linear, &options(), &wet).glass_on);
}

#[test]
fn puddles_draw_their_reflections_in_both_paths() {
    let mut i = inputs(ViewKind::Window);
    i.wetness = 0.5;
    let plain = decide(ShadingPath::Linear, &options(), &i);
    assert!(plain.puddles_wanted && plain.reflection_frame && plain.masked_frame);
    // (Enhanced is masked already: the puddles do not need a frame of their own)
    let enhanced = decide(ShadingPath::Hdr, &options(), &i);
    assert!(enhanced.puddles_wanted && !enhanced.reflection_frame && enhanced.masked_frame);
}

#[test]
fn puddles_wait_for_what_they_need() {
    let wet = || {
        let mut i = inputs(ViewKind::Window);
        i.wetness = 0.5;
        i
    };
    let wanted =
        |o: &RenderOptions, i: &FrameInputs| decide(ShadingPath::Linear, o, i).puddles_wanted;
    assert!(wanted(&options(), &wet()));
    // snow covers them
    let mut i = wet();
    i.snow = 1.0;
    assert!(!wanted(&options(), &i));
    // the reflections off, a street with nothing that takes puddles, no puddle pass, a debug view
    let off = RenderOptions {
        reflections: false,
        ..options()
    };
    assert!(!wanted(&off, &wet()));
    let mut i = wet();
    i.has_puddle_material = false;
    assert!(!wanted(&options(), &i));
    let mut i = wet();
    i.built.puddles = false;
    assert!(!wanted(&options(), &i));
    let mut i = wet();
    i.switches.debug_view = true;
    assert!(!wanted(&options(), &i));
    let no_puddles = RenderOptions {
        puddle_reflections: false,
        ..options()
    };
    assert!(!wanted(&no_puddles, &wet()));
    // not in a mirror
    let mut i = inputs(ViewKind::Mirror);
    i.wetness = 0.5;
    assert!(!wanted(&options(), &i));
    // without the plain path's masked pipelines the puddles are wanted but not drawn by it
    let mut i = wet();
    i.built.reflections = false;
    let f = decide(ShadingPath::Linear, &options(), &i);
    assert!(f.puddles_wanted && !f.reflection_frame && !f.masked_frame);
}

#[test]
fn the_options_turn_a_pass_off() {
    let mut i = inputs(ViewKind::Window);
    i.rain = 0.5;
    let off = RenderOptions {
        ssao: false,
        fxaa: false,
        glass_picture: false,
        ..options()
    };
    let f = decide(ShadingPath::Linear, &off, &i);
    assert!(!f.ao_on && !f.fxaa_without_msaa && !f.glass_on && !f.scaled && !f.prepass_on);
}

#[test]
fn the_occlusion_follows_the_option_and_the_built_pass() {
    let off = RenderOptions {
        ssao: false,
        ..options()
    };
    assert!(!decide(ShadingPath::Linear, &off, &inputs(ViewKind::Window)).ao_on);
    let mut i = inputs(ViewKind::Window);
    i.built.ssao = false;
    assert!(!decide(ShadingPath::Linear, &options(), &i).ao_on);
    // (Enhanced keeps its prepass without the occlusion)
    assert!(decide(ShadingPath::Hdr, &off, &inputs(ViewKind::Window)).prepass_on);
}
