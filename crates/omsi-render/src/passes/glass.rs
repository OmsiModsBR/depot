//! The glass pass: what lies behind the panes, and drawing the glass over it.

use super::draw::{encode_batches, Batch};
use crate::assets::GpuTexture;
use crate::renderer::Renderer;
use crate::scene::{Scene, TextureId};

impl Renderer {
    /// The rain films on the glass: a copy of the clean picture behind them, then the panes'
    /// drops drawn over it.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn encode_glass(
        &mut self,
        scene: &Scene,
        encoder: &mut wgpu::CommandEncoder,
        scene_view: &wgpu::TextureView,
        rain_batches: &[Batch],
        width: u32,
        height: u32,
        enhanced: bool,
        glass_on: bool,
        masked_frame: bool,
        puddles_on: bool,
        reflection_frame: bool,
    ) {
        if glass_on {
            let hdr = masked_frame.then(|| &self.targets.hdr_targets[&(width, height)]);
            let view = hdr.map_or(scene_view, |h| {
                h.puddles
                    .as_ref()
                    .filter(|_| puddles_on)
                    .map_or(&h.view, |p| &p.view)
            });
            let behind = self.targets.glass_picture.as_ref().unwrap();
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
            let pipes = &self.main_pass(enhanced, reflection_frame).rain_pipelines;
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
                    view: &self.targets.ao.as_ref().unwrap().depth_view,
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

    /// The scene's slot for the picture behind the glass (black until a frame is drawn).
    pub(crate) fn glass_slot(&self, scene: &mut Scene) -> TextureId {
        if let Some(id) = scene.glass_slot {
            return id;
        }
        scene.textures.push(GpuTexture::showing(
            self.assets.black_texture.texture.clone(),
            self.assets.black_texture.view.clone(),
            (1, 1),
        ));
        let id = scene.textures.len() - 1;
        scene.glass_slot = Some(id);
        id
    }

    /// Refraction reads a copy made before drawing films, avoiding rain/wiper feedback.
    pub(crate) fn prepare_glass_behind(
        &mut self,
        scene: &mut Scene,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) {
        let Some(id) = scene.glass_slot else { return };
        if self.targets.glass_picture.as_ref().is_none_or(|v| {
            v.texture().width() != width
                || v.texture().height() != height
                || v.texture().format() != format
        }) {
            let tex = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("picture behind glass"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            self.targets.glass_picture = Some(tex.create_view(&Default::default()));
        }
        let view = self.targets.glass_picture.as_ref().unwrap();
        if scene.textures[id].view != *view {
            scene.textures[id] =
                GpuTexture::showing(view.texture().clone(), view.clone(), (width, height));
            self.rebind_textures(scene, &[id]);
        }
    }
}
