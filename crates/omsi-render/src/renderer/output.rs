//! The frame's destinations on the renderer: an image rendered off screen and read back,
//! the triple screen's panels, the HUD overlays and the GPU timers.

use crate::lighting::Lighting;
use crate::output::{read_texture, swap_triple_culling};
use crate::renderer::Renderer;
use crate::scene::{Camera, Scene, TripleScreen};
use anyhow::Result;

impl Renderer {
    pub(crate) fn render_image(
        &mut self,
        scene: &mut Scene,
        width: u32,
        height: u32,
        camera: &Camera,
        lighting: &Lighting,
        rig: Option<&TripleScreen>,
    ) -> Result<Vec<u8>> {
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.gpu.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        // a picture on its own: the enhanced exposure is where the light puts it at once
        self.instant_exposure = true;
        if let Some(rig) = rig {
            self.render_triple(scene, &view, width, height, camera, lighting, rig);
        } else {
            self.render(scene, &view, width, height, camera, lighting);
        }
        self.instant_exposure = false;
        let mut out = self.read_texture(&tex, wgpu::TextureAspect::All)?;
        // BGRA surfaces → swap
        if matches!(
            self.gpu.format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        ) {
            for px in out.chunks_exact_mut(4) {
                px.swap(0, 2);
            }
        }
        Ok(out)
    }

    /// A texture's pixels read back, the GPU waited for.
    pub(crate) fn read_texture(
        &self,
        texture: &wgpu::Texture,
        aspect: wgpu::TextureAspect,
    ) -> Result<Vec<u8>> {
        read_texture(&self.device, &self.queue, texture, aspect)
    }

    /// Swap in a triple screen panel's own culling history.
    pub(crate) fn swap_triple_culling(&mut self, panel: usize) {
        swap_triple_culling(&mut self.culling, panel);
    }

    /// The HUD overlays' rect buffers and bind groups for this frame.
    pub(crate) fn prepare_overlays(&self, scene: &mut Scene, full_w: u32, full_h: u32) {
        self.presenter.prepare_overlays(
            &self.device,
            &self.queue,
            &self.sky.sky_sampler,
            scene,
            full_w,
            full_h,
        );
    }

    /// Take in the pass times of the last timed frame once its readback has arrived.
    pub(crate) fn collect_gpu_timers(&mut self) {
        self.diagnostics
            .collect_gpu_timers(&self.device, &self.queue);
    }
}
