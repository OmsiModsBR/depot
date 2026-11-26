//! What a picture draws, decided in one place: `FrameFeatures::decide` is a function of the
//! graphics path, the options, the kind of view and the weather, with no GPU in it, so a test
//! can ask it for every view and path (`tests/frame_features.rs`).

use super::options::RenderOptions;
use crate::interface::{ModeView, RenderMode};
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
    pub fn of(output: Output) -> Self {
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
    /// The mode's own pipelines.
    pub own_mode: bool,
    /// The plain path's pipelines with the puddles' masks.
    pub reflections: bool,
    /// The puddles' ray pass.
    pub puddles: bool,
    /// The ambient occlusion passes.
    pub ssao: bool,
}

/// What a diagnostic switch of the environment says about the frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameSwitches {
    /// A debug view of the shading is on (the puddles do not draw over it).
    pub debug_view: bool,
}

impl FrameSwitches {
    pub fn from_environment() -> Self {
        Self {
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
pub fn puddle_weather(wetness: f32, snow: f32) -> bool {
    wetness * (1.0 - snow.clamp(0.0, 1.0)) > 0.05
}

impl FrameFeatures {
    /// What a picture draws, from the graphics `path`, the renderer's `options` and the
    /// picture's `inputs`.
    pub fn decide(path: ShadingPath, options: &RenderOptions, i: &FrameInputs) -> FrameFeatures {
        Self::decide_with(super::build::mode_for(path), options, i)
    }

    /// `decide` for the render `mode` of the graphics path.
    pub fn decide_with(
        mode: &dyn RenderMode,
        options: &RenderOptions,
        i: &FrameInputs,
    ) -> FrameFeatures {
        let with_overlays = i.view.with_overlays();
        let xr_view = i.view.own_projection();
        let shading = mode.shading(&ModeView {
            with_overlays,
            xr_view,
            mirror_enhanced: options.mirror_enhanced,
            built: i.built.own_mode,
        });
        // the mode's own post passes finish the picture
        let enhanced_path = shading.own_post;
        // FXAA on the plain graphics too, where there is no multisampling to smooth the edges
        // (the Enhanced path has it in its post passes): the picture is drawn into a texture
        // of the window's size and smoothed on its way to the window, the HUD after
        let fxaa_without_msaa = with_overlays
            && i.picture_size == i.full_size
            && options.fxaa
            && options.msaa <= 1
            && !enhanced_path;
        // Rain films are drawn after copying the clean current-frame scene. Classic graphics
        // need a sampleable scene target too, even without puddles or scaling.
        let glass_on = with_overlays
            && i.has_glass_slot
            && (i.rain > 0.001 || i.wetness > 0.02)
            && options.glass_picture;
        let scaled =
            i.picture_size != i.full_size || fxaa_without_msaa || (glass_on && !enhanced_path);
        // (which mirrors are shaded plainly is the mode's to say: `Enhanced::shading`)
        let own_shading = shading.own_shading;
        // Rain reflections belong to all graphics modes. Classic shading uses the same
        // scene/mask targets, then presents their linear colour without Enhanced grading.
        let puddles_wanted = with_overlays
            && i.built.puddles
            && options.reflections
            && options.puddle_reflections
            && puddle_weather(i.wetness, i.snow)
            && i.has_puddle_material
            && !i.switches.debug_view;
        let reflection_frame = !own_shading && puddles_wanted && i.built.reflections;
        let masked_frame = own_shading || reflection_frame;
        // ambient occlusion only for the real picture, not for the mirrors
        let ao_on = with_overlays && options.ssao && i.built.ssao;
        // the enhanced path's shading is costly: the depth prepass keeps it to the visible
        // surface (without multisampling)
        let prepass_on =
            ao_on || puddles_wanted || glass_on || (own_shading && (with_overlays || xr_view));
        FrameFeatures {
            with_overlays,
            fxaa_without_msaa,
            glass_on,
            scaled,
            xr_view,
            lead_view: i.lead_view,
            own_shading,
            puddles_wanted,
            reflection_frame,
            masked_frame,
            ao_on,
            prepass_on,
        }
    }
}
