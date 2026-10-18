//! The depth prepass and the ambient occlusion (SSAO), with their pipelines and uniform.

use super::draw::{encode_batches, one_sided_primitive, Batch};
use crate::gpu::{fullscreen_pipeline, pass_timer, DEPTH_FORMAT};
use crate::scene::{Camera, Scene};
use crate::targets::TargetCache;
use glam::Mat4;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct SsaoUniform {
    pub(crate) inv_proj: [[f32; 4]; 4],
    pub(crate) params: [f32; 4],
    /// xy: the projection's off-centre shift (0 for the window's own symmetric frustum;
    /// a headset eye or a triple screen's side panel has one)
    pub(crate) shift: [f32; 4],
}

/// The depth prepass's pipelines (`pipe_code` kinds 0-2, culled or not), single-sampled
/// and, with multisampling, multisampled.
pub(crate) fn prepass_pipelines(
    device: &wgpu::Device,
    camera_layout: &wgpu::BindGroupLayout,
    material_layout: &wgpu::BindGroupLayout,
    msaa: u32,
    shader: &wgpu::ShaderModule,
    vertex_layout: &wgpu::VertexBufferLayout<'_>,
) -> PrepassPipelines {
    let prepass_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("prepass"),
        bind_group_layouts: &[Some(&camera_layout), Some(&material_layout)],
        immediate_size: 0,
    });
    // the prepass culls exactly as the main pass does: a back face that wrote depth
    // here would hide what the main pass then draws behind it
    let make_prepass_samples = |kind: u8, cull: bool, samples: u32| {
        let fragment = match kind {
            0 => "fs_shadow",
            1 => "fs_shadow_test",
            2 => "fs_transmap_depth",
            _ => unreachable!(),
        };
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("depth prepass"),
            layout: Some(&prepass_pl),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[vertex_layout.clone()],
                compilation_options: Default::default(),
            },
            primitive: one_sided_primitive(cull),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: samples,
                ..Default::default()
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some(fragment),
                targets: &[],
                compilation_options: Default::default(),
            }),
            multiview_mask: None,
            cache: None,
        })
    };
    let make_prepass = |kind: u8, cull: bool| make_prepass_samples(kind, cull, 1);
    let prepass_pipelines = [
        make_prepass(0, false),
        make_prepass(0, true),
        make_prepass(1, false),
        make_prepass(1, true),
        make_prepass(2, false),
        make_prepass(2, true),
    ];
    // Apple views with cutout/blended draws also need these pipelines: their
    // visibility depends on shading, unlike opaque hidden-surface removal.
    // Purely opaque Apple views still skip this pass below to avoid its cost.
    let prepass_msaa_pipelines = (msaa > 1).then(|| {
        [
            (0, false),
            (0, true),
            (1, false),
            (1, true),
            (2, false),
            (2, true),
        ]
        .map(|(kind, cull)| make_prepass_samples(kind, cull, msaa))
    });
    PrepassPipelines {
        prepass_pipelines,
        prepass_msaa_pipelines,
    }
}

/// The ambient occlusion: its bind group layout, uniform buffer and sampler, and the AO and
/// blur pipelines (none on OpenGL).
pub(crate) fn ao_pipelines(device: &wgpu::Device, gl: bool) -> SsaoPass {
    // ambient occlusion: a depth prepass with the camera projection, then the AO and a blur
    log::info!("renderer: compiling the SSAO shaders");
    let ssao_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("ssao"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/post/ssao.wgsl").into()),
    });
    let ao_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("ssao"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
        ],
    });
    let ao_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ssao params"),
        size: std::mem::size_of::<SsaoUniform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let ao_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    let ao_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("ssao"),
        bind_group_layouts: &[Some(&ao_layout)],
        immediate_size: 0,
    });
    let make_ao = |entry: &str| {
        fullscreen_pipeline(
            device,
            entry,
            &ao_pl,
            &ssao_shader,
            "vs_main",
            entry,
            wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rg16Float,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            },
        )
    };
    let ssao_pipeline = (!gl).then(|| make_ao("fs_ssao"));
    let blur_pipeline = (!gl).then(|| make_ao("fs_blur"));
    SsaoPass {
        ao_sampler,
        ao_layout,
        ao_buf,
        ssao_pipeline,
        blur_pipeline,
    }
}

