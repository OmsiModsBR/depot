//! The render targets on the renderer: the target cache asked for a size with the parts it
//! builds its bind groups from.

use crate::renderer::Renderer;
use crate::targets::GroupParts;

impl Renderer {
    /// The smaller colour target of the render scale for this size, with its bind group.
    pub(crate) fn scale_target(&mut self, w: u32, h: u32) -> (wgpu::TextureView, wgpu::BindGroup) {
        let (presenter, device, sampler) = (&self.presenter, &self.device, &self.post.post_sampler);
        self.targets.scale_target(
            &self.device,
            self.gpu.format,
            |v| presenter.picture_group(device, sampler, v),
            w,
            h,
        )
    }

    /// The HDR colour targets of the enhanced path for this size.
    pub(crate) fn hdr_targets(&mut self, w: u32, h: u32) -> bool {
        let (presenter, device, sampler) = (&self.presenter, &self.device, &self.post.post_sampler);
        self.targets.hdr_targets(
            &self.device,
            self.options.msaa,
            GroupParts {
                layout: &self.post.post_layout,
                buf: &self.post.post_buf,
            },
            &self.post.post_sampler,
            &self.assets,
            &self.adaptation.adapt_views,
            |v| presenter.picture_group(device, sampler, v),
            w,
            h,
        )
    }

    /// The ambient-occlusion textures for a target of this size (rebuilt on resize).
    pub(crate) fn ensure_ao(&mut self, w: u32, h: u32) -> bool {
        self.targets.ensure_ao(
            &self.device,
            GroupParts {
                layout: &self.ssao.ao_layout,
                buf: &self.ssao.ao_buf,
            },
            self.fog_lamps
                .fog_lamps_pipeline
                .is_some()
                .then_some(GroupParts {
                    layout: &self.fog_lamps.fog_lamps_layout,
                    buf: &self.fog_lamps.fog_lamps_buf,
                }),
            self.puddles.is_some(),
            w,
            h,
        )
    }
}
