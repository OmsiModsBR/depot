//! What a render mode decides for the renderer, one method per step of a frame. The
//! renderer asks the mode instead of testing which shading path is on; a mode is built by
//! `renderer/build.rs`, the one function that names them.
//!
//! The steps here are the ones that need nothing but data (what the frame is, the lights,
//! the camera): the mode answers with a value, or changes the parts the core hands it. The
//! passes themselves (the HDR frame, the post passes, the probe) are still recorded by the
//! renderer; they move into the modes with phase C5.

use crate::lighting::{GpuPointLight, Lighting, PointLight};
use crate::passes::{Exposure, FrameFeatures};
use crate::scene::{Camera, Scene};
use crate::sky::SkyResources;
use glam::Vec3;

/// What a picture is, as the mode needs it to choose its shading.
#[derive(Debug, Clone, Copy)]
pub struct ModeView {
    /// The window's own picture (not a mirror's or a render texture's).
    pub with_overlays: bool,
    /// A headset's eye.
    pub xr_view: bool,
    /// Draw the mirrors with the mode's own shading too (`RenderOptions::mirror_enhanced`).
    pub mirror_enhanced: bool,
    /// The mode's own pipelines were built.
    pub built: bool,
}

/// How a picture is shaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModeShading {
    /// The mode's own post passes finish the picture (it is drawn in high range). Mirrors of
    /// the window's frame are part of it even when they are shaded plainly.
    pub own_post: bool,
    /// The scene of this picture is drawn with the mode's own shading.
    pub own_shading: bool,
}

/// Which set of the main pass's pipelines draws a picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipeSet {
    /// The plain shading, in the swap chain's format.
    Plain,
    /// The mode's own shading, in high range.
    Own,
    /// The plain shading in high range, for the reflections of the puddles.
    Reflections,
}

/// What the mode may change when the window's picture leads the frame.
pub(crate) struct LeadViewParts<'a> {
    pub scene: &'a Scene,
    pub lighting: &'a Lighting,
    pub camera: &'a Camera,
    /// The camera's position relative to the scene's render origin.
    pub cam_rel: Vec3,
    pub aspect: f32,
    /// The metering the lamps in view feed.
    pub adaptation: &'a mut Exposure,
    /// The sky, which takes the glow of the lamps round the camera.
    pub sky: &'a mut SkyResources,
}

/// A render mode: the steps of a frame that differ between the shading paths.
pub(crate) trait RenderMode: Sync {
    /// How a picture of `view` is shaded.
    fn shading(&self, view: &ModeView) -> ModeShading;

    /// The pipelines a picture of these `features` is drawn with.
    fn pipe_set(&self, features: &FrameFeatures) -> PipeSet;

    /// Whether the lamp `l` is drawn by this mode's light records in a picture of `features`.
    fn draws_light(&self, l: &PointLight, features: &FrameFeatures) -> bool;

    /// The record of the lamp `l` at `p` (relative to the render origin) for the shader.
    fn light_record(&self, l: &PointLight, p: Vec3, features: &FrameFeatures) -> GpuPointLight;

    /// Whether street lamps cast shadows in a picture of `features`.
    fn lamp_shadows(&self, features: &FrameFeatures) -> bool;

    /// What the window's picture feeds the mode's own look before it is drawn (the lamps in
    /// view, the glow of the night sky); nothing for a mode with no such state.
    fn lead_view(&self, features: &FrameFeatures, parts: LeadViewParts<'_>);
}
