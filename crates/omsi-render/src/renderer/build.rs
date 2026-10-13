//! Making everything the renderer draws with, once at start (`Renderer::build`).

use super::options::RenderOptions;
use crate::assets::{AssetStore, TexAddressing};
use crate::gpu::{GpuContext, Samplers};
use crate::lighting::{GpuCorona, LightMapAtlas};
use crate::output::{Diagnostics, Presenter};
use crate::passes::{Exposure, ShadowResources, SpriteResources};
use crate::scene::{CullHistory, Layouts};
use crate::sky::{EnhancedUniform, SkyResources};
use crate::targets::TargetCache;

use super::state::{color_targets, Renderer, DEPTH_FORMAT, RT_GBUF};
use crate::effects::{puddles, rt};
use crate::gpu::{basic_pipelines, sixteen_texture_units, GL_BACKEND};
use crate::passes::{ao_pipelines, fog_lamps_pipelines, post_pipelines, prepass_pipelines};
use crate::passes::{
    one_sided_primitive, PassPipelines, PIPE_ALPHA_TEST, PIPE_BLEND, PIPE_BLEND_NO_WRITE,
    PIPE_KINDS, PIPE_SURFACE_DEPTH, PIPE_TERRAIN_PAINT,
};
use crate::scene::{CameraUniform, Vertex};
use crate::shaders::{
    corona_shader_source, scene_shader_source, sky_shader_source, snow_shader_source,
};

