//! The pipelines of the lamp coronas and the smoke: instanced sprites drawn over the scene.

use crate::gpu::{color_targets, DEPTH_FORMAT};
use crate::lighting::GpuCorona;
use crate::scene::Layouts;
use crate::shaders::corona_shader_source;

/// What the corona and smoke pipelines are made from.
pub struct CoronaBuilder<'a> {
    device: &'a wgpu::Device,
    module: wgpu::ShaderModule,
    layout: wgpu::PipelineLayout,
    msaa: u32,
}

/// The corona sprite's instance: its position, size and colours.
const CORONA_VERTEX: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
    array_stride: std::mem::size_of::<GpuCorona>() as u64,
    step_mode: wgpu::VertexStepMode::Instance,
    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32, 2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4],
};

/// Added to what is behind (the enhanced path, in high range).
pub fn additive() -> wgpu::BlendState {
    wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent::REPLACE,
    }
}

/// OMSI's lamp sprites: SRCBLEND ONE, DESTBLEND INVSRCCOLOR (src + dst * (1 - src)), which
/// keeps a coloured sprite's hue over a lit background instead of washing it to white.
pub fn screen() -> wgpu::BlendState {
    wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrc,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent::REPLACE,
    }
}

/// The smoke's blend: ordinary alpha.
pub fn alpha_blend() -> wgpu::BlendState {
    wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::SrcAlpha,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent::REPLACE,
    }
}

impl<'a> CoronaBuilder<'a> {
    /// `corona_layout` is the sprites' bind group (`SpriteResources::corona_layout`).
    /// `mode`: the mode's own fragment shaders, added to the module.
    pub fn new(
        device: &'a wgpu::Device,
        layouts: &Layouts,
        corona_layout: &wgpu::BindGroupLayout,
        msaa: u32,
        mode: &str,
    ) -> Self {
        log::info!("renderer: compiling the coronas shaders");
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("corona"),
            source: wgpu::ShaderSource::Wgsl(corona_shader_source(mode).into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("corona"),
            bind_group_layouts: &[Some(&layouts.camera_layout), Some(corona_layout)],
            immediate_size: 0,
        });
        Self {
            device,
            module,
            layout,
            msaa,
        }
    }

    pub fn pipeline(
        &self,
        format: wgpu::TextureFormat,
        fs: &str,
        blend: wgpu::BlendState,
    ) -> wgpu::RenderPipeline {
        self.device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("corona"),
                layout: Some(&self.layout),
                vertex: wgpu::VertexState {
                    module: &self.module,
                    entry_point: Some("vs_main"),
                    buffers: &[CORONA_VERTEX],
                    compilation_options: Default::default(),
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None,
                    front_face: wgpu::FrontFace::Ccw,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(false),
                    depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: self.msaa,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &self.module,
                    entry_point: Some(fs),
                    targets: &color_targets(format, Some(blend), wgpu::ColorWrites::COLOR, false),
                    compilation_options: Default::default(),
                }),
                multiview_mask: None,
                cache: None,
            })
    }
}
