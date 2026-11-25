//! The snowfall's pipeline (`snow.wgsl`): the camera group and its own parameters, no
//! vertices.

use crate::gpu::{color_targets, uniform_entry, DEPTH_FORMAT};
use crate::scene::Layouts;
use crate::shaders::snow_shader_source;

/// The snowfall's bind group: its parameters, read by the vertex stage.
pub(crate) fn snow_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("snow"),
        entries: &[uniform_entry(0, wgpu::ShaderStages::VERTEX)],
    })
}

/// What the snowfall's pipelines are made from.
pub(crate) struct SnowBuilder<'a> {
    device: &'a wgpu::Device,
    module: wgpu::ShaderModule,
    layout: wgpu::PipelineLayout,
    msaa: u32,
}

impl<'a> SnowBuilder<'a> {
    /// `mode`: the mode's own fragment shaders, added to the module.
    pub(crate) fn new(
        device: &'a wgpu::Device,
        layouts: &Layouts,
        snow_layout: &wgpu::BindGroupLayout,
        msaa: u32,
        mode: &str,
    ) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("snow"),
            source: wgpu::ShaderSource::Wgsl(snow_shader_source(mode).into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("snow"),
            bind_group_layouts: &[Some(&layouts.camera_layout), Some(snow_layout)],
            immediate_size: 0,
        });
        Self {
            device,
            module,
            layout,
            msaa,
        }
    }

    pub(crate) fn pipeline(&self, format: wgpu::TextureFormat, fs: &str) -> wgpu::RenderPipeline {
        log::info!("renderer: compiling the snowfall shader ({fs})");
        let premultiplied = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent::REPLACE,
        };
        self.device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("snow"),
                layout: Some(&self.layout),
                vertex: wgpu::VertexState {
                    module: &self.module,
                    entry_point: Some("vs_snow"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None,
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
                    targets: &color_targets(
                        format,
                        Some(premultiplied),
                        wgpu::ColorWrites::COLOR,
                        false,
                    ),
                    compilation_options: Default::default(),
                }),
                multiview_mask: None,
                cache: None,
            })
    }
}
