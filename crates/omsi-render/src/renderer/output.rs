//! The frame's destinations on the renderer: an image rendered off screen and read back,
//! the triple screen's panels, the HUD overlays and the GPU timers.

use crate::lighting::Lighting;
use crate::output::{read_texture, swap_triple_culling, Output};
use crate::renderer::Renderer;
use crate::scene::{panel_width, Camera, Scene, TripleScreen};
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

    /// Three independently culled and shaded panels, composited into a spanning window.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_triple_frame(
        &mut self,
        scene: &mut Scene,
        target: &wgpu::TextureView,
        width: u32,
        height: u32,
        camera: &Camera,
        lighting: &Lighting,
        rig: &TripleScreen,
    ) {
        if width < 3 || height == 0 || self.device_lost().is_some() {
            return;
        }
        let views = rig.views(camera, width, height);
        // all three panels have the same size: they share the size-keyed targets
        let pw = panel_width(width);
        self.presenter.ensure_triple_targets(
            &self.device,
            &self.queue,
            self.gpu.format,
            &self.sky.sky_sampler,
            &views,
            pw,
            width,
            height,
        );
        // the HUD is drawn once over the whole window, after the panels: they draw none
        // (its rect buffers are kept aside, not rebuilt each frame)
        let overlays = std::mem::take(&mut scene.overlays);
        let overlay_res = std::mem::take(&mut scene.overlay_res);
        let env_heading = self.env_heading.replace(Some(camera.yaw));
        // Centre meters exposure and updates shared lighting once. All panels
        // then use that same exposure and sun shadow atlas.
        for (turn, i) in [1, 0, 2].into_iter().enumerate() {
            let view = self.presenter.triple_targets.as_ref().unwrap().1[i]
                .0
                .clone();
            self.swap_triple_culling(i);
            self.render_inner(
                scene,
                &view,
                pw,
                height,
                &views[i].camera,
                lighting,
                Output::TriplePanel {
                    projection: views[i].projection,
                    later: turn != 0,
                },
            );
            self.swap_triple_culling(i);
        }
        self.env_heading.set(env_heading);
        scene.overlays = overlays;
        scene.overlay_res = overlay_res;
        self.prepare_overlays(scene, width, height);
        self.presenter.composite_triple(
            &self.device,
            &self.queue,
            target,
            &views,
            &scene.overlay_res,
            width,
            height,
        );
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
