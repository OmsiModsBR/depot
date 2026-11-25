//! What a render mode decides for the renderer, one method per step of a frame. The
//! renderer asks the mode instead of testing which shading path is on; a mode is built by
//! `renderer/build.rs`, the one function that names them.
//!
//! The steps here are the ones that need nothing but data (what the frame is, the lights,
//! the camera): the mode answers with a value, or changes the parts the core hands it. The
//! passes themselves (the HDR frame, the probe) are still recorded by the
//! renderer; they move into the modes with phase C5.

use crate::effects::puddles::PuddleLook;
use crate::lighting::{ExposureLog, GpuPointLight, Lighting, PointLight};
use crate::passes::{Exposure, FrameFeatures, PassPipelines, PostPipelines};
use crate::scene::{Camera, Scene, TextureId};
use crate::sky::SkyResources;
use crate::targets::{GroupParts, TargetCache};
use glam::{Mat4, Vec3};

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

/// How far a picture sees through the fog, for the draws that are left out beyond it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FogCull {
    /// Nothing farther than this (m) is drawn, where the mode's fog is the plain one.
    pub far: f32,
    /// The mode's fog is its own: the ground and the big things are drawn whatever their
    /// distance (the sky below the horizon is not the fog's colour), and the shadow blocks are
    /// not left out by it.
    pub own: bool,
}

impl FogCull {
    /// The plain fog's: nothing behind it is drawn, where the fog has swallowed 99 % of a thing
    /// there is nothing left to see of it (a storm's 700 m of sight cuts the draws of a city
    /// map by two thirds).
    pub fn plain(lighting: &Lighting, camera: &Camera) -> Self {
        let far = if lighting.fog_density > 1e-7 {
            (4.6 / lighting.fog_density).min(camera.far)
        } else {
            camera.far
        };
        Self { far, own: false }
    }
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

/// What the lamps' light in the fog is drawn from and into (`RenderMode::encode_fog_lamps`).
pub(crate) struct FogLampsParts<'a> {
    pub queue: &'a wgpu::Queue,
    /// The fog's half-size target and its two bind groups.
    pub fog: Option<(&'a wgpu::TextureView, &'a [wgpu::BindGroup; 2])>,
    /// The picture the lamps' light is added onto.
    pub picture: Option<&'a wgpu::TextureView>,
    pub scene: &'a Scene,
    pub encoder: &'a mut wgpu::CommandEncoder,
    pub lighting: &'a Lighting,
    pub vp_mat: Mat4,
    pub width: u32,
    pub height: u32,
    pub features: &'a FrameFeatures,
    pub tset: &'a Option<wgpu::QuerySet>,
    pub timed: &'a mut Vec<&'static str>,
}

/// What the mode's post passes work with (`ModeResources::encode_post`): the core's post
/// pipelines, the targets, the eye's adaptation and the sky it follows, and the picture.
pub(crate) struct PostParts<'a> {
    pub queue: &'a wgpu::Queue,
    pub post: &'a PostPipelines,
    pub targets: &'a TargetCache,
    pub adaptation: &'a mut Exposure,
    pub sky: &'a SkyResources,
    /// The options' multisampling and FXAA.
    pub msaa: u32,
    pub fxaa: bool,
    pub instant_exposure: bool,
    /// The exposure log, when this picture is sampled into it (the window's, the device
    /// not lost).
    pub exposure_log: Option<&'a mut ExposureLog>,
    /// The presenter's pipeline for the HUD overlays at full size.
    pub overlay_pipeline_1x: &'a wgpu::RenderPipeline,
    pub scene: &'a Scene,
    pub encoder: &'a mut wgpu::CommandEncoder,
    pub lighting: &'a Lighting,
    pub scene_view: &'a wgpu::TextureView,
    pub overlays: &'a [(TextureId, [f32; 4])],
    pub width: u32,
    pub height: u32,
    pub dt: f32,
    pub features: &'a FrameFeatures,
    pub puddles_on: bool,
    pub tset: &'a Option<wgpu::QuerySet>,
    pub timed: &'a mut Vec<&'static str>,
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

    /// How far a picture of `features` sees through the fog (nothing behind it is drawn).
    fn fog_cull(&self, features: &FrameFeatures, lighting: &Lighting, camera: &Camera) -> FogCull;

    /// How the puddles' reflections are lit in a picture of `features`, or none while the
    /// mode's light is not ready (its sky is still being worked out).
    fn puddle_look(
        &self,
        features: &FrameFeatures,
        lighting: &Lighting,
        sky: &SkyResources,
        adaptation: &Exposure,
    ) -> Option<PuddleLook>;
}

/// What a mode draws with that only it needs, kept by the renderer for as long as the mode is
/// built (none where the mode is left out). The steps of [`RenderMode`] are decisions;
/// these are the mode's own pipelines and passes.
pub(crate) trait ModeResources: Send + Sync {
    /// The pipelines of the main pass in the mode's own shading .
    fn own_pipelines(&self) -> Option<&PassPipelines> {
        None
    }

    /// The same, to swap a pipeline in a test.
    #[cfg(test)]
    fn own_pipelines_mut(&mut self) -> Option<&mut PassPipelines> {
        None
    }

    /// The enhanced sky tone-mapped into the plain pass's target, for the mirrors of a
    /// picture in the mode's own shading.
    fn sky_mirror_pipeline(&self) -> Option<&wgpu::RenderPipeline> {
        None
    }

    /// The layout and parameters of the lamps' light in the fog, when the mode draws it
    /// (the targets make the bind groups it is read through).
    fn fog_lamps_group(&self) -> Option<GroupParts<'_>> {
        None
    }

    /// The lamps' light in the fog and the sun's shafts, over the picture before its glow
    /// and metering.
    fn encode_fog_lamps(&self, _parts: FogLampsParts<'_>) {}

    /// The mode's post passes over its picture: glow, metering, adaptation, tone curve and
    /// FXAA; nothing for a mode that leaves the picture as it was drawn.
    fn encode_post(&self, _parts: PostParts<'_>) {}
}
