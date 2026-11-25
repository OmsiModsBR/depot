//! What a picture draws: the choices its start settles (`renderer::features`) and the passes
//! branch on, as one value instead of a list of flags.

/// What a picture draws, decided at its start (`FrameFeatures::decide`): the passes and paths
/// the draw lists and the encoders branch on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameFeatures {
    /// The window's own pictures (the window and a triple screen's panels): the HUD and the
    /// overlays are drawn, the ambient occlusion and the glass picture belong to them.
    pub with_overlays: bool,
    /// The FXAA on the plain graphics, the picture drawn into a texture of the window's size.
    pub fxaa_without_msaa: bool,
    /// The rain's glass picture (what is behind the glass is drawn first).
    pub glass_on: bool,
    /// The picture goes through a scaled target (`scene_view`).
    pub scaled: bool,
    /// A picture with a projection of its own (a headset's eye or a triple screen's panel).
    pub xr_view: bool,
    /// The leading view: the one that moves the clocks, the exposure and the sky cube on.
    pub lead_view: bool,
    /// The picture is drawn by the Enhanced path.
    pub own_shading: bool,
    /// The puddles and their reflections (`reflection_frame` when drawn by their own pass).
    pub puddles_wanted: bool,
    pub reflection_frame: bool,
    /// The scene drawn into the masked targets (Enhanced or the reflections).
    pub masked_frame: bool,
    /// The ambient occlusion pass.
    pub ao_on: bool,
    /// The depth prepass before the shading.
    pub prepass_on: bool,
}
