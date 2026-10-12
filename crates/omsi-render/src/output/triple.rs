//! A frame delivered to a triple screen: the three panels and the culling each keeps.

use crate::gpu::buffer_init;
use crate::lighting::Lighting;
use crate::renderer::Renderer;
use crate::scene::{panel_width, Camera, Scene, TripleScreen};

impl Renderer {
    /// Three independently culled and shaded physical panels, composited into a
    /// spanning window. HUD and menu remain in window pixels and are drawn once.
    pub fn render_triple(
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
        if self.triple_targets.as_ref().map(|t| t.0) != Some((width, height)) {
            let targets = views
                .iter()
                .map(|v| {
                    let w = pw;
                    let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("triple screen panel"),
                        size: wgpu::Extent3d {
                            width: w,
                            height,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: self.format,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                            | wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    });
                    let view = texture.create_view(&Default::default());
                    // (z: where the panel starts in the window, for `fs_panel`)
                    let params = [w as f32, height as f32, v.viewport[0] as f32, 0.0];
                    let buf = buffer_init(
                        &self.device,
                        &self.queue,
                        Some("triple screen composite"),
                        bytemuck::cast_slice(&params),
                        wgpu::BufferUsages::UNIFORM,
                    );
                    let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("triple screen composite"),
                        layout: &self.upscale_layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: buf.as_entire_binding(),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::TextureView(&view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 2,
                                resource: wgpu::BindingResource::Sampler(&self.sky_sampler),
                            },
                        ],
                    });
                    (view, group)
                })
                .collect();
            self.triple_targets = Some(((width, height), targets));
        }
        // the HUD is drawn once over the whole window, after the panels: they draw none
        // (its rect buffers are kept aside, not rebuilt each frame)
        let overlays = std::mem::take(&mut scene.overlays);
        let overlay_res = std::mem::take(&mut scene.overlay_res);
        let env_heading = self.env_heading.replace(Some(camera.yaw));
        // Centre meters exposure and updates shared lighting once. All panels
        // then use that same exposure and sun shadow atlas.
        for (turn, i) in [1, 0, 2].into_iter().enumerate() {
            let view = self.triple_targets.as_ref().unwrap().1[i].0.clone();
            self.swap_triple_culling(i);
            self.render_inner(
                scene,
                &view,
                pw,
                height,
                &views[i].camera,
                lighting,
                true,
                None,
                Some(views[i].projection),
                turn != 0,
            );
            self.swap_triple_culling(i);
        }
        self.env_heading.set(env_heading);
        scene.overlays = overlays;
        scene.overlay_res = overlay_res;
        self.prepare_overlays(scene, width, height);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("triple screen composite"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("triple screen composite"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
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
            pass.set_pipeline(&self.panel_pipeline);
            for (i, v) in views.iter().enumerate() {
                // (the right panel loses its last pixel or two at the window's edge)
                let [x, y, w, h] = v.viewport;
                pass.set_scissor_rect(x, y, w.min(width - x), h);
                pass.set_bind_group(0, &self.triple_targets.as_ref().unwrap().1[i].1, &[]);
                pass.draw(0..3, 0..1);
            }
            pass.set_scissor_rect(0, 0, width, height);
            pass.set_pipeline(&self.overlay_pipeline_1x);
            for (_, _, group, _) in &scene.overlay_res {
                pass.set_bind_group(0, group, &[]);
                pass.draw(0..6, 0..1);
            }
        }
        self.queue.submit(Some(encoder.finish()));
    }

    pub(crate) fn swap_triple_culling(&mut self, panel: usize) {
        // Visibility and LOD hysteresis belong to one projection. A side panel
        // must not replace the previous centre view's history each frame.
        let history = &mut self.triple_culling[panel];
        std::mem::swap(self.cull_drawn.get_mut(), &mut history.drawn);
        std::mem::swap(self.object_sizes.get_mut(), &mut history.sizes);
        std::mem::swap(self.object_sizes_scratch.get_mut(), &mut history.scratch);
    }
}
