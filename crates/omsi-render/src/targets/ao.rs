//! The ambient-occlusion pass's textures for one target size, and the lamps' light in the
//! fog worked out beside them.

use crate::renderer::{Renderer, DEPTH_FORMAT};

/// The textures of the ambient-occlusion pass for one target size.
pub(crate) struct AoTargets {
    pub(crate) size: (u32, u32),
    pub(crate) depth_view: wgpu::TextureView,
    pub(crate) ao_view: wgpu::TextureView,
    pub(crate) blur_view: wgpu::TextureView,
    pub(crate) ssao_bg: wgpu::BindGroup,
    pub(crate) blur_bg: wgpu::BindGroup,
    /// The lamps' light in the fog (`fog_lamps.wgsl`): worked out at half size into
    /// `fog_view`, then added onto the picture (bind groups of the two passes).
    pub(crate) fog_view: wgpu::TextureView,
    pub(crate) fog_bg: Option<[wgpu::BindGroup; 2]>,
}

impl Renderer {
    /// The ambient-occlusion textures for a target of this size (rebuilt on resize).
    pub(crate) fn ensure_ao(&mut self, w: u32, h: u32) -> bool {
        if self.ao.as_ref().map(|a| a.size == (w, h)).unwrap_or(false) {
            return false;
        }
        let size = wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        };
        let depth = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("prepass depth"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | if self.puddles.is_some() {
                    wgpu::TextureUsages::COPY_SRC
                } else {
                    wgpu::TextureUsages::empty()
                },
            view_formats: &[],
        });
        // the AO itself at half size: four times fewer pixels, and the blur hides the rest
        let half = wgpu::Extent3d {
            width: w.div_ceil(2),
            height: h.div_ceil(2),
            depth_or_array_layers: 1,
        };
        let mk = |label: &str| {
            self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: half,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                // r the occlusion, g the view depth (m) it was worked out at: the main
                // pass upsamples it by depth, not bilinearly (see `ao_at` in shader.wgsl)
                format: wgpu::TextureFormat::Rg16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
        };
        let ao = mk("ssao");
        let blur = mk("ssao blur");
        let depth_view = depth.create_view(&Default::default());
        let ao_view = ao.create_view(&Default::default());
        let blur_view = blur.create_view(&Default::default());

        let bg = |label: &str, tex: &wgpu::TextureView| {
            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &self.ao_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.ao_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&depth_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(tex),
                    },
                ],
            })
        };
        // the SSAO pass reads no AO texture, but the layout wants one: bind the blur target
        let ssao_bg = bg("ssao", &blur_view);
        let blur_bg = bg("ssao blur", &ao_view);
        let fog_view = self
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("fog lamps"),
                // (one row more: the sun's visibility, see fog_lamps.wgsl)
                size: wgpu::Extent3d {
                    height: half.height + 1,
                    ..half
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let fog_bg = self.fog_lamps_pipeline.is_some().then(|| {
            // (the half-size pass reads no result: the layout wants one, the AO's is bound)
            [&ao_view, &fog_view].map(|read| {
                self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("fog lamps"),
                    layout: &self.fog_lamps_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: self.fog_lamps_buf.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&depth_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(read),
                        },
                    ],
                })
            })
        });
        self.ao = Some(AoTargets {
            size: (w, h),
            depth_view,
            ao_view,
            blur_view,
            ssao_bg,
            blur_bg,
            fog_view,
            fog_bg,
        });
        true
    }
}