impl Renderer {
    /// The pipelines, samplers and fixed textures of a renderer whose options are settled.
    pub(crate) fn build(
        device: wgpu::Device,
        queue: wgpu::Queue,
        adapter_name: String,
        format: wgpu::TextureFormat,
        options: RenderOptions,
    ) -> Renderer {
        let (msaa, shadow_size) = (options.msaa, options.shadow_size);
        RT_GBUF.store(options.ray_tracing, std::sync::atomic::Ordering::Relaxed);
        let gpu = GpuContext::open(&device, format, msaa, options.ray_tracing);
        if omsi_cfg::env::var("OMSI_FAKE_GPU_ERROR").as_deref() == Ok("build") && msaa > 1 {
            // test hook for the fallback in `new_with`: a sample count no device takes
            let _ = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("invalid"),
                size: wgpu::Extent3d {
                    width: 4,
                    height: 4,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 3,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
        }
        // One module for both paths: the enhanced fragment shader shares the vertex shader,
        // which the depth prepass relies on to the last bit (see `VsOut::clip`).
        log::info!("renderer: compiling the scene shaders");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("omsi"),
            source: wgpu::ShaderSource::Wgsl(
                scene_shader_source(GL_BACKEND.load(std::sync::atomic::Ordering::Relaxed)).into(),
            ),
        });
        let layouts = Layouts::new(&device);
        let light_map = LightMapAtlas::new(&device);
        let snow_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("snow"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let sprites = SpriteResources::new(&device, &queue, &snow_layout);
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("omsi"),
            bind_group_layouts: &[Some(&layouts.camera_layout), Some(&layouts.material_layout)],
            immediate_size: 0,
        });
        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2],
        };
        let shadows = ShadowResources::new(
            &device,
            shadow_size,
            &shader,
            &layouts.material_layout,
            &vertex_layout,
        );
        let make = |format: wgpu::TextureFormat,
                    fs: &str,
                    blend: Option<wgpu::BlendState>,
                    depth_write: bool,
                    cull: bool,
                    bias: i32,
                    alpha_to_coverage: bool,
                    terrain_paint: bool,
                    samples: u32| {
            let use_alpha_to_coverage = alpha_to_coverage && samples > 1;
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("omsi"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[vertex_layout.clone()],
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
                    module: &shader,
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
                        fs == "fs_enhanced",
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
        };
        let bias: i32 = omsi_cfg::env::var("OMSI_SURFACE_BIAS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(-24);
        // one pipeline per `pipe_code`: the kind decides blending and the depth write
        let scene_pipelines =
            |f: wgpu::TextureFormat, fs: &str, samples: u32| -> Vec<wgpu::RenderPipeline> {
                let mut out = Vec::with_capacity(PIPE_KINDS as usize * 4);
                for kind in 0..PIPE_KINDS {
                    let blend =
                        matches!(kind, PIPE_BLEND | PIPE_BLEND_NO_WRITE | PIPE_TERRAIN_PAINT)
                            .then_some(wgpu::BlendState::ALPHA_BLENDING);
                    let depth_write = kind != PIPE_BLEND_NO_WRITE && kind != PIPE_TERRAIN_PAINT;
                    for cull in [false, true] {
                        for surface in [false, true] {
                            out.push(make(
                                f,
                                if kind == PIPE_SURFACE_DEPTH {
                                    "fs_surface_depth"
                                } else {
                                    fs
                                },
                                blend,
                                depth_write,
                                cull,
                                if surface { bias } else { 0 },
                                kind == PIPE_ALPHA_TEST,
                                kind == PIPE_TERRAIN_PAINT && fs == "fs_enhanced",
                                samples,
                            ));
                        }
                    }
                }
                out
            };
        let hdr_format = wgpu::TextureFormat::Rgba16Float;
        let samplers = Samplers::new(&device, options.anisotropy);
        let camera_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera"),
            size: std::mem::size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let assets = AssetStore::new(&device, &queue);
        log::info!("renderer: compiling the coronas shaders");
        let corona_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("corona"),
            source: wgpu::ShaderSource::Wgsl(corona_shader_source().into()),
        });
        let corona_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("corona"),
            bind_group_layouts: &[Some(&layouts.camera_layout), Some(&sprites.corona_layout)],
            immediate_size: 0,
        });
        let corona_vertex = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<GpuCorona>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32, 2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4],
        };
        let additive = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent::REPLACE,
        };
        // Omsi's lamp sprites: SRCBLEND ONE, DESTBLEND INVSRCCOLOR (src + dst * (1 - src)),
        // which keeps a coloured sprite's hue over a lit background instead of washing it to white
        let screen = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrc,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent::REPLACE,
        };
        let alpha_blend = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent::REPLACE,
        };
        // the snowfall (snow.wgsl): the camera group and its own parameters, no vertices
        let snow_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("snow"),
            source: wgpu::ShaderSource::Wgsl(snow_shader_source().into()),
        });
        let snow_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("snow"),
            bind_group_layouts: &[Some(&layouts.camera_layout), Some(&snow_layout)],
            immediate_size: 0,
        });
        let premultiplied_snow = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent::REPLACE,
        };
        let snow_pipeline_for = |f: wgpu::TextureFormat, fs: &str| {
            log::info!("renderer: compiling the snowfall shader ({fs})");
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("snow"),
                layout: Some(&snow_pl),
                vertex: wgpu::VertexState {
                    module: &snow_shader,
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
                    count: msaa,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &snow_shader,
                    entry_point: Some(fs),
                    targets: &color_targets(
                        f,
                        Some(premultiplied_snow),
                        wgpu::ColorWrites::COLOR,
                        false,
                        false,
                    ),
                    compilation_options: Default::default(),
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let corona_pipeline_for = |f: wgpu::TextureFormat, fs: &str, blend: wgpu::BlendState| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("corona"),
                layout: Some(&corona_pl),
                vertex: wgpu::VertexState {
                    module: &corona_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[corona_vertex.clone()],
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
                    count: msaa,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &corona_shader,
                    entry_point: Some(fs),
                    targets: &color_targets(f, Some(blend), wgpu::ColorWrites::COLOR, false, false),
                    compilation_options: Default::default(),
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        log::info!("renderer: compiling the sky and clouds shaders");
        let sky_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sky"),
            source: wgpu::ShaderSource::Wgsl(sky_shader_source().into()),
        });
        let sky_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sky"),
            bind_group_layouts: &[Some(&layouts.camera_layout), Some(&layouts.sky_layout)],
            immediate_size: 0,
        });
        let sky_vertex = wgpu::VertexBufferLayout {
            array_stride: 12,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3],
        };
        let sky_pipeline_for = |f: wgpu::TextureFormat, fs: &str| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("sky"),
                layout: Some(&sky_pl),
                vertex: wgpu::VertexState {
                    module: &sky_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[sky_vertex.clone()],
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
                    module: &sky_shader,
                    entry_point: Some(fs),
                    targets: &color_targets(f, None, wgpu::ColorWrites::COLOR, false, false),
                    compilation_options: Default::default(),
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let pass = PassPipelines {
            pipelines: scene_pipelines(format, "fs_main", msaa),
            rain_pipelines: scene_pipelines(format, "fs_main", 1),
            corona_pipeline: corona_pipeline_for(format, "fs_main", screen),
            smoke_pipeline: corona_pipeline_for(format, "fs_smoke", alpha_blend),
            snow_pipeline: (!basic_pipelines()).then(|| snow_pipeline_for(format, "fs_snow")),
            sky_pipeline: sky_pipeline_for(format, "fs_main"),
        };
        // the enhanced path: its own lighting in all three
        let leave_out_enhanced = options.no_enhanced
            && (cfg!(target_os = "android")
                || adapter_name.to_ascii_lowercase().contains("opengl")
                || GL_BACKEND.load(std::sync::atomic::Ordering::Relaxed));
        // (its textures do not fit OpenGL's units here, see `sixteen_texture_units`)
        if sixteen_texture_units() && !options.no_enhanced {
            log::warn!("renderer: the enhanced graphics take more textures than OpenGL has units for on {adapter_name}; drawing vanilla+");
        }
        let leave_out_enhanced = leave_out_enhanced || sixteen_texture_units();
        let sky_mirror_pipeline =
            (!leave_out_enhanced).then(|| sky_pipeline_for(format, "fs_enhanced_mirror"));
        let hdr_pass = (!leave_out_enhanced).then(|| PassPipelines {
            pipelines: scene_pipelines(hdr_format, "fs_enhanced", msaa),
            rain_pipelines: scene_pipelines(hdr_format, "fs_enhanced", 1),
            corona_pipeline: corona_pipeline_for(hdr_format, "fs_enhanced", additive),
            smoke_pipeline: corona_pipeline_for(hdr_format, "fs_smoke_enhanced", alpha_blend),
            snow_pipeline: (!basic_pipelines())
                .then(|| snow_pipeline_for(hdr_format, "fs_snow_enhanced")),
            sky_pipeline: sky_pipeline_for(hdr_format, "fs_enhanced"),
        });
        let reflection_pass = (!leave_out_enhanced
            && !GL_BACKEND.load(std::sync::atomic::Ordering::Relaxed))
        .then(|| PassPipelines {
            pipelines: scene_pipelines(hdr_format, "fs_vanilla_reflections", msaa),
            rain_pipelines: scene_pipelines(hdr_format, "fs_vanilla_reflections", 1),
            corona_pipeline: corona_pipeline_for(hdr_format, "fs_main", screen),
            smoke_pipeline: corona_pipeline_for(hdr_format, "fs_smoke", alpha_blend),
            snow_pipeline: (!basic_pipelines()).then(|| snow_pipeline_for(hdr_format, "fs_snow")),
            sky_pipeline: sky_pipeline_for(hdr_format, "fs_main"),
        });
        let presenter = Presenter::new(&device, format, msaa);
        let gl = GL_BACKEND.load(std::sync::atomic::Ordering::Relaxed);
        let ssao = ao_pipelines(&device, gl);
        let fog_lamps = fog_lamps_pipelines(&device, &layouts.camera_layout, gl, hdr_format);
        let prepass = prepass_pipelines(
            &device,
            &layouts.camera_layout,
            &layouts.material_layout,
            msaa,
            &shader,
            &vertex_layout,
        );
        let post = post_pipelines(&device, format, hdr_format);
        let adaptation = Exposure::new(&device, &post, &assets.white_texture, hdr_format);
        // --- enhanced lighting: the uniform, the sky table, the reflection probe
        let enh_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("enhanced lighting"),
            size: std::mem::size_of::<EnhancedUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sky = SkyResources::new(
            &device,
            &queue,
            &layouts.sky_layout,
            &sky_shader,
            &camera_buf,
            &enh_buf,
            &samplers.lin_sampler,
            hdr_format,
        );
        let diagnostics = Diagnostics::new(&device);
        // GLES cannot reliably read the depth buffer (the same limitation as SSAO).
        let puddles = (!gl && !leave_out_enhanced).then(|| {
            puddles::Pipelines::new(
                &device,
                &shader,
                &layouts.camera_layout,
                &layouts.material_layout,
            )
        });
        let rt = options.ray_tracing.then(|| rt::RayTracer::new(&device));
        Renderer {
            gpu,
            presenter,
            culling: CullHistory::new(),
            targets: TargetCache::new(),
            dynamic_scale: std::cell::Cell::new(1.0),
            shadows,
            assets,
            samplers,
            address_next: std::cell::Cell::new(TexAddressing::Wrap),
            light_map_next: std::cell::Cell::new(false),
            light_map,
            puddles,
            post,
            adaptation,
            diagnostics,
            enh_buf,
            sky,
            texture_aspect: None,
            last_frame: None,
            instant_exposure: false,
            // (OMSI_RENDER_CLOCK=<s>: the animations' clock starts that far on - offscreen
            // stills of moving things, the windy trees, at different moments)
            started: std::time::Instant::now()
                .checked_sub(std::time::Duration::from_secs_f64(
                    omsi_cfg::env::var("OMSI_RENDER_CLOCK")
                        .ok()
                        .and_then(|v| v.trim().parse::<f64>().ok())
                        .filter(|s| s.is_finite() && *s >= 0.0)
                        .unwrap_or(0.0),
                ))
                .unwrap_or_else(std::time::Instant::now),
            tree_gust_drift: std::cell::Cell::new((glam::DVec2::ZERO, 0.0)),
            ssao,
            layouts,
            prepass,
            fog_lamps,
            device,
            queue,
            adapter_name,
            pass,
            hdr_pass,
            sky_mirror_pipeline,
            reflection_pass,
            sprites,
            camera_buf,
            shadow_blobs: options.shadow_blobs,
            options,
            env_heading: Default::default(),
            blend_by_origin: false,
            stats: Default::default(),
            counts: Default::default(),
            rt,
        }
    }
}