impl SsaoPass {
    /// The depth prepass and the ambient occlusion (single-sampled, with the camera's
    /// projection), into the targets' AO textures.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn encode_prepass_and_ao(
        &self,
        queue: &wgpu::Queue,
        prepass: &PrepassPipelines,
        targets: &TargetCache,
        scene: &Scene,
        prepass_encoder: &mut wgpu::CommandEncoder,
        prepass_batches: &[Batch],
        camera: &Camera,
        projection: Option<Mat4>,
        aspect: f32,
        width: u32,
        height: u32,
        prepass_on: bool,
        ao_on: bool,
        tset: &Option<wgpu::QuerySet>,
        timed: &mut Vec<&'static str>,
    ) {
        // --- depth prepass + ambient occlusion (single-sampled, camera projection)
        if prepass_on {
            let proj = projection.unwrap_or_else(|| {
                Mat4::perspective_rh(camera.fov_deg.to_radians(), aspect, camera.far, camera.near)
            });
            let u = SsaoUniform {
                inv_proj: proj.inverse().to_cols_array_2d(),
                params: [
                    1.0,
                    1.4,
                    width.div_ceil(2) as f32,
                    height.div_ceil(2) as f32,
                ],
                shift: [proj.z_axis.x, proj.z_axis.y, 0.0, 0.0],
            };
            queue.write_buffer(&self.ao_buf, 0, bytemuck::bytes_of(&u));
            let ao = targets.ao.as_ref().unwrap();
            {
                let mut pass = prepass_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("depth prepass"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &ao.depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(0.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: pass_timer(tset.as_ref(), timed, "depth prepass"),
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_bind_group(0, scene.camera_bind_group.as_ref().unwrap(), &[]);
                encode_batches(&mut pass, scene, &prepass_batches, |pipe| {
                    &prepass.prepass_pipelines[pipe as usize]
                });
            }
            for (pipe, bg, target, pass_label) in [
                (&self.ssao_pipeline, &ao.ssao_bg, &ao.ao_view, "ssao"),
                (&self.blur_pipeline, &ao.blur_bg, &ao.blur_view, "ssao blur"),
            ] {
                let Some(pipe) = pipe.as_ref().filter(|_| ao_on) else {
                    break;
                };
                let mut pass = prepass_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("ssao"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: target,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: pass_timer(tset.as_ref(), timed, pass_label),
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(pipe);
                pass.set_bind_group(0, bg, &[]);
                pass.draw(0..3, 0..1);
            }
        }
    }
}

/// The depth prepass's pipelines.
pub(crate) struct PrepassPipelines {
    /// Depth prepass pipelines (plain, alpha-tested; each two-sided and culled),
    /// single-sampled, camera projection.
    /// Plain opaque, alpha-tested, and the opaque portions of blended transmap materials;
    /// each has a two-sided and a culled variant.
    pub(crate) prepass_pipelines: [wgpu::RenderPipeline; 6],
    /// The same, multisampled: the enhanced main pass's own depth laid first (see
    /// `render_inner`), so that its costly shading runs once per visible surface.
    pub(crate) prepass_msaa_pipelines: Option<[wgpu::RenderPipeline; 6]>,
}

/// Ambient occlusion: its sampler, bind group layout, parameters and pipelines.
pub(crate) struct SsaoPass {
    pub(crate) ao_sampler: wgpu::Sampler,
    pub(crate) ao_layout: wgpu::BindGroupLayout,
    pub(crate) ao_buf: wgpu::Buffer,
    /// Ambient occlusion and its blur; none on OpenGL (GLES), whose shading language cannot
    /// read a depth texture texel by texel - the pipelines failed there, AO off or not (#422).
    pub(crate) ssao_pipeline: Option<wgpu::RenderPipeline>,
    pub(crate) blur_pipeline: Option<wgpu::RenderPipeline>,
}
