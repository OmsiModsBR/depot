//! The reflections (Enhanced+): resolving the surfaces and tracing what they reflect.

use super::draw::Batch;
use crate::lighting::Lighting;
use crate::renderer::Renderer;
use crate::scene::{Camera, CameraUniform, Scene, TextureId};
use glam::{DVec3, Mat4};

impl Renderer {
    /// Vanilla with reflections: the high-range picture brought into the target, the overlays
    /// on top when the picture is not scaled.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn encode_reflection_resolve(
        &self,
        scene: &Scene,
        encoder: &mut wgpu::CommandEncoder,
        scene_view: &wgpu::TextureView,
        overlays: &[(TextureId, [f32; 4])],
        width: u32,
        height: u32,
        puddles_on: bool,
        reflection_frame: bool,
        scaled: bool,
    ) {
        if reflection_frame {
            let h = &self.hdr_targets[&(width, height)];
            let bg = h
                .puddles
                .as_ref()
                .filter(|_| puddles_on)
                .map_or(&h.classic_bg, |p| &p.classic_bg);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("classic reflections present"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: scene_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.copy_pipeline);
            pass.set_bind_group(0, bg, &[]);
            pass.draw(0..3, 0..1);
            if !overlays.is_empty() && !scaled {
                pass.set_pipeline(&self.overlay_pipeline_1x);
                for (k, _) in overlays.iter().enumerate() {
                    if let Some((_, _, bg, _)) = scene.overlay_res.get(k) {
                        pass.set_bind_group(0, bg, &[]);
                        pass.draw(0..6, 0..1);
                    }
                }
            }
        }
    }

    /// The reflections: Enhanced+'s traced ones, or the puddles' (whether they are drawn is
    /// returned).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn encode_reflections(
        &mut self,
        scene: &Scene,
        encoder: &mut wgpu::CommandEncoder,
        camera: &Camera,
        lighting: &Lighting,
        cu: &CameraUniform,
        list: &[u32],
        main_batches: &[Batch],
        cab_batches: &[Batch],
        projection: Option<Mat4>,
        ro: DVec3,
        aspect: f32,
        width: u32,
        height: u32,
        puddles_wanted: bool,
        rt_frame: bool,
        tset: &Option<wgpu::QuerySet>,
        timed: &mut Vec<&'static str>,
    ) -> bool {
        // Weather alone is insufficient: leave the allocation and reflection passes out when
        // the visible batches contain no moisture-tagged surface (a showroom, bare terrain).
        // Enhanced+: the traced reflections (wet roads' too) in place of the puddles' rays
        if rt_frame {
            self.encode_rt_reflections(encoder, width, height, tset.as_ref(), timed);
        }
        let puddles_on = puddles_wanted
            && !rt_frame
            && main_batches
                .iter()
                .any(|b| scene.materials[b.material as usize].uniform.params2[2] > 0.0)
            && self.prepare_puddle_reflections(
                width, height, camera, aspect, projection, &cu, lighting, ro,
            );
        if puddles_on {
            // (the vehicle the camera is in is drawn into the puddles' picture as well)
            let all: Vec<Batch>;
            let batches = if cab_batches.is_empty() {
                main_batches
            } else {
                all = main_batches.iter().chain(cab_batches).cloned().collect();
                &all
            };
            self.encode_puddle_reflections(
                encoder,
                width,
                height,
                scene,
                batches,
                &list,
                lighting,
                camera,
                tset.as_ref(),
                timed,
            );
        }
        puddles_on
    }
}
