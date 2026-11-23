//! Enhanced: its own lighting in high range, with the lamps' glow on the night sky, metering
//! and tone mapping. Mirrors are shaded plainly unless asked otherwise.

mod fog_lamps;

use crate::interface::{
    FogLampsParts, LeadViewParts, ModeResources, ModeShading, ModeView, PipeSet, RenderMode,
};
use crate::lighting::{
    drawn_by, gpu_light, lamp_sky_glow, view_lamp_light, GpuPointLight, PointLight,
};
use crate::passes::{FrameFeatures, PassPipelines};
use crate::targets::GroupParts;
use glam::Vec3;

#[cfg(test)]
pub(crate) use fog_lamps::FogLampUniform;
pub(crate) use fog_lamps::{fog_lamps_pipelines, FogLampsPass};

pub(crate) struct Enhanced;

/// What only the enhanced mode draws with: its main-pass pipelines in high range, the sky of
/// its mirrors, and the lamps' light in the fog.
pub(crate) struct EnhancedResources {
    pass: PassPipelines,
    sky_mirror: wgpu::RenderPipeline,
    fog_lamps: FogLampsPass,
}

impl EnhancedResources {
    pub(crate) fn new(
        pass: PassPipelines,
        sky_mirror: wgpu::RenderPipeline,
        fog_lamps: FogLampsPass,
    ) -> Self {
        Self {
            pass,
            sky_mirror,
            fog_lamps,
        }
    }
}

impl ModeResources for EnhancedResources {
    fn own_pipelines(&self) -> Option<&PassPipelines> {
        Some(&self.pass)
    }

    #[cfg(test)]
    fn own_pipelines_mut(&mut self) -> Option<&mut PassPipelines> {
        Some(&mut self.pass)
    }

    fn sky_mirror_pipeline(&self) -> Option<&wgpu::RenderPipeline> {
        Some(&self.sky_mirror)
    }

    fn fog_lamps_group(&self) -> Option<GroupParts<'_>> {
        self.fog_lamps
            .fog_lamps_pipeline
            .is_some()
            .then_some(GroupParts {
                layout: &self.fog_lamps.fog_lamps_layout,
                buf: &self.fog_lamps.fog_lamps_buf,
            })
    }

    fn encode_fog_lamps(&self, parts: FogLampsParts<'_>) {
        self.fog_lamps.encode_fog_lamps(parts);
    }
}

impl RenderMode for Enhanced {
    fn shading(&self, view: &ModeView) -> ModeShading {
        // The mirrors are drawn with the plain shading even in Enhanced: without the depth
        // prepass (sized for the window) every layer of a mirror's picture ran the enhanced
        // shader, 12.7 ms of GPU time for one 256-pixel mirror against 5.8 ms for the whole
        // window; plainly shaded it is 0.6 ms, and a mirror's small picture shows no
        // difference worth that. `OMSI_MIRROR_ENHANCED=1` draws them enhanced again. The
        // headset's eyes are the real picture as much as the window is (#784: VR showed the
        // plain graphics with Enhanced on).
        ModeShading {
            own_post: view.built,
            own_shading: view.built && (view.with_overlays || view.xr_view || view.mirror_enhanced),
        }
    }

    fn pipe_set(&self, features: &FrameFeatures) -> PipeSet {
        if features.enhanced {
            PipeSet::Own
        } else if features.reflection_frame {
            PipeSet::Reflections
        } else {
            PipeSet::Plain
        }
    }

    fn draws_light(&self, l: &PointLight, features: &FrameFeatures) -> bool {
        drawn_by(l, features.enhanced)
    }

    fn light_record(&self, l: &PointLight, p: Vec3, features: &FrameFeatures) -> GpuPointLight {
        gpu_light(l, p, features.enhanced)
    }

    fn lamp_shadows(&self, features: &FrameFeatures) -> bool {
        features.enhanced
    }

    fn lead_view(&self, features: &FrameFeatures, p: LeadViewParts<'_>) {
        // (a mirror takes the window's light - its own call would move the exposure on -
        // unless it comes before the window's first frame)
        if !(features.enhanced && features.lead_view) {
            return;
        }
        let LeadViewParts {
            scene,
            lighting,
            camera,
            cam_rel,
            aspect,
            adaptation,
            sky,
        } = p;
        let ground = lighting
            .fog_base
            .or(lighting.inside.map(|v| v.0.z))
            .map(|z| (z - scene.render_origin.z) as f32);
        adaptation.view_lamps = Some(view_lamp_light(
            &scene.lights,
            scene.render_origin,
            cam_rel,
            (camera.forward(), camera.right(), camera.up()),
            camera.fov_deg,
            aspect,
            ground,
        ));
        if crate::gpu::debug().debug_view_lamps {
            log::info!("view lamps: {:.6}", adaptation.view_lamps.unwrap_or(0.0));
        }
        // the night sky's glow from the lamps round the camera (the window's view leads), in
        // steps of a tenth: the sky is recomputed for a new value, not for every metre driven
        let raw = lamp_sky_glow(&scene.lights, scene.render_origin, cam_rel);
        let target = raw.clamp(0.03, 1.5);
        let step = (target.ln() * 10.0).round() / 10.0;
        let changed = sky.city_glow.is_none_or(|g| (g.ln() - step).abs() > 0.15);
        if changed {
            sky.city_glow = Some(step.exp());
        }
        if crate::gpu::debug().debug_sky && changed {
            log::info!(
                "sky glow from the lamps: {raw:.3} (taken {:.3})",
                step.exp()
            );
        }
    }
}
