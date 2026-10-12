//! The enhanced path's high-range colour targets of one size: the HDR chain, the glow's
//! levels and the bind groups of the passes between them.

use crate::effects::puddles;
use crate::renderer::{rt_gbuf, Renderer, AUX_FORMAT, GBUF_FORMAT, MASK_FORMAT};

/// High-range colour targets of the enhanced path for one size: the multisampled one the
/// scene is drawn into, the single-sampled one it resolves into, the glow's levels, the
/// tone-mapped picture FXAA reads, and the bind groups of the passes between them.
pub(crate) struct HdrTargets {
    pub(crate) msaa_view: Option<wgpu::TextureView>,
    pub(crate) view: wgpu::TextureView,
    /// The screen mask (`MASK_FORMAT`), multisampled and resolved like the picture.
    pub(crate) mask_msaa: Option<wgpu::TextureView>,
    pub(crate) mask: wgpu::TextureView,
    /// Enhanced+: the reflections' surfaces (`GBUF_FORMAT`, `AUX_FORMAT`): (multisampled,
    /// resolved) each.
    pub(crate) gbuf: Option<[(Option<wgpu::TextureView>, wgpu::TextureView); 2]>,
    /// Glow levels at 1/2, 1/4, ... of the size, and the upsampled sums per level.
    pub(crate) down: Vec<wgpu::TextureView>,
    pub(crate) up: Vec<wgpu::TextureView>,
    pub(crate) ldr: wgpu::TextureView,
    /// down[i] reads the picture (i = 0) or down[i - 1]; up[i] reads up[i + 1] (or the
    /// last down level) and down[i].
    pub(crate) down_bg: Vec<wgpu::BindGroup>,
    pub(crate) up_bg: Vec<wgpu::BindGroup>,
    pub(crate) meter_bg: wgpu::BindGroup,
    /// Tone mapping with the adapted exposure in `adapt[k]`.
    pub(crate) tonemap_bg: [wgpu::BindGroup; 2],
    pub(crate) fxaa_bg: wgpu::BindGroup,
    /// Classic shading is presented directly, without the Enhanced tone curve.
    pub(crate) classic_bg: wgpu::BindGroup,
    /// Allocated only when wet roads need scene reflections in the main view.
    pub(crate) puddles: Option<puddles::Targets>,
}

/// Glow levels of the enhanced post path.
pub(crate) const GLOW_LEVELS: usize = 6;

impl Renderer {
    /// The HDR colour targets of the enhanced path for this size, with the glow's levels,
    /// the tone-mapped picture FXAA reads and the post passes' bind groups.
    pub(crate) fn hdr_targets(&mut self, w: u32, h: u32) -> bool {
        self.target_use.insert((w, h), std::time::Instant::now());
        if self.hdr_targets.contains_key(&(w, h)) {
            return false;
        }
        self.evict_targets();
        let fmt = wgpu::TextureFormat::Rgba16Float;
        let target = |label: &str, tw: u32, th: u32, format: wgpu::TextureFormat, samples: u32| {
            let usage = if samples > 1 {
                wgpu::TextureUsages::RENDER_ATTACHMENT
            } else {
                wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC
            };
            self.device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: tw.max(1),
                        height: th.max(1),
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let msaa_view =
            (self.options.msaa > 1).then(|| target("hdr msaa", w, h, fmt, self.options.msaa));
        let view = target("hdr", w, h, fmt, 1);
        let mask_msaa = (self.options.msaa > 1)
            .then(|| target("screen mask msaa", w, h, MASK_FORMAT, self.options.msaa));
        let mask = target("screen mask", w, h, MASK_FORMAT, 1);
        let gbuf = rt_gbuf().then(|| {
            [GBUF_FORMAT, AUX_FORMAT].map(|f| {
                (
                    (self.options.msaa > 1)
                        .then(|| target("rt surfaces msaa", w, h, f, self.options.msaa)),
                    target("rt surfaces", w, h, f, 1),
                )
            })
        });
        // the glow: halving until the smallest level is a few dozen pixels across
        let levels = GLOW_LEVELS
            .min((w.min(h).max(16) as f32).log2() as usize - 3)
            .max(1);
        let down: Vec<wgpu::TextureView> = (1..=levels)
            .map(|k| target("glow down", w >> k, h >> k, fmt, 1))
            .collect();
        let up: Vec<wgpu::TextureView> = (1..=levels)
            .map(|k| target("glow up", w >> k, h >> k, fmt, 1))
            .collect();
        let ldr = target("tone mapped", w, h, wgpu::TextureFormat::Rgba8Unorm, 1);
        let bg_with = |src: &wgpu::TextureView,
                       base: &wgpu::TextureView,
                       adapt: &wgpu::TextureView,
                       screen_mask: &wgpu::TextureView| {
            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("post"),
                layout: &self.post_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.post_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(src),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.post_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(base),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(adapt),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::TextureView(screen_mask),
                    },
                ],
            })
        };
        let bg = |src: &wgpu::TextureView, base: &wgpu::TextureView, adapt: &wgpu::TextureView| {
            bg_with(src, base, adapt, &self.black_texture.view)
        };
        let none = &self.white_texture.view;
        // (the first level of the glow reads the screen mask as its `t_base`: no light from
        // the screens)
        let down_bg: Vec<wgpu::BindGroup> = (0..levels)
            .map(|i| {
                bg(
                    if i == 0 { &view } else { &down[i - 1] },
                    if i == 0 { &mask } else { none },
                    none,
                )
            })
            .collect();
        let up_bg: Vec<wgpu::BindGroup> = (0..levels)
            .map(|i| {
                bg(
                    if i + 1 == levels {
                        &down[i]
                    } else {
                        &up[i + 1]
                    },
                    &down[i],
                    none,
                )
            })
            .collect();
        let meter_bg = bg(&down[levels - 1], none, none);
        let tonemap_bg = [
            bg_with(&view, &up[0], &self.adapt_views[0], &mask),
            bg_with(&view, &up[0], &self.adapt_views[1], &mask),
        ];
        // (FXAA reads the screen mask as its `t_base` and leaves the screens as they are)
        let fxaa_bg = bg(&ldr, &mask, none);
        let classic_bg = self.picture_group(&view);
        self.hdr_targets.insert(
            (w, h),
            HdrTargets {
                msaa_view,
                view,
                mask_msaa,
                mask,
                gbuf,
                down,
                up,
                ldr,
                down_bg,
                up_bg,
                meter_bg,
                tonemap_bg,
                fxaa_bg,
                classic_bg,
                puddles: None,
            },
        );
        true
    }
}
