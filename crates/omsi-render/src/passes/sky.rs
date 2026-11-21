//! The sky's passes: its pipelines, a face of the enhanced sky cube, and the reflection
//! probe with its blurred mip chain.

use super::draw::PostPass;
use crate::gpu::{color_targets, pass_timer, DEPTH_FORMAT};
use crate::scene::{Layouts, Scene};
use crate::shaders::sky_shader_source;
use crate::sky::{SkyResources, SKY_CUBE_EVERY, SKY_CUBE_HISTORY, SKY_CUBE_ROUNDS};

/// What the sky's pipelines are made from: the sky shader (the sky resources draw with it
/// too), its layout and the dome's vertex.
pub(crate) struct SkyBuilder<'a> {
    device: &'a wgpu::Device,
    pub(crate) module: wgpu::ShaderModule,
    layout: wgpu::PipelineLayout,
    msaa: u32,
}

/// The dome's vertex: a position.
const SKY_VERTEX: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
    array_stride: 12,
    step_mode: wgpu::VertexStepMode::Vertex,
    attributes: &wgpu::vertex_attr_array![0 => Float32x3],
};

impl<'a> SkyBuilder<'a> {
    pub(crate) fn new(device: &'a wgpu::Device, layouts: &Layouts, msaa: u32) -> Self {
        log::info!("renderer: compiling the sky and clouds shaders");
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sky"),
            source: wgpu::ShaderSource::Wgsl(sky_shader_source().into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sky"),
            bind_group_layouts: &[Some(&layouts.camera_layout), Some(&layouts.sky_layout)],
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
        self.device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("sky"),
                layout: Some(&self.layout),
                vertex: wgpu::VertexState {
                    module: &self.module,
                    entry_point: Some("vs_main"),
                    buffers: &[SKY_VERTEX],
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
                    depth_compare: Some(wgpu::CompareFunction::Always),
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
                    targets: &color_targets(format, None, wgpu::ColorWrites::COLOR, false),
                    compilation_options: Default::default(),
                }),
                multiview_mask: None,
                cache: None,
            })
    }
}

impl SkyResources {
    /// The enhanced sky cube: a face a frame, all six the first time and for a new sky.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn encode_sky_cube(
        &mut self,
        scene: &Scene,
        prepass_encoder: &mut wgpu::CommandEncoder,
        enhanced: bool,
        lead_view: bool,
        probe_redraw: bool,
        redraw_near: bool,
        instant_exposure: bool,
        tset: &Option<wgpu::QuerySet>,
        timed: &mut Vec<&'static str>,
    ) {
        // --- the enhanced sky cube: a face a frame (all six the first time and for a new
        // sky), drawn in the window's frame only
        if enhanced && (lead_view || probe_redraw) {
            if let (Some(probe), Some(sky_bg)) =
                (self.probe.as_mut(), scene.sky_bind_group.as_ref())
            {
                // (face, round, the old picture's share): a whole new cube is every round
                // of every face averaged; afterwards one face a frame, blended in
                let full = !probe.cube_filled || instant_exposure;
                probe.cube_wait += 1;
                let recapture = std::mem::take(&mut probe.cube_recapture);
                let draws: Vec<(u32, u32, f64)> = if full {
                    (0..6)
                        .flat_map(|f| {
                            (0..SKY_CUBE_ROUNDS).map(move |r| (f, r, r as f64 / (r as f64 + 1.0)))
                        })
                        .collect()
                } else if recapture {
                    // the eye moved on: every face from the new one, nothing kept from the
                    // old place
                    let round = (probe.cube_round / 6) % SKY_CUBE_ROUNDS;
                    (0..6).map(|f| (f, round, 0.0)).collect()
                } else if (probe.cube_wait >= SKY_CUBE_EVERY && !redraw_near)
                    || probe.cube_wait >= SKY_CUBE_EVERY * 2
                    || !lead_view
                {
                    // (on a frame that keeps the near shadow map: the two costliest
                    // occasional passes never fall on the same frame)
                    vec![(
                        probe.cube_next,
                        (probe.cube_round / 6) % SKY_CUBE_ROUNDS,
                        SKY_CUBE_HISTORY,
                    )]
                } else {
                    Vec::new()
                };
                let single = draws.len() == 1;
                if !draws.is_empty() {
                    probe.cube_wait = 0;
                    probe.cube_next = (probe.cube_next + 1) % 6;
                    probe.cube_round = probe.cube_round.wrapping_add(1);
                }
                probe.cube_filled = true;
                for (f, round, history) in draws {
                    let mut pass = PostPass::new("sky cube", &probe.cube_faces[f as usize])
                        .load(if history > 0.0 {
                            wgpu::LoadOp::Load
                        } else {
                            wgpu::LoadOp::Clear(wgpu::Color::BLACK)
                        })
                        .timer(if single {
                            pass_timer(tset.as_ref(), timed, "sky cube")
                        } else {
                            None
                        })
                        .begin(prepass_encoder);
                    pass.set_pipeline(&probe.cube_pipeline);
                    pass.set_blend_constant(wgpu::Color {
                        r: history,
                        g: history,
                        b: history,
                        a: history,
                    });
                    pass.set_bind_group(
                        0,
                        &probe.cube_bind_groups[(f * SKY_CUBE_ROUNDS + round) as usize],
                        &[],
                    );
                    pass.set_bind_group(1, sky_bg, &[]);
                    pass.draw(0..3, 0..1);
                }
            }
        }
    }

    /// The enhanced path's reflection probe: the sky into its six faces, then the blurred
    /// mip levels.
    pub(crate) fn encode_probe(
        &mut self,
        scene: &Scene,
        prepass_encoder: &mut wgpu::CommandEncoder,
        probe_redraw: bool,
        tset: &Option<wgpu::QuerySet>,
        timed: &mut Vec<&'static str>,
    ) {
        // --- the reflection probe of the enhanced path: the sky into the six faces, then
        // each blurrier level from the sharper ones (three faces a pass)
        if probe_redraw {
            if let (Some(probe), Some(sky_bg)) =
                (self.probe.as_ref(), scene.sky_bind_group.as_ref())
            {
                for (m, faces) in probe.faces.iter().enumerate() {
                    for (half, bg) in probe.bind_groups[m].iter().enumerate() {
                        let attachments: Vec<Option<wgpu::RenderPassColorAttachment>> = faces
                            [half * 3..half * 3 + 3]
                            .iter()
                            .map(|v| {
                                Some(wgpu::RenderPassColorAttachment {
                                    view: v,
                                    depth_slice: None,
                                    resolve_target: None,
                                    ops: wgpu::Operations {
                                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                        store: wgpu::StoreOp::Store,
                                    },
                                })
                            })
                            .collect();
                        let mut pass =
                            prepass_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: Some("reflection probe"),
                                color_attachments: &attachments,
                                depth_stencil_attachment: None,
                                timestamp_writes: if m == 0 && half == 0 {
                                    pass_timer(tset.as_ref(), timed, "probe")
                                } else {
                                    None
                                },
                                occlusion_query_set: None,
                                multiview_mask: None,
                            });
                        pass.set_pipeline(if m == 0 {
                            &probe.sky_pipeline
                        } else {
                            &probe.filter_pipeline
                        });
                        pass.set_bind_group(0, bg, &[]);
                        pass.set_bind_group(1, sky_bg, &[]);
                        pass.draw(0..3, 0..1);
                    }
                }
            }
        }
    }
}
