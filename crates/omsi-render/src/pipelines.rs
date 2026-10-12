//! The render pipelines the renderer makes once, at start: the scene's, the shadows', the
//! sky's, the overlays' and the post passes'.

use super::*;

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
}

/// The pipelines of the main pass for one colour target format: the swap chain's, and
/// the high-range one of the enhanced path.
pub(crate) struct PassPipelines {
    /// Single-sampled films, drawn after resolving the scene and its reflections.
    pub(crate) rain_pipelines: Vec<wgpu::RenderPipeline>,
    /// Indexed by `pipe_code`: 4 depth/blend kinds x culled x depth-biased.
    pub(crate) pipelines: Vec<wgpu::RenderPipeline>,
    pub(crate) corona_pipeline: wgpu::RenderPipeline,
    /// Smoke particles (`[smoke]`): the corona sprite alpha-blended with the smoke texture.
    pub(crate) smoke_pipeline: wgpu::RenderPipeline,
    /// The snowfall (snow.wgsl), opaque over the scene (premultiplied); none with
    /// `basic_pipelines`.
    pub(crate) snow_pipeline: Option<wgpu::RenderPipeline>,
    pub(crate) sky_pipeline: wgpu::RenderPipeline,
}

/// The HUD overlays: their bind group layout, shader and pipeline.
pub(crate) fn overlay_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    msaa: u32,
) -> (
    wgpu::BindGroupLayout,
    wgpu::ShaderModule,
    wgpu::PipelineLayout,
    wgpu::BlendState,
    wgpu::RenderPipeline,
) {
    // HUD overlay quads
    let overlay_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("overlay"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
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
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });
    log::info!("renderer: compiling the overlays shaders");
    let overlay_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("overlay"),
        source: wgpu::ShaderSource::Wgsl(include_str!("overlay.wgsl").into()),
    });
    let overlay_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("overlay"),
        bind_group_layouts: &[Some(&overlay_layout)],
        immediate_size: 0,
    });
    let premul = wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent::OVER,
    };
    let overlay_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("overlay"),
        layout: Some(&overlay_pl),
        vertex: wgpu::VertexState {
            module: &overlay_shader,
            entry_point: Some("vs_main"),
            buffers: &[],
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
            count: msaa,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        fragment: Some(wgpu::FragmentState {
            module: &overlay_shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(premul),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        multiview_mask: None,
        cache: None,
    });
    (
        overlay_layout,
        overlay_shader,
        overlay_pl,
        premul,
        overlay_pipeline,
    )
}

/// The ambient occlusion: its bind group layout, uniform buffer and sampler, and the AO and
/// blur pipelines (none on OpenGL).
pub(crate) fn ao_pipelines(
    device: &wgpu::Device,
    gl: bool,
) -> (
    wgpu::BindGroupLayout,
    wgpu::Buffer,
    wgpu::Sampler,
    Option<wgpu::RenderPipeline>,
    Option<wgpu::RenderPipeline>,
) {
    // ambient occlusion: a depth prepass with the camera projection, then the AO and a blur
    log::info!("renderer: compiling the SSAO shaders");
    let ssao_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("ssao"),
        source: wgpu::ShaderSource::Wgsl(include_str!("ssao.wgsl").into()),
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
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(entry),
            layout: Some(&ao_pl),
            vertex: wgpu::VertexState {
                module: &ssao_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &ssao_shader,
                entry_point: Some(entry),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rg16Float,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            multiview_mask: None,
            cache: None,
        })
    };
    let ssao_pipeline = (!gl).then(|| make_ao("fs_ssao"));
    let blur_pipeline = (!gl).then(|| make_ao("fs_blur"));
    (ao_layout, ao_buf, ao_sampler, ssao_pipeline, blur_pipeline)
}

