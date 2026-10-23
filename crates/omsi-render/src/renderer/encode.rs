//! A picture's passes recorded (`Renderer::encode_frame`), from the shadow maps to the
//! delivery to its output.

use super::frame::{DrawLists, FrameClock, FrameContext, FrameEncoders};
use super::state::Renderer;
use crate::gpu::HDR_FORMAT;
use crate::passes::record_bundles;
use crate::scene::Scene;

impl Renderer {
    /// The picture's passes, recorded into three encoders (and the main pass's parts): the
    /// shadow maps, the depth prepass with the ambient occlusion and the sky, and the picture
    /// itself up to its delivery to the output.
    pub(crate) fn encode_frame(
        &mut self,
        scene: &mut Scene,
        f: &FrameContext,
        clock: &mut FrameClock,
        lists: &DrawLists,
    ) -> FrameEncoders {
        let (target, camera, lighting) = (f.target, f.camera, f.lighting);
        let scene_view = f.scene_view();
        let with_overlays = f.with_overlays;
        let projection = f.projection;
        let ro = f.ro;
        let full_w = f.full_w;
        let full_h = f.full_h;
        let width = f.width;
        let height = f.height;
        let vanilla_fxaa = f.features.vanilla_fxaa;
        let glass_on = f.features.glass_on;
        let scaled = f.features.scaled;
        let aspect = f.aspect;
        let xr_view = f.features.xr_view;
        let lead_view = f.features.lead_view;
        let enhanced = f.features.enhanced;
        let puddles_wanted = f.features.puddles_wanted;
        let reflection_frame = f.features.reflection_frame;
        let masked_frame = f.features.masked_frame;
        let rt_frame = f.features.rt_frame;
        let ao_on = f.features.ao_on;
        let prepass_on = f.features.prepass_on;
        let dt = f.dt;
        let draw_shadows = f.draw_shadows;
        let redraw_near = f.redraw_near;
        let redraw_far = f.redraw_far;
        let vp_mat = f.vp_mat;
        let probe_redraw = f.probe_redraw;
        let scene_target = &f.scene_target;
        let overlays = &f.overlays;
        let lamp_shadows = &f.lamp_shadows;
        let cu = &f.cu;
        let list = &lists.list;
        let has_presurface = lists.has_presurface;
        let shadow_batches = &lists.shadow_batches;
        let prepass_batches = &lists.prepass_batches;
        let main_batches = &lists.main_batches;
        let cab_batches = &lists.cab_batches;
        let rain_batches = &lists.rain_batches;
        // OMSI_NO_BUNDLES=1 records the main pass directly, for comparison. (Splitting the
        // pass in two to finish the halves side by side was tried as well: the second half
        // has to load the first one's targets back into the GPU's tile memory, which cost
        // more GPU time than it saved on the CPU.)
        let main_bundles = if !omsi_cfg::flags::NO_BUNDLES.is_set() {
            let pp = self.main_pass(enhanced, reflection_frame);
            let format = if masked_frame {
                HDR_FORMAT
            } else {
                self.gpu.format
            };
            record_bundles(
                &self.device,
                self.gpu.encoding_pool.as_ref(),
                scene,
                &main_batches,
                pp,
                scene.camera_bind_group.as_ref().expect("camera bind group"),
                format,
                self.options.msaa,
            )
        } else {
            Vec::new()
        };
        clock.stage(self, "bundles", "mirror.bundles");
        // three command buffers, finished side by side (see below): the shadow maps, the
        // depth prepass with the ambient occlusion, and the picture itself
        let mut shadow_encoder =
            self.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("shadow maps"),
                });
        self.flush_pending_meshes(scene, &mut shadow_encoder);
        let mut prepass_encoder =
            self.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("depth prepass"),
                });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("picture"),
            });
        self.shadows.encode_shadow_maps(
            self.options.shadow_size,
            scene,
            &mut shadow_encoder,
            &shadow_batches,
            &lamp_shadows,
            draw_shadows,
            redraw_near,
            redraw_far,
            &clock.tset,
            &mut clock.timed,
        );
        self.ssao.encode_prepass_and_ao(
            &self.queue,
            &self.prepass,
            &self.targets,
            scene,
            &mut prepass_encoder,
            &prepass_batches,
            camera,
            projection,
            aspect,
            width,
            height,
            prepass_on,
            ao_on,
            &clock.tset,
            &mut clock.timed,
        );
        if rt_frame {
            self.encode_ray_tracing(
                &mut prepass_encoder,
                scene,
                clock.tset.as_ref(),
                &mut clock.timed,
            );
        }
        self.sky.encode_sky_cube(
            scene,
            &mut prepass_encoder,
            enhanced,
            lead_view,
            probe_redraw,
            redraw_near,
            self.instant_exposure,
            &clock.tset,
            &mut clock.timed,
        );
        self.sky.encode_probe(
            scene,
            &mut prepass_encoder,
            probe_redraw,
            &clock.tset,
            &mut clock.timed,
        );
        let main_parts = self.encode_main_pass(
            scene,
            &mut encoder,
            lighting,
            scene_view,
            &main_batches,
            &cab_batches,
            &prepass_batches,
            &main_bundles,
            &overlays,
            width,
            height,
            enhanced,
            reflection_frame,
            masked_frame,
            has_presurface,
            prepass_on,
            ao_on,
            puddles_wanted,
            scaled,
            with_overlays,
            xr_view,
            &clock.tset,
            &mut clock.timed,
        );
        let puddles_on = self.encode_reflections(
            scene,
            &mut encoder,
            camera,
            lighting,
            &cu,
            &list,
            &main_batches,
            &cab_batches,
            projection,
            ro,
            aspect,
            width,
            height,
            puddles_wanted,
            rt_frame,
            &clock.tset,
            &mut clock.timed,
        );
        // (the AO's fog target and the picture the fog is added onto: the puddles' when they
        // drew this frame)
        let fog = self
            .targets
            .ao
            .as_ref()
            .and_then(|ao| ao.fog_bg.as_ref().map(|bg| (&ao.fog_view, bg)));
        let fog_picture = self.targets.hdr_targets.get(&(width, height)).map(|h| {
            h.puddles
                .as_ref()
                .filter(|_| puddles_on)
                .map_or(&h.view, |p| &p.view)
        });
        self.fog_lamps.encode_fog_lamps(
            &self.queue,
            fog,
            fog_picture,
            scene,
            &mut encoder,
            lighting,
            vp_mat,
            width,
            height,
            enhanced,
            masked_frame,
            prepass_on,
            with_overlays,
            &clock.tset,
            &mut clock.timed,
        );
        self.encode_glass(
            scene,
            &mut encoder,
            scene_view,
            &rain_batches,
            width,
            height,
            enhanced,
            glass_on,
            masked_frame,
            puddles_on,
            reflection_frame,
        );
        self.encode_reflection_resolve(
            scene,
            &mut encoder,
            scene_view,
            &overlays,
            width,
            height,
            puddles_on,
            reflection_frame,
            scaled,
        );
        self.encode_enhanced_post(
            scene,
            &mut encoder,
            lighting,
            scene_view,
            &overlays,
            width,
            height,
            dt,
            enhanced,
            lead_view,
            puddles_on,
            rt_frame,
            scaled,
            with_overlays,
            &clock.tset,
            &mut clock.timed,
        );
        self.presenter.encode_present(
            &self.queue,
            scene,
            &mut encoder,
            &scene_target,
            target,
            &overlays,
            width,
            height,
            full_w,
            vanilla_fxaa,
            &clock.tset,
            &mut clock.timed,
        );
        // (the window's picture only: whoever holds the hook draws over it, at full size; a
        // triple screen's panels are not covered)
        if f.window_picture {
            if let Some(hook) = self.overlay_hook.as_mut() {
                hook(
                    &self.device,
                    &self.queue,
                    &mut encoder,
                    target,
                    (full_w, full_h),
                );
            }
        }
        clock.stage(self, "encode", "mirror.encode");
        FrameEncoders {
            shadow_encoder,
            prepass_encoder,
            encoder,
            main_parts,
        }
    }
}
