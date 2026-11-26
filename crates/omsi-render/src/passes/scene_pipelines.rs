//! The scene's pipelines of the main pass: one set per colour format, fragment entry and
//! sample count, indexed by `pipe_code`.

use super::draw::{
    one_sided_primitive, PIPE_ALPHA_TEST, PIPE_BLEND, PIPE_BLEND_NO_WRITE, PIPE_KINDS,
    PIPE_SURFACE_DEPTH, PIPE_TERRAIN_PAINT,
};
use crate::gpu::{color_targets, DEPTH_FORMAT};
use crate::scene::{Layouts, Vertex};

/// What the scene's pipelines are made from: the scene shader (also the one the shadow,
/// depth and puddle pipelines draw with), its layout, and the depth bias of surfaces.
pub struct SceneBuilder<'a> {
    device: &'a wgpu::Device,
    shader: &'a wgpu::ShaderModule,
    layout: wgpu::PipelineLayout,
    vertex_layout: wgpu::VertexBufferLayout<'static>,
    surface_bias: i32,
}

impl<'a> SceneBuilder<'a> {
    pub fn new(
        device: &'a wgpu::Device,
        shader: &'a wgpu::ShaderModule,
        layouts: &Layouts,
    ) -> Self {
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("omsi"),
            bind_group_layouts: &[Some(&layouts.camera_layout), Some(&layouts.material_layout)],
            immediate_size: 0,
        });
        Self {
            device,
            shader,
            layout,
            vertex_layout: Vertex::LAYOUT,
            surface_bias: crate::gpu::debug().surface_bias.unwrap_or(-24),
        }
    }

    /// The vertex layout the scene's meshes use (the depth prepass draws with it too).
    pub fn vertex_layout(&self) -> &wgpu::VertexBufferLayout<'static> {
        &self.vertex_layout
    }

    #[allow(clippy::too_many_arguments)]
    fn make(
        &self,
        format: wgpu::TextureFormat,
        fs: &str,
        blend: Option<wgpu::BlendState>,
        depth_write: bool,
        cull: bool,
        bias: i32,
        alpha_to_coverage: bool,
        terrain_paint: bool,
        samples: u32,
    ) -> wgpu::RenderPipeline {
        let use_alpha_to_coverage = alpha_to_coverage && samples > 1;
        self.device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("omsi"),
                layout: Some(&self.layout),
                vertex: wgpu::VertexState {
                    module: self.shader,
                    entry_point: Some("vs_main"),
                    buffers: std::slice::from_ref(&self.vertex_layout),
                    compilation_options: Default::default(),
                },
                primitive: one_sided_primitive(cull),
                // Reversed Z: the near plane is 1 and the far plane 0, so nearer means
                // greater. The depth bias keeps its meaning (negative = towards the viewer)
                // only if its sign is turned around with the axis.
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(depth_write),
                    depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
                    stencil: Default::default(),
                    bias: wgpu::DepthBiasState {
                        constant: -bias,
                        slope_scale: if bias != 0 {
                            -bias.signum() as f32 * 2.0
                        } else {
                            0.0
                        },
                        clamp: 0.0,
                    },
                }),
                multisample: wgpu::MultisampleState {
                    count: samples,
                    mask: !0,
                    alpha_to_coverage_enabled: use_alpha_to_coverage,
                },
                fragment: Some(wgpu::FragmentState {
                    module: self.shader,
                    entry_point: Some(fs),
                    targets: &color_targets(
                        format,
                        blend,
                        if fs == "fs_surface_depth" {
                            wgpu::ColorWrites::empty()
                        } else {
                            wgpu::ColorWrites::ALL
                        },
                        fs != "fs_surface_depth",
                    ),
                    // Only cutouts and painted terrain keep their discard; opaque and
                    // ordinary blended materials retain early depth testing.
                    compilation_options: wgpu::PipelineCompilationOptions {
                        constants: &[
                            ("ALPHA_TEST", if alpha_to_coverage { 1.0 } else { 0.0 }),
                            (
                                "ALPHA_TO_COVERAGE",
                                if use_alpha_to_coverage { 1.0 } else { 0.0 },
                            ),
                            ("TERRAIN_PAINT", if terrain_paint { 1.0 } else { 0.0 }),
                        ],
                        ..Default::default()
                    },
                }),
                multiview_mask: None,
                cache: None,
            })
    }

    /// One pipeline per `pipe_code`: the kind decides blending and the depth write.
    pub fn pipelines(
        &self,
        format: wgpu::TextureFormat,
        fs: &str,
        samples: u32,
    ) -> Vec<wgpu::RenderPipeline> {
        let mut out = Vec::with_capacity(PIPE_KINDS as usize * 4);
        for kind in 0..PIPE_KINDS {
            let blend = matches!(kind, PIPE_BLEND | PIPE_BLEND_NO_WRITE | PIPE_TERRAIN_PAINT)
                .then_some(wgpu::BlendState::ALPHA_BLENDING);
            let depth_write = kind != PIPE_BLEND_NO_WRITE && kind != PIPE_TERRAIN_PAINT;
            for cull in [false, true] {
                for surface in [false, true] {
                    out.push(self.make(
                        format,
                        if kind == PIPE_SURFACE_DEPTH {
                            "fs_surface_depth"
                        } else {
                            fs
                        },
                        blend,
                        depth_write,
                        cull,
                        if surface { self.surface_bias } else { 0 },
                        kind == PIPE_ALPHA_TEST,
                        kind == PIPE_TERRAIN_PAINT && fs == "fs_enhanced",
                        samples,
                    ));
                }
            }
        }
        out
    }
}