/// The lamps' light in the fog: its bind group layout, uniform buffer and the two pipelines
/// (none on OpenGL).
pub(crate) fn fog_lamps_pipelines(
    device: &wgpu::Device,
    camera_layout: &wgpu::BindGroupLayout,
    gl: bool,
    hdr_format: wgpu::TextureFormat,
) -> (
    wgpu::BindGroupLayout,
    wgpu::Buffer,
    Option<[wgpu::RenderPipeline; 2]>,
) {
    // the lamps in the fog: the camera group (lights, grid, the enhanced uniform) and the
    // prepass depth, added onto the high-range picture
    let fog_lamps_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("fog lamps"),
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
    let fog_lamps_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("fog lamps params"),
        size: std::mem::size_of::<FogLampUniform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let fog_lamps_pipeline = (!gl && array_path() != ArrayPath::NoStorage && !basic_pipelines())
        .then(|| {
            log::info!("renderer: compiling the lamps in the fog shaders");
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("fog lamps"),
                source: wgpu::ShaderSource::Wgsl(fog_lamps_shader_source().into()),
            });
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("fog lamps"),
                bind_group_layouts: &[Some(&camera_layout), Some(&fog_lamps_layout)],
                immediate_size: 0,
            });
            let make = |entry: &str, blend: Option<wgpu::BlendState>| {
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(entry),
                    layout: Some(&layout),
                    vertex: wgpu::VertexState {
                        module: &module,
                        entry_point: Some("vs_fog_lamps"),
                        buffers: &[],
                        compilation_options: Default::default(),
                    },
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        cull_mode: None,
                        ..Default::default()
                    },
                    depth_stencil: None,
                    multisample: Default::default(),
                    fragment: Some(wgpu::FragmentState {
                        module: &module,
                        entry_point: Some(entry),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: hdr_format,
                            blend,
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                        compilation_options: Default::default(),
                    }),
                    multiview_mask: None,
                    cache: None,
                })
            };
            // (the second added onto what is there)
            let add = wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::Zero,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
            };
            [
                make("fs_fog_lamps", None),
                make("fs_fog_composite", Some(add)),
            ]
        });
    (fog_lamps_layout, fog_lamps_buf, fog_lamps_pipeline)
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
) -> ([wgpu::RenderPipeline; 6], Option<[wgpu::RenderPipeline; 6]>) {
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
    (prepass_pipelines, prepass_msaa_pipelines)
}

/// A fragment shader's filterable 2D texture at `binding`.
pub(crate) fn float_tex(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

/// The mip map generator (mip.wgsl): its bind group layout, pipeline and sampler.
pub(crate) fn mip_pipeline(
    device: &wgpu::Device,
) -> (wgpu::BindGroupLayout, wgpu::RenderPipeline, wgpu::Sampler) {
    // --- mipmaps on the GPU: the CPU box filter took up to a second per bus spawn
    log::info!("renderer: compiling the mip maps shaders");
    let mip_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("mip"),
        source: wgpu::ShaderSource::Wgsl(include_str!("mip.wgsl").into()),
    });
    let mip_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("mip"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });
    let mip_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("mip"),
        bind_group_layouts: &[Some(&mip_layout)],
        immediate_size: 0,
    });
    let mip_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("mip"),
        layout: Some(&mip_pl),
        vertex: wgpu::VertexState {
            module: &mip_shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &mip_shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        multiview_mask: None,
        cache: None,
    });
    let mip_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    (mip_layout, mip_pipeline, mip_sampler)
}

