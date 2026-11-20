//! What a picture draws, decided in one place: `FrameFeatures::decide` is a function of the
//! graphics path, the options, the kind of view and the weather, with no GPU in it, so a test
//! can ask it for every view and path (`tests/frame_features.rs`).

use super::options::RenderOptions;
use crate::lighting::ShadingPath;
use crate::output::Output;
use crate::passes::FrameFeatures;

/// Where a picture goes, as far as what it draws is concerned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewKind {
    /// The window's picture, with the HUD and the overlays.
    Window,
    /// One panel of a triple screen: the window's picture with a projection of its own.
    TriplePanel,
    /// One eye of the headset.
    XrEye,
    /// A texture of the scene (a mirror, a screen).
    Mirror,
}

impl ViewKind {
    pub(crate) fn of(output: Output) -> Self {
        match output {
            Output::Window => ViewKind::Window,
            Output::TriplePanel { .. } => ViewKind::TriplePanel,
            Output::XrEye { .. } => ViewKind::XrEye,
            Output::Texture { .. } => ViewKind::Mirror,
        }
    }

    /// The window's pictures draw the HUD and the overlays.
    fn with_overlays(self) -> bool {
        matches!(self, ViewKind::Window | ViewKind::TriplePanel)
    }

    /// A picture with a projection of its own (the headset's eyes and the triple screen's
    /// panels are the real picture as much as the window is).
    fn own_projection(self) -> bool {
        matches!(self, ViewKind::TriplePanel | ViewKind::XrEye)
    }
}

/// What this renderer was built with: a pass that is not there cannot be drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuiltPasses {
    /// The Enhanced path's pipelines.
    pub enhanced: bool,
    /// The plain path's pipelines with the puddles' masks.
    pub reflections: bool,
    /// The puddles' ray pass.
    pub puddles: bool,
    /// The ambient occlusion passes.
    pub ssao: bool,
}

/// The switches of the environment the frame reads (`OMSI_NO_*`, `OMSI_MIRROR_ENHANCED`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameSwitches {
    pub no_fxaa: bool,
    pub no_glass_picture: bool,
    pub no_puddle_reflections: bool,
    pub no_ao: bool,
    pub mirror_enhanced: bool,
    /// A debug view of the shading is on (the puddles do not draw over it).
    pub debug_view: bool,
}

impl FrameSwitches {
    pub(crate) fn from_environment() -> Self {
        Self {
            no_fxaa: omsi_cfg::flags::NO_FXAA.is_set(),
            no_glass_picture: omsi_cfg::flags::NO_GLASS_PICTURE.is_set(),
            no_puddle_reflections: omsi_cfg::flags::NO_PUDDLE_REFLECTIONS.is_set(),
            no_ao: omsi_cfg::flags::NO_AO.is_set(),
            mirror_enhanced: omsi_cfg::flags::MIRROR_ENHANCED.is_set(),
            debug_view: crate::lighting::debug_view() != 0.0,
        }
    }
}

/// What the decision reads of a picture.
#[derive(Clone, Copy, Debug)]
pub struct FrameInputs {
    pub view: ViewKind,
    /// The leading view: the window, or the headset's first eye.
    pub lead_view: bool,
    /// The window's size, and the size of the picture (smaller with the render scale).
    pub full_size: (u32, u32),
    pub picture_size: (u32, u32),
    pub rain: f32,
    pub wetness: f32,
    pub snow: f32,
    /// The scene has a glass picture slot (a pane with rain on it).
    pub has_glass_slot: bool,
    /// Some material takes the puddles (`moisture` above 0).
    pub has_puddle_material: bool,
    pub built: BuiltPasses,
    pub switches: FrameSwitches,
}

/// Whether the weather is wet enough for puddles that mirror (snow covers them).
pub(crate) fn puddle_weather(wetness: f32, snow: f32) -> bool {
    wetness * (1.0 - snow.clamp(0.0, 1.0)) > 0.05
}

impl FrameFeatures {
    /// What a picture draws, from the graphics `path`, the renderer's `options` and the
    /// picture's `inputs`.
    pub fn decide(path: ShadingPath, options: &RenderOptions, i: &FrameInputs) -> FrameFeatures {
        let with_overlays = i.view.with_overlays();
        let xr_view = i.view.own_projection();
        // the path's own pipelines are there
        let enhanced_path = path == ShadingPath::Hdr && i.built.enhanced;
        // FXAA on the plain graphics too, where there is no multisampling to smooth the edges
        // (the Enhanced path has it in its post passes): the picture is drawn into a texture
        // of the window's size and smoothed on its way to the window, the HUD after
        let fxaa_without_msaa = with_overlays
            && i.picture_size == i.full_size
            && options.fxaa
            && options.msaa <= 1
            && !enhanced_path
            && !i.switches.no_fxaa;
        // Rain films are drawn after copying the clean current-frame scene. Classic graphics
        // need a sampleable scene target too, even without puddles or scaling.
        let glass_on = with_overlays
            && i.has_glass_slot
            && (i.rain > 0.001 || i.wetness > 0.02)
            && !i.switches.no_glass_picture;
        let scaled =
            i.picture_size != i.full_size || fxaa_without_msaa || (glass_on && !enhanced_path);
        // The mirrors are drawn with the plain shading even in Enhanced: without the depth
        // prepass (sized for the window) every layer of a mirror's picture ran the enhanced
        // shader, 12.7 ms of GPU time for one 256-pixel mirror against 5.8 ms for the whole
        // window; plainly shaded it is 0.6 ms, and a mirror's small picture shows no
        // difference worth that. `OMSI_MIRROR_ENHANCED=1` draws them enhanced again. The
        // headset's eyes are the real picture as much as the window is (#784: VR showed the
        // plain graphics with Enhanced on).
        let enhanced = enhanced_path && (with_overlays || xr_view || i.switches.mirror_enhanced);
        // Rain reflections belong to all graphics modes. Classic shading uses the same
        // scene/mask targets, then presents their linear colour without Enhanced grading.
        let puddles_wanted = with_overlays
            && i.built.puddles
            && options.reflections
            && puddle_weather(i.wetness, i.snow)
            && i.has_puddle_material
            && !i.switches.debug_view
            && !i.switches.no_puddle_reflections;
        let reflection_frame = !enhanced && puddles_wanted && i.built.reflections;
        let masked_frame = enhanced || reflection_frame;
        // ambient occlusion only for the real picture, not for the mirrors
        let ao_on = with_overlays && options.ssao && i.built.ssao && !i.switches.no_ao;
        // the enhanced path's shading is costly: the depth prepass keeps it to the visible
        // surface (without multisampling)
        let prepass_on =
            ao_on || puddles_wanted || glass_on || (enhanced && (with_overlays || xr_view));
        FrameFeatures {
            with_overlays,
            fxaa_without_msaa,
            glass_on,
            scaled,
            xr_view,
            lead_view: i.lead_view,
            enhanced,
            puddles_wanted,
            reflection_frame,
            masked_frame,
            ao_on,
            prepass_on,
        }
    }
}
