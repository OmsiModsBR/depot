//! Enhanced: its own lighting in high range, with the lamps' glow on the night sky, metering
//! and tone mapping. Mirrors are shaded plainly unless asked otherwise.

mod fog_lamps;
mod lights;
mod pipelines;
mod post;
mod scene_shader;

use crate::effects::puddles::PuddleLook;
use crate::interface::{
    FogCull, FogLampsParts, LeadViewParts, ModeResources, ModeShading, ModeView, PipeSet,
    PostParts, RenderMode,
};
use crate::lighting::{lamp_sky_glow, view_lamp_light, GpuPointLight, Lighting, PointLight};
use crate::passes::{Exposure, FrameFeatures, PassPipelines, PathBuilders};
use crate::scene::Camera;
use crate::sky::{atmosphere, SkyResources};
use crate::targets::GroupParts;
use glam::Vec3;

pub(crate) use fog_lamps::{fog_lamps_pipelines, FogLampsPass};
#[cfg(test)]
pub(crate) use fog_lamps::{fog_lamps_shader_source, FogLampUniform};
pub(crate) use scene_shader::{corona_text, scene_text, sky_text, snow_text};

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

    /// Everything the mode draws with, built from the core's pass builders.
    pub(crate) fn build(
        builders: &PathBuilders<'_>,
        device: &wgpu::Device,
        camera_layout: &wgpu::BindGroupLayout,
        swap_format: wgpu::TextureFormat,
        hdr_format: wgpu::TextureFormat,
        gl: bool,
    ) -> Self {
        Self::new(
            pipelines::main_pass(builders, hdr_format),
            pipelines::sky_mirror(builders, swap_format),
            fog_lamps_pipelines(device, camera_layout, gl, hdr_format),
        )
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

    fn encode_post(&self, parts: PostParts<'_>) {
        post::encode_post(parts);
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
        if features.own_shading {
            PipeSet::Own
        } else if features.reflection_frame {
            PipeSet::Reflections
        } else {
            PipeSet::Plain
        }
    }

    fn draws_light(&self, l: &PointLight, _features: &FrameFeatures) -> bool {
        lights::draws(l)
    }

    fn light_record(&self, l: &PointLight, p: Vec3, _features: &FrameFeatures) -> GpuPointLight {
        lights::record(l, p)
    }

    fn lamp_shadows(&self, features: &FrameFeatures) -> bool {
        features.own_shading
    }

    fn lead_view(&self, features: &FrameFeatures, p: LeadViewParts<'_>) {
        // (a mirror takes the window's light - its own call would move the exposure on -
        // unless it comes before the window's first frame)
        if !(features.own_shading && features.lead_view) {
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

    fn fog_cull(&self, features: &FrameFeatures, lighting: &Lighting, camera: &Camera) -> FogCull {
        // (a mirror drawn plainly has the plain fog)
        if !features.own_shading {
            return FogCull::plain(lighting, camera);
        }
        // The enhanced picture's fog is another one: none at all below the weather's 1e-4
        // (the clear air there is the sky model's, kilometres deep), and above it a layer that
        // thins out with height (`layer_depth` in enhanced_common.wgsl, 300 m scale height over
        // the fog's base). Culled by the vanilla density, whatever stood in thin fog went
        // missing in plain sight - most of all seen from above, with the camera zoomed out.
        let far = if lighting.fog_density > 1e-4 {
            let base = lighting
                .fog_base
                .or(lighting.inside.map(|v| v.0.z))
                .unwrap_or(camera.position.z - 2.0);
            let kh = ((camera.position.z - base).max(0.0) / 300.0) as f32;
            // a point on the ground seen from the camera's height: the thinnest fog a line of
            // sight down to the scenery passes through
            let thin = if kh < 1e-3 {
                1.0
            } else {
                (1.0 - (-kh).exp()) / kh
            };
            (4.6 / (lighting.fog_density * thin)).min(camera.far)
        } else {
            camera.far
        };
        FogCull { far, own: true }
    }

    fn puddle_look(
        &self,
        features: &FrameFeatures,
        lighting: &Lighting,
        sky: &SkyResources,
        adaptation: &Exposure,
    ) -> Option<PuddleLook> {
        // (a mirror drawn plainly is lit plainly)
        if !features.own_shading {
            return Some(PuddleLook::plain(lighting));
        }
        let st = sky.sky_state.as_ref()?;
        let surround = [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z]
            .into_iter()
            .map(|n| atmosphere::sh_irradiance(&st.sh, n))
            .sum::<Vec3>()
            * (0.25 / (6.0 * std::f32::consts::PI));
        Some(PuddleLook {
            surround,
            exposure: adaptation.exposure.unwrap_or(0.0).exp(),
            own: true,
        })
    }
}