/// The enhanced path's post passes (post.wgsl): their bind group layout, uniform buffer,
/// sampler and pipelines.
pub(crate) fn post_pipelines(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    hdr_format: wgpu::TextureFormat,
) -> (
    wgpu::BindGroupLayout,
    wgpu::Buffer,
    wgpu::Sampler,
    PostPipelines,
) {
    // --- enhanced graphics: the post passes (glow, metering, adaptation, tone curve, FXAA)
    log::info!("renderer: compiling the post passes shaders");
    let post_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("post"),
        source: wgpu::ShaderSource::Wgsl(include_str!("post.wgsl").into()),
    });
    let post_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("post"),
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
            float_tex(1),
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
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
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(entry),
            layout: Some(&post_pl),
            vertex: wgpu::VertexState {
                module: &post_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &post_shader,
                entry_point: Some(entry),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            multiview_mask: None,
            cache: None,
        })
    };
    let post = PostPipelines {
        down_first: post_pipeline("fs_down_first", hdr_format),
        down: post_pipeline("fs_down", hdr_format),
        up: post_pipeline("fs_up", hdr_format),
        meter: post_pipeline("fs_meter", hdr_format),
        adapt: post_pipeline("fs_adapt", hdr_format),
        tonemap: post_pipeline("fs_tonemap", format),
        tonemap_encoded: post_pipeline("fs_tonemap_encoded", wgpu::TextureFormat::Rgba8Unorm),
        fxaa: post_pipeline("fs_fxaa", format),
    };
    (post_layout, post_buf, post_sampler, post)
}

/// The overlays drawn single-sampled and the VR interface's pipeline (xr_ui.wgsl), both on
/// the overlays' layout.
pub(crate) fn overlay_1x_pipelines(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    overlay_pl: &wgpu::PipelineLayout,
    overlay_shader: &wgpu::ShaderModule,
    premul: wgpu::BlendState,
) -> (wgpu::RenderPipeline, wgpu::RenderPipeline) {
    let overlay_pipeline_1x = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("overlay 1x"),
        layout: Some(&overlay_pl),
        vertex: wgpu::VertexState {
            module: &overlay_shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            front_face: wgpu::FrontFace::Ccw,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &overlay_shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(premul),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        multiview_mask: None,
        cache: None,
    });
    log::info!("renderer: compiling the VR interface shaders");
    let xr_ui_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("OpenXR spatial UI"),
        source: wgpu::ShaderSource::Wgsl(include_str!("xr_ui.wgsl").into()),
    });
    let xr_ui_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("OpenXR spatial UI"),
        layout: Some(&overlay_pl),
        vertex: wgpu::VertexState {
            module: &xr_ui_shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            front_face: wgpu::FrontFace::Ccw,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &xr_ui_shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(premul),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        multiview_mask: None,
        cache: None,
    });
    (overlay_pipeline_1x, xr_ui_pipeline)
}

/// The render scale's pipelines (upscale.wgsl): the scaled-up picture, the plain copy and a
/// triple screen's panel, with their bind group layout and uniform buffer.
pub(crate) fn upscale_pipelines(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
) -> (
    wgpu::BindGroupLayout,
    wgpu::RenderPipeline,
    wgpu::RenderPipeline,
    wgpu::RenderPipeline,
    wgpu::Buffer,
) {
    // --- render scale: the smaller 3D picture scaled up to the window
    log::info!("renderer: compiling the upscaler shaders");
    let upscale_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("upscale"),
        source: wgpu::ShaderSource::Wgsl(include_str!("upscale.wgsl").into()),
    });
    let upscale_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("upscale"),
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
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });
    let upscale_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("upscale"),
        bind_group_layouts: &[Some(&upscale_layout)],
        immediate_size: 0,
    });
    let upscale_pipeline_for = |entry| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("upscale"),
            layout: Some(&upscale_pl),
            vertex: wgpu::VertexState {
                module: &upscale_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &upscale_shader,
                entry_point: Some(entry),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            multiview_mask: None,
            cache: None,
        })
    };
    let upscale_pipeline = upscale_pipeline_for("fs_main");
    let copy_pipeline = upscale_pipeline_for("fs_copy");
    let panel_pipeline = upscale_pipeline_for("fs_panel");
    let upscale_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("upscale params"),
        size: 16,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    (
        upscale_layout,
        upscale_pipeline,
        copy_pipeline,
        panel_pipeline,
        upscale_buf,
    )
}

