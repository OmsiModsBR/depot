//! The effects on the renderer: the ray tracing given the parts it reads.

use super::Renderer;
use crate::effects::rt::RtParts;
use crate::lighting::Lighting;
use crate::scene::{Camera, Scene};
use glam::Mat4;

impl Renderer {
    /// What the ray tracing reads from the rest of the renderer.
    fn rt_parts(&self) -> RtParts<'_> {
        RtParts {
            device: &self.device,
            queue: &self.queue,
            assets: &self.assets,
            enh_buf: &self.enh_buf,
            samplers: &self.samplers,
            sky: &self.sky,
            targets: &self.targets,
            shadow_casters: self.options.omsi_shadow_casters,
        }
    }

    /// The frame's ray-tracing input (`RayTracer::prepare`). False when there is nothing to
    /// trace with.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare_ray_tracing(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        lighting: &Lighting,
        view_proj: Mat4,
        proj: Mat4,
        w: u32,
        h: u32,
        dt: f32,
    ) -> bool {
        let Some(mut rt) = self.rt.take() else {
            return false;
        };
        let ok = rt.prepare(
            &self.rt_parts(),
            scene,
            camera,
            lighting,
            view_proj,
            proj,
            w,
            h,
            dt,
        );
        self.rt = Some(rt);
        ok
    }

    /// This frame's structures and the traced lighting (`RayTracer::encode`).
    pub(crate) fn encode_ray_tracing(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        scene: &Scene,
        queries: Option<&wgpu::QuerySet>,
        timed: &mut Vec<&'static str>,
    ) {
        if let Some(mut rt) = self.rt.take() {
            rt.encode(&self.rt_parts(), encoder, scene, queries, timed);
            self.rt = Some(rt);
        }
    }

    /// The ray-traced reflections of the window's picture (`RayTracer::encode_reflections`).
    pub(crate) fn encode_rt_reflections(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        w: u32,
        h: u32,
        queries: Option<&wgpu::QuerySet>,
        timed: &mut Vec<&'static str>,
    ) {
        if let Some(mut rt) = self.rt.take() {
            rt.encode_reflections(&self.rt_parts(), encoder, w, h, queries, timed);
            self.rt = Some(rt);
        }
    }
}
