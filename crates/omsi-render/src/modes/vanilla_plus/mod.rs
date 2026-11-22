//! Vanilla+: the plain shading in the swap chain's format, with FXAA where there is no
//! multisampling and the puddles' own reflection pass.

use crate::interface::{LeadViewParts, ModeShading, ModeView, PipeSet, RenderMode};
use crate::lighting::{drawn_by, gpu_light, GpuPointLight, PointLight};
use crate::passes::FrameFeatures;
use glam::Vec3;

pub(crate) struct VanillaPlus;

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
        drawn_by(l, false)
    }

    fn light_record(&self, l: &PointLight, p: Vec3, _features: &FrameFeatures) -> GpuPointLight {
        gpu_light(l, p, false)
    }

    fn lamp_shadows(&self, _features: &FrameFeatures) -> bool {
        false
    }

    fn lead_view(&self, _features: &FrameFeatures, _parts: LeadViewParts<'_>) {}
}
