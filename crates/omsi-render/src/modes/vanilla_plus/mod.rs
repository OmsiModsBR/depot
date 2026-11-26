//! Vanilla+: the plain shading in the swap chain's format, with FXAA where there is no
//! multisampling and the puddles' own reflection pass.

pub mod pipelines;

use crate::effects::puddles::PuddleLook;
use crate::interface::{FogCull, LeadViewParts, ModeShading, ModeView, PipeSet, RenderMode};
use crate::lighting::{gpu_light, lights_up, GpuPointLight, LightMode, Lighting, PointLight};
use crate::passes::{Exposure, FrameFeatures, PassPipelines, PathBuilders};
use crate::scene::Camera;
use crate::sky::SkyResources;
use glam::Vec3;

pub struct VanillaPlus;

/// The plain shading in the swap chain's format, which every picture without the other mode's
/// shading is drawn with.
pub fn plain_pass(builders: &PathBuilders<'_>, format: wgpu::TextureFormat) -> PassPipelines {
    pipelines::plain(builders, format)
}

/// The plain shading in high range, for the puddles' reflections.
pub fn reflection_pass(builders: &PathBuilders<'_>, hdr: wgpu::TextureFormat) -> PassPipelines {
    pipelines::reflections(builders, hdr)
}

/// The WGSL Vanilla+ adds to the scene module (`fs_vanilla_reflections`).
pub fn scene_text() -> &'static str {
    include_str!("shaders/reflections.wgsl")
}

impl RenderMode for VanillaPlus {
    fn shading(&self, _view: &ModeView) -> ModeShading {
        ModeShading {
            own_post: false,
            own_shading: false,
        }
    }

    fn pipe_set(&self, features: &FrameFeatures) -> PipeSet {
        if features.reflection_frame {
            PipeSet::Reflections
        } else {
            PipeSet::Plain
        }
    }

    fn draws_light(&self, l: &PointLight, _features: &FrameFeatures) -> bool {
        lights_up(l) && l.mode != LightMode::Enhanced
    }

    fn light_record(&self, l: &PointLight, p: Vec3, _features: &FrameFeatures) -> GpuPointLight {
        gpu_light(l, p)
    }

    fn lamp_shadows(&self, _features: &FrameFeatures) -> bool {
        false
    }

    fn lead_view(&self, _features: &FrameFeatures, _parts: LeadViewParts<'_>) {}

    fn fog_cull(&self, _features: &FrameFeatures, lighting: &Lighting, camera: &Camera) -> FogCull {
        FogCull::plain(lighting, camera)
    }

    fn puddle_look(
        &self,
        _features: &FrameFeatures,
        lighting: &Lighting,
        _sky: &SkyResources,
        _adaptation: &Exposure,
    ) -> Option<PuddleLook> {
        Some(PuddleLook::plain(lighting))
    }
}
