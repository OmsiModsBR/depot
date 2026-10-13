//! The glass pass: what lies behind the panes, and drawing the glass over it.

use super::draw::{encode_batches, Batch};
use crate::scene::Scene;
use crate::targets::TargetCache;

/// The rain films on the glass: a copy of the clean picture behind them, then the panes'
/// drops drawn over it. `rain_pipelines`: the main pass's for this picture.
#[allow(clippy::too_many_arguments)]
pub(crate) fn encode_glass(
    targets: &TargetCache,
    rain_pipelines: &[wgpu::RenderPipeline],
    scene: &Scene,
    encoder: &mut wgpu::CommandEncoder,
    scene_view: &wgpu::TextureView,
    rain_batches: &[Batch],
    width: u32,
    height: u32,
    glass_on: bool,
    masked_frame: bool,
    puddles_on: bool,
) {
    if glass_on {
        let hdr = masked_frame.then(|| &targets.hdr_targets[&(width, height)]);
        let view = hdr.map_or(scene_view, |h| {
            h.puddles
                .as_ref()
                .filter(|_| puddles_on)
                .map_or(&h.view, |p| &p.view)
        });
        let behind = targets.glass_picture.as_ref().unwrap();
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: view.texture(),
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: behind.texture(),
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let colours = [
            Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            }),
            hdr.map(|h| wgpu::RenderPassColorAttachment {
                view: &h.mask,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            }),
            hdr.and_then(|h| h.gbuf.as_ref())
                .map(|g| wgpu::RenderPassColorAttachment {
                    view: &g[0].1,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                }),
            hdr.and_then(|h| h.gbuf.as_ref())
                .map(|g| wgpu::RenderPassColorAttachment {
                    view: &g[1].1,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                }),
        ];
        let pipes = rain_pipelines;
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("rain on current scene"),
            color_attachments: &colours[..if colours[2].is_some() {
                4
            } else if hdr.is_some() {
                2
            } else {
                1
            }],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &targets.ao.as_ref().unwrap().depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_bind_group(0, scene.camera_bind_group.as_ref().unwrap(), &[]);
        encode_batches(&mut pass, scene, &rain_batches, |pipe| {
            &pipes[pipe as usize]
        });
    }
}