/// The sun's shadow maps (near and far), their sampler, and the pipelines drawn into them:
/// the clearing one and one per kind and cascade.
pub(crate) fn shadow_pipelines(
    device: &wgpu::Device,
    shadow_size: u32,
    shader: &wgpu::ShaderModule,
    shadow_layout: &wgpu::BindGroupLayout,
    material_layout: &wgpu::BindGroupLayout,
    vertex_layout: &wgpu::VertexBufferLayout<'_>,
) -> (
    wgpu::TextureView,
    wgpu::TextureView,
    wgpu::Sampler,
    wgpu::RenderPipeline,
    Vec<wgpu::RenderPipeline>,
) {
    // sun shadow map: depth only, from the light's orthographic camera
    let shadow_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("shadow map"),
        size: wgpu::Extent3d {
            // near cascade on the left, close cascade on the right
            width: shadow_size * 2,
            height: shadow_size,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let shadow_view = shadow_tex.create_view(&Default::default());
    let shadow_tex_far = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("shadow map far"),
        // (and the street lamps' tiles under it, see `LAMP_SHADOWS`)
        size: wgpu::Extent3d {
            width: shadow_size,
            height: (shadow_size as f32 * FAR_MAP_ASPECT) as u32,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let shadow_view_far = shadow_tex_far.create_view(&Default::default());
    let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        compare: Some(wgpu::CompareFunction::LessEqual),
        ..Default::default()
    });
    let shadow_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("shadow"),
        bind_group_layouts: &[Some(&shadow_layout), Some(&material_layout)],
        immediate_size: 0,
    });
    let make_shadow = |kind: u8, cascade: u8| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow"),
            layout: Some(&shadow_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some(match cascade {
                    0 => "vs_shadow",
                    1 => "vs_shadow_far",
                    2 => "vs_shadow_close",
                    3 => "vs_shadow_lamp0",
                    4 => "vs_shadow_lamp1",
                    5 => "vs_shadow_lamp2",
                    _ => "vs_shadow_lamp3",
                }),
                buffers: &[vertex_layout.clone()],
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                front_face: wgpu::FrontFace::Cw,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: wgpu::DepthBiasState {
                    constant: 4,
                    slope_scale: 3.0,
                    clamp: 0.0,
                },
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some(if kind == PIPE_ALPHA_TEST {
                    "fs_shadow_test"
                } else {
                    "fs_shadow"
                }),
                targets: &[],
                compilation_options: Default::default(),
            }),
            multiview_mask: None,
            cache: None,
        })
    };
    let shadow_clear_pipeline = {
        log::info!("renderer: compiling the shadow clear shader");
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shadow clear"),
            source: wgpu::ShaderSource::Wgsl(
                "@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
                    let x = f32(i32(i & 1u) * 4 - 1);
                    let y = f32(i32(i >> 1u) * 4 - 1);
                    return vec4<f32>(x, y, 1.0, 1.0);
                }"
                .into(),
            ),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shadow clear"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow clear"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: None,
            multiview_mask: None,
            cache: None,
        })
    };
    let mut shadow_pipelines = vec![
        make_shadow(PIPE_OPAQUE, 0),
        make_shadow(PIPE_ALPHA_TEST, 0),
        make_shadow(PIPE_OPAQUE, 1),
        make_shadow(PIPE_ALPHA_TEST, 1),
        make_shadow(PIPE_OPAQUE, 2),
        make_shadow(PIPE_ALPHA_TEST, 2),
    ];
    if !basic_pipelines() {
        // (the street lamps' tiles, 6 + 2 k + kind)
        log::info!("renderer: compiling the street lamps' shadow shaders");
        for cascade in 3..=6 {
            shadow_pipelines.push(make_shadow(PIPE_OPAQUE, cascade));
            shadow_pipelines.push(make_shadow(PIPE_ALPHA_TEST, cascade));
        }
    }
    (
        shadow_view,
        shadow_view_far,
        shadow_sampler,
        shadow_clear_pipeline,
        shadow_pipelines,
    )
}
