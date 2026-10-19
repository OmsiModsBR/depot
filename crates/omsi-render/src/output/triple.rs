//! A frame delivered to a triple screen: the three panels, the culling each keeps, their
//! targets and the composite of the panels and the HUD into the window.

use super::window::Presenter;
use crate::gpu::{buffer_init, target_2d};
use crate::scene::{CullHistory, ScreenView, TextureId};

/// Swap in a panel's own culling history (and the last one's back out).
pub(crate) fn swap_triple_culling(culling: &mut CullHistory, panel: usize) {
    // Visibility and LOD hysteresis belong to one projection. A side panel
    // must not replace the previous centre view's history each frame.
    let history = &mut culling.triple_culling[panel];
    std::mem::swap(culling.cull_drawn.get_mut(), &mut history.drawn);
    std::mem::swap(culling.object_sizes.get_mut(), &mut history.sizes);
    std::mem::swap(culling.object_sizes_scratch.get_mut(), &mut history.scratch);
}

impl Presenter {
    /// The panels' targets and their composite bind groups, made anew when the window's
    /// size changes (all three panels are `pw` by `height`).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn ensure_triple_targets(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        sampler: &wgpu::Sampler,
        views: &[ScreenView; 3],
        pw: u32,
        width: u32,
        height: u32,
    ) {
        if self.triple_targets.as_ref().map(|t| t.0) != Some((width, height)) {
            let targets = views
                .iter()
                .map(|v| {
                    let w = pw;
                    let texture = target_2d(
                        device,
                        "triple screen panel",
                        wgpu::Extent3d {
                            width: w,
                            height,
                            depth_or_array_layers: 1,
                        },
                        1,
                        format,
                        wgpu::TextureUsages::RENDER_ATTACHMENT
                            | wgpu::TextureUsages::TEXTURE_BINDING,
                    );
                    let view = texture.create_view(&Default::default());
                    // (z: where the panel starts in the window, for `fs_panel`)
                    let params = [w as f32, height as f32, v.viewport[0] as f32, 0.0];
                    let buf = buffer_init(
                        &device,
                        &queue,
                        Some("triple screen composite"),
                        bytemuck::cast_slice(&params),
                        wgpu::BufferUsages::UNIFORM,
                    );
                    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
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
                                resource: wgpu::BindingResource::Sampler(sampler),
                            },
                        ],
                    });
                    (view, group)
                })
                .collect();
            self.triple_targets = Some(((width, height), targets));
        }
    }

    /// The panels copied to their places in `target`, and the HUD drawn once over the whole
    /// window.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn composite_triple(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        views: &[ScreenView; 3],
        overlay_res: &[(TextureId, wgpu::Buffer, wgpu::BindGroup, [f32; 8])],
        width: u32,
        height: u32,
    ) {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
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
            for (_, _, group, _) in overlay_res {
                pass.set_bind_group(0, group, &[]);
                pass.draw(0..6, 0..1);
            }
        }
        queue.submit(Some(encoder.finish()));
    }
}
