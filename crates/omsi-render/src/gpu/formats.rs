//! The formats of the render targets every layer draws into, and the colour targets of a
//! scene pipeline drawing into them.

pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Samples per pixel of the scene passes when nothing else is asked for (the shadow maps
/// stay single-sampled). Four samples take the staircase off every edge; the multisampled
/// colour target resolves into the window or the mirror texture at the end of the main pass.
pub const MSAA: u32 = 4;

/// The enhanced pass's second target: r is 1 where the bus's own screens are
/// (`MaterialExtra::screen`), 0 elsewhere - the glow takes no light from them and FXAA
/// passes them through; g is 1 on an LED panel's own dots (`MaterialExtra::led`), which the
/// glow's source keeps and multiplies up (see `post.wgsl`). b carries the puddle's
/// reflected-light weight; a is blend coverage. Sharing this attachment avoids another
/// geometry pass or a full normal/material buffer just for water.
pub(crate) const MASK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// Enhanced+: the enhanced pass's third and fourth targets, what the ray-traced reflections
/// start from (rt.wgsl): the surface's normal (xyz) and how much it reflects (w), and its
/// view depth and roughness (rg) - all weighted by that reflection (premultiplied), so that
/// a pane blended over a wet road leaves the stronger reflection's surface in them.
pub(crate) const GBUF_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

pub(crate) const AUX_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg16Float;

/// The renderer is built for Enhanced+: its HDR pipelines and targets have the two above.
pub(crate) static RT_GBUF: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub(crate) fn rt_gbuf() -> bool {
    RT_GBUF.load(std::sync::atomic::Ordering::Relaxed)
}

pub(crate) const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// The colour targets of a pipeline drawing into `format`: Enhanced and classic puddle
/// shading draw into `HDR_FORMAT` with the screen mask beside it,
/// written by the scene's own shader only (`mask`), coverage-blended where the colour is.
pub(crate) fn color_targets(
    format: wgpu::TextureFormat,
    blend: Option<wgpu::BlendState>,
    write: wgpu::ColorWrites,
    mask: bool,
    gbuf: bool,
) -> Vec<Option<wgpu::ColorTargetState>> {
    let mut v = vec![Some(wgpu::ColorTargetState {
        format,
        blend,
        write_mask: write,
    })];
    if format == HDR_FORMAT {
        v.push(Some(wgpu::ColorTargetState {
            format: MASK_FORMAT,
            blend: blend.map(|_| wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent::REPLACE,
            }),
            write_mask: if mask {
                wgpu::ColorWrites::ALL
            } else {
                wgpu::ColorWrites::empty()
            },
        }));
        if rt_gbuf() {
            // (premultiplied by the reflection's weight, the shader's alpha)
            let over = blend.map(|_| wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
            });
            let write = if gbuf {
                wgpu::ColorWrites::ALL
            } else {
                wgpu::ColorWrites::empty()
            };
            v.push(Some(wgpu::ColorTargetState {
                format: GBUF_FORMAT,
                blend: over,
                write_mask: write,
            }));
            v.push(Some(wgpu::ColorTargetState {
                format: AUX_FORMAT,
                blend: over,
                write_mask: write,
            }));
        }
    }
    v
}
