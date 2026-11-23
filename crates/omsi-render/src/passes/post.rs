//! The post pipelines and the eye's adaptation; the post passes themselves are Enhanced's
//! (`modes/enhanced/post.rs`).

use crate::assets::GpuTexture;
use crate::gpu::{float_tex, fullscreen_pipeline, sampler_entry, target_2d, uniform_entry};
use crate::shaders::post_shader_source;

/// The enhanced path's post passes (post.wgsl `PostParams`).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct PostUniform {
    pub(crate) a: [f32; 4],
    pub(crate) b: [f32; 4],
    pub(crate) c: [f32; 4],
    pub(crate) d: [f32; 4],
}

/// The enhanced path's post pipelines (post.wgsl).
pub(crate) struct PostPipelines {
    pub(crate) down_first: wgpu::RenderPipeline,
    pub(crate) down: wgpu::RenderPipeline,
    pub(crate) up: wgpu::RenderPipeline,
    pub(crate) meter: wgpu::RenderPipeline,
    pub(crate) adapt: wgpu::RenderPipeline,
    /// Straight into the target, or gamma-encoded for FXAA.
    pub(crate) tonemap: wgpu::RenderPipeline,
    pub(crate) tonemap_encoded: wgpu::RenderPipeline,
    pub(crate) fxaa: wgpu::RenderPipeline,
    /// Their bind group layout, uniform buffer and sampler.
    pub(crate) post_layout: wgpu::BindGroupLayout,
    pub(crate) post_buf: wgpu::Buffer,
    pub(crate) post_sampler: wgpu::Sampler,
}

/// The enhanced path's post passes (post.wgsl): their bind group layout, uniform buffer,
/// sampler and pipelines.
pub(crate) fn post_pipelines(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    hdr_format: wgpu::TextureFormat,
) -> PostPipelines {
    // --- enhanced graphics: the post passes (glow, metering, adaptation, tone curve, FXAA)
    log::info!("renderer: compiling the post passes shaders");
    let post_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("post"),
        source: wgpu::ShaderSource::Wgsl(post_shader_source().into()),
    });
    let post_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("post"),
        entries: &[
            uniform_entry(0, wgpu::ShaderStages::FRAGMENT),
            float_tex(1),
            sampler_entry(2),
            float_tex(3),
            float_tex(4),
            // (the screen mask for the tone mapping: the condensation on the bus's
            // panes, see post.wgsl `misted`)
            float_tex(5),
        ],
    });
    let post_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("post params"),
        size: std::mem::size_of::<PostUniform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let post_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    let post_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("post"),
        bind_group_layouts: &[Some(&post_layout)],
        immediate_size: 0,
    });
    let post_pipeline = |entry: &str, target: wgpu::TextureFormat| {
        fullscreen_pipeline(
            device,
            entry,
            &post_pl,
            &post_shader,
            "vs_main",
            entry,
            wgpu::ColorTargetState {
                format: target,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            },
        )
    };
    PostPipelines {
        down_first: post_pipeline("fs_down_first", hdr_format),
        down: post_pipeline("fs_down", hdr_format),
        up: post_pipeline("fs_up", hdr_format),
        meter: post_pipeline("fs_meter", hdr_format),
        adapt: post_pipeline("fs_adapt", hdr_format),
        tonemap: post_pipeline("fs_tonemap", format),
        tonemap_encoded: post_pipeline("fs_tonemap_encoded", wgpu::TextureFormat::Rgba8Unorm),
        fxaa: post_pipeline("fs_fxaa", format),
        post_layout,
        post_buf,
        post_sampler,
    }
}

/// The eye's adaptation: the metered luminance, the adapted value and the exposure as it follows
/// the sky.
pub(crate) struct Exposure {
    /// The metered mean log luminance (1x1), and the adapted value in two textures drawn
    /// into by turns (`adapt_front` is the current one), with their bind groups.
    pub(crate) meter_view: wgpu::TextureView,
    pub(crate) adapt_views: [wgpu::TextureView; 2],
    pub(crate) adapt_bg: [wgpu::BindGroup; 2],
    pub(crate) adapt_front: usize,
    /// The lamps' and headlights' light on what the camera looks at (`view_lamp_light`),
    /// which the eye adapts to by night.
    pub(crate) view_lamps: Option<f32>,
    /// The pre-exposure (natural log) as it follows the sky's.
    pub(crate) exposure: Option<f32>,
}

impl Exposure {
    /// The meter's and the adapted value's textures and their bind groups.
    pub(crate) fn new(
        device: &wgpu::Device,
        post: &PostPipelines,
        white_texture: &GpuTexture,
        hdr_format: wgpu::TextureFormat,
    ) -> Self {
        let one = wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        };
        let tiny = |label: &str| {
            target_2d(
                device,
                label,
                one,
                1,
                hdr_format,
                wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
            )
            .create_view(&Default::default())
        };
        let meter_view = tiny("exposure meter");
        let adapt_views = [tiny("exposure a"), tiny("exposure b")];
        // adapt_bg[k] reads the meter and adapt[k] (and draws into the other one)
        let adapt_bg = [0usize, 1].map(|k| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("exposure"),
                layout: &post.post_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: post.post_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&meter_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&post.post_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&adapt_views[k]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(&white_texture.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::TextureView(&white_texture.view),
                    },
                ],
            })
        });
        Self {
            meter_view,
            adapt_views,
            adapt_bg,
            adapt_front: 0,
            view_lamps: None,
            exposure: None,
        }
    }
}
