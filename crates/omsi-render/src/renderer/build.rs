//! Making everything the renderer draws with, once at start (`Renderer::build`).
use crate::assets::AssetStore;
use crate::gpu::GpuContext;
use crate::gpu::Layouts;
use crate::gpu::Samplers;
use crate::lighting::LightMapAtlas;
use crate::output::Diagnostics;
use crate::output::Presenter;
use crate::passes::Exposure;
use crate::passes::ShadowResources;
use crate::scene::CullHistory;
use crate::sky::SkyResources;
use crate::targets::TargetCache;

use super::options::RenderOptions;
use super::state::{color_targets, Renderer, DEPTH_FORMAT, RT_GBUF};
use crate::assets::{material_layout_entries, mip_pipeline, upload_texture, TexAddressing};
use crate::effects::{puddles, rt};
use crate::gpu::{
    array_layout_entry, array_path, basic_pipelines, buffer_init, gpu_error_text,
    sixteen_texture_units, DevicePoller, GL_BACKEND,
};
use crate::lighting::{ExposureLog, GpuCorona, LM_ATLAS_TILES, LM_TILE_PX};
use crate::output::GpuTimers;
use crate::output::{overlay_1x_pipelines, overlay_pipeline, upscale_pipelines};
use crate::passes::{
    ao_pipelines, fog_lamps_pipelines, post_pipelines, prepass_pipelines, shadow_pipelines,
};
use crate::passes::{
    float_tex, one_sided_primitive, PassPipelines, PIPE_ALPHA_TEST, PIPE_BLEND,
    PIPE_BLEND_NO_WRITE, PIPE_KINDS, PIPE_SURFACE_DEPTH, PIPE_TERRAIN_PAINT,
};
use crate::scene::{camera_layout_entries, CameraUniform, Vertex};
use crate::shaders::{
    cloud_noise_textures, corona_shader_source, scene_shader_source, sky_shader_source,
    snow_shader_source, SnowUniform,
};
use crate::sky::{
    atmosphere, EnhancedUniform, Probe, PROBE_MIPS, PROBE_SIZE, SKY_CUBE_ROUNDS, SKY_CUBE_SIZE,
};
use glam::{DVec3, Mat4, Vec3};
use std::collections::HashMap;
use std::sync::Arc;

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
        // A GPU error while multisampling is on is logged and switches multisampling off
        // at the next frame (`render_inner`). Otherwise it is logged and the game goes on:
        // wgpu's own handler ends the process, and one call a driver refused (a limit of
        // that card, a bigger map than the last) closed the game a few seconds into the
        // drive - a wrong picture for a frame is better than no game. The first errors and
        // then every thousandth reach the log.
        let gpu_error = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let rt_error = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let out_of_memory = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let device_lost: Arc<std::sync::Mutex<Option<String>>> = Default::default();
        {
            let lost = device_lost.clone();
            device.set_device_lost_callback(move |reason, message| {
                // (dropping the device at the end also calls this: that is no loss)
                if matches!(reason, wgpu::DeviceLostReason::Destroyed) {
                    return;
                }
                log::error!("the graphics device was lost ({reason:?}): {message}");
                *lost.lock().unwrap_or_else(|e| e.into_inner()) =
                    Some(format!("{reason:?}: {message}"));
            });
        }
        {
            let flag = gpu_error.clone();
            let rt_flag = rt_error.clone();
            let rt_on = options.ray_tracing;
            let oom = out_of_memory.clone();
            let count = Arc::new(std::sync::atomic::AtomicU64::new(0));
            device.on_uncaptured_error(Arc::new(move |e: wgpu::Error| {
                if matches!(e, wgpu::Error::OutOfMemory { .. }) {
                    oom.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                let n = count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if rt_on {
                    // (the ray tracing goes first: the likelier cause, and the dearer feature)
                    if !rt_flag.swap(true, std::sync::atomic::Ordering::Relaxed) {
                        log::error!("GPU error with ray tracing (Enhanced+ draws as Enhanced from now on): {}", gpu_error_text(&e));
                    }
                } else if msaa > 1 && !flag.swap(true, std::sync::atomic::Ordering::Relaxed) {
                    log::error!("GPU error with {msaa}x MSAA (drawing without it from now on): {}", gpu_error_text(&e));
                } else if n < 20 || n % 1000 == 0 {
                    log::error!("GPU error #{} (the game goes on): {}", n + 1, gpu_error_text(&e));
                }
            }));
        }
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
        let shadow_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow camera"),
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
                array_layout_entry(1, wgpu::ShaderStages::VERTEX, true),
                array_layout_entry(2, wgpu::ShaderStages::VERTEX, true),
                array_layout_entry(10, wgpu::ShaderStages::VERTEX, false),
            ],
        });
        let camera_entries = camera_layout_entries(array_path(), sixteen_texture_units());
        let camera_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera"),
            entries: &camera_entries,
        });
        let lm_atlas = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("light map atlas"),
            size: wgpu::Extent3d {
                width: LM_ATLAS_TILES * LM_TILE_PX,
                height: LM_ATLAS_TILES * LM_TILE_PX,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let lm_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("light map atlas place"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let material_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("material"),
            entries: &material_layout_entries(),
        });
        // coronas: camera group + a corona texture group
        let corona_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("corona"),
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
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("omsi"),
            bind_group_layouts: &[Some(&camera_layout), Some(&material_layout)],
            immediate_size: 0,
        });
        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2],
        };
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
        let (shadow_view, shadow_view_far, shadow_sampler, shadow_clear_pipeline, shadow_pipelines) =
            shadow_pipelines(
                &device,
                shadow_size,
                &shader,
                &shadow_layout,
                &material_layout,
                &vertex_layout,
            );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: options.anisotropy,
            ..Default::default()
        });
        // [matl_texadress_clamp]: a number plate is a small quad whose texture must not
        // repeat beyond its edge - repeated, the plate text tiled the whole rear of the bus
        let clamp_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: options.anisotropy,
            ..Default::default()
        });
        let mirror_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::MirrorRepeat,
            address_mode_v: wgpu::AddressMode::MirrorRepeat,
            address_mode_w: wgpu::AddressMode::MirrorRepeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: options.anisotropy,
            ..Default::default()
        });
        let camera_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera"),
            size: std::mem::size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let white = omsi_texture::Image::solid([255, 255, 255, 255]);
        let white_texture = upload_texture(&device, &queue, &white, false);
        let black_texture = upload_texture(
            &device,
            &queue,
            &omsi_texture::Image::solid([0, 0, 0, 255]),
            false,
        );
        let flat_normal_texture = upload_texture(
            &device,
            &queue,
            &omsi_texture::Image::solid([128, 128, 255, 255]),
            false,
        );
        // corona sprite: soft radial falloff
        let cs = 64u32;
        let mut corona_img = omsi_texture::Image {
            width: cs,
            height: cs,
            rgba: vec![0; (cs * cs * 4) as usize],
            has_alpha: false,
        };
        for y in 0..cs {
            for x in 0..cs {
                let dx = (x as f32 + 0.5) / cs as f32 * 2.0 - 1.0;
                let dy = (y as f32 + 0.5) / cs as f32 * 2.0 - 1.0;
                let r = (dx * dx + dy * dy).sqrt();
                let v = ((1.0 - r).max(0.0)).powf(1.6) * 255.0;
                let o = ((y * cs + x) * 4) as usize;
                corona_img.rgba[o..o + 4].copy_from_slice(&[v as u8, v as u8, v as u8, 255]);
            }
        }
        let corona_texture = upload_texture(&device, &queue, &corona_img, false);
        let corona_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let corona_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("corona"),
            layout: &corona_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&corona_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&corona_sampler),
                },
            ],
        });
        // a soft grey puff until the app hands over the game's own `Texture/rauch.tga`
        let mut puff = omsi_texture::Image {
            width: cs,
            height: cs,
            rgba: vec![0; (cs * cs * 4) as usize],
            has_alpha: true,
        };
        for y in 0..cs {
            for x in 0..cs {
                let dx = (x as f32 + 0.5) / cs as f32 * 2.0 - 1.0;
                let dy = (y as f32 + 0.5) / cs as f32 * 2.0 - 1.0;
                let a = (1.0 - (dx * dx + dy * dy).sqrt()).max(0.0).powf(1.2) * 255.0;
                let o = ((y * cs + x) * 4) as usize;
                puff.rgba[o..o + 4].copy_from_slice(&[255, 255, 255, a as u8]);
            }
        }
        let puff_texture = upload_texture(&device, &queue, &puff, false);
        let smoke_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("smoke"),
            layout: &corona_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&puff_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&corona_sampler),
                },
            ],
        });
        drop(puff_texture);
        log::info!("renderer: compiling the coronas shaders");
        let corona_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("corona"),
            source: wgpu::ShaderSource::Wgsl(corona_shader_source().into()),
        });
        let corona_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("corona"),
            bind_group_layouts: &[Some(&camera_layout), Some(&corona_layout)],
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
        let snow_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("snow params"),
            size: std::mem::size_of::<SnowUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let snow_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("snow"),
            layout: &snow_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: snow_buf.as_entire_binding(),
            }],
        });
        let snow_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("snow"),
            bind_group_layouts: &[Some(&camera_layout), Some(&snow_layout)],
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
        drop(corona_texture);
        // sky dome
        let sky_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sky"),
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
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D3,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let (cloud_shape_view, cloud_detail_view, cloud_sampler, cloud_shape_cpu) =
            cloud_noise_textures(&device, &queue);
        log::info!("renderer: compiling the sky and clouds shaders");
        let sky_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sky"),
            source: wgpu::ShaderSource::Wgsl(sky_shader_source().into()),
        });
        let sky_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sky"),
            bind_group_layouts: &[Some(&camera_layout), Some(&sky_layout)],
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
        let sky_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        // dome: latitude rings from a little below the horizon to the zenith
        let (mut sv, mut si): (Vec<[f32; 3]>, Vec<u32>) = (Vec::new(), Vec::new());
        let (rings, segs) = (12u32, 32u32);
        for r in 0..=rings {
            let elev = -0.15 + (std::f32::consts::FRAC_PI_2 + 0.15) * r as f32 / rings as f32;
            for sgm in 0..=segs {
                let az = sgm as f32 / segs as f32 * std::f32::consts::TAU;
                sv.push([elev.cos() * az.sin(), elev.cos() * az.cos(), elev.sin()]);
            }
        }
        for r in 0..rings {
            for sgm in 0..segs {
                let a = r * (segs + 1) + sgm;
                let b = a + segs + 1;
                si.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
            }
        }
        let sky_vb = buffer_init(
            &device,
            &queue,
            Some("sky vb"),
            bytemuck::cast_slice(&sv),
            wgpu::BufferUsages::VERTEX,
        );
        let sky_ib = buffer_init(
            &device,
            &queue,
            Some("sky ib"),
            bytemuck::cast_slice(&si),
            wgpu::BufferUsages::INDEX,
        );
        let sky_mesh = (sky_vb, sky_ib, si.len() as u32);
        let (overlay_layout, overlay_shader, overlay_pl, premul, overlay_pipeline) =
            overlay_pipeline(&device, format, msaa);
        let gl = GL_BACKEND.load(std::sync::atomic::Ordering::Relaxed);
        let (ao_layout, ao_buf, ao_sampler, ssao_pipeline, blur_pipeline) =
            ao_pipelines(&device, gl);
        let (fog_lamps_layout, fog_lamps_buf, fog_lamps_pipeline) =
            fog_lamps_pipelines(&device, &camera_layout, gl, hdr_format);
        let (prepass_pipelines, prepass_msaa_pipelines) = prepass_pipelines(
            &device,
            &camera_layout,
            &material_layout,
            msaa,
            &shader,
            &vertex_layout,
        );
        let (mip_layout, mip_pipeline, mip_sampler) = mip_pipeline(&device);
        let (post_layout, post_buf, post_sampler, post) =
            post_pipelines(&device, format, hdr_format);
        let one = wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        };
        let tiny = |label: &str| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: one,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: hdr_format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let meter_view = tiny("exposure meter");
        let adapt_views = [tiny("exposure a"), tiny("exposure b")];
        // adapt_bg[k] reads the meter and adapt[k] (and draws into the other one)
        let adapt_bg = [0usize, 1].map(|k| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("exposure"),
                layout: &post_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: post_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&meter_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&post_sampler),
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
        // --- enhanced lighting: the uniform, the sky table, the reflection probe
        let enh_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("enhanced lighting"),
            size: std::mem::size_of::<EnhancedUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sky_lut = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("sky table"),
            size: wgpu::Extent3d {
                width: atmosphere::SKY_LUT_W,
                height: atmosphere::SKY_LUT_H,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: hdr_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let sky_lut_view = sky_lut.create_view(&Default::default());
        let lin_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let uniform_entry = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let probe_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("probe"),
            entries: &[
                uniform_entry(0),
                uniform_entry(11),
                wgpu::BindGroupLayoutEntry {
                    binding: 13,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                float_tex(14),
                uniform_entry(15),
                wgpu::BindGroupLayoutEntry {
                    binding: 16,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let probe = {
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("reflection probe"),
                size: wgpu::Extent3d {
                    width: PROBE_SIZE,
                    height: PROBE_SIZE,
                    depth_or_array_layers: 6,
                },
                mip_level_count: PROBE_MIPS,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: hdr_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let cube = |base: u32, count: u32| {
                tex.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("probe cube"),
                    dimension: Some(wgpu::TextureViewDimension::Cube),
                    base_mip_level: base,
                    mip_level_count: Some(count),
                    base_array_layer: 0,
                    array_layer_count: Some(6),
                    ..Default::default()
                })
            };
            let view = cube(0, PROBE_MIPS);
            let faces: Vec<Vec<wgpu::TextureView>> = (0..PROBE_MIPS)
                .map(|m| {
                    (0..6)
                        .map(|f| {
                            tex.create_view(&wgpu::TextureViewDescriptor {
                                label: Some("probe face"),
                                dimension: Some(wgpu::TextureViewDimension::D2),
                                base_mip_level: m,
                                mip_level_count: Some(1),
                                base_array_layer: f,
                                array_layer_count: Some(1),
                                ..Default::default()
                            })
                        })
                        .collect()
                })
                .collect();
            // level 0 is drawn from the sky and reads nothing: a black cube stands in
            let dummy = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("probe placeholder"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 6,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: hdr_format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let dummy_view = dummy.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::Cube),
                ..Default::default()
            });
            let cube_tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("sky cube"),
                size: wgpu::Extent3d {
                    width: SKY_CUBE_SIZE,
                    height: SKY_CUBE_SIZE,
                    depth_or_array_layers: 6,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: hdr_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let cube_view = cube_tex.create_view(&wgpu::TextureViewDescriptor {
                label: Some("sky cube"),
                dimension: Some(wgpu::TextureViewDimension::Cube),
                ..Default::default()
            });
            let bind_groups = (0..PROBE_MIPS)
                .map(|m| {
                    // level 0 is the sky cube (clouds and all) looked up, the others the
                    // level above blurred
                    let src = if m == 0 {
                        cube_view.clone()
                    } else {
                        cube(0, m)
                    };
                    [0u32, 3].map(|first| {
                        let rough = m as f32 / (PROBE_MIPS - 1) as f32;
                        let buf = buffer_init(
                            &device,
                            &queue,
                            Some("probe pass"),
                            bytemuck::cast_slice(&[
                                first as f32,
                                rough,
                                PROBE_SIZE as f32,
                                m as f32,
                            ]),
                            wgpu::BufferUsages::UNIFORM,
                        );
                        device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("probe"),
                            layout: &probe_layout,
                            entries: &[
                                wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: camera_buf.as_entire_binding(),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 11,
                                    resource: enh_buf.as_entire_binding(),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 13,
                                    resource: wgpu::BindingResource::Sampler(&lin_sampler),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 14,
                                    resource: wgpu::BindingResource::TextureView(&sky_lut_view),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 15,
                                    resource: buf.as_entire_binding(),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 16,
                                    resource: wgpu::BindingResource::TextureView(&src),
                                },
                            ],
                        })
                    })
                })
                .collect();
            let probe_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("probe"),
                bind_group_layouts: &[Some(&probe_layout), Some(&sky_layout)],
                immediate_size: 0,
            });
            let target = Some(wgpu::ColorTargetState {
                format: hdr_format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            });
            let probe_pipeline = |entry: &str| {
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(entry),
                    layout: Some(&probe_pl),
                    vertex: wgpu::VertexState {
                        module: &sky_shader,
                        entry_point: Some("vs_probe"),
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
                        module: &sky_shader,
                        entry_point: Some(entry),
                        targets: &[target.clone(), target.clone(), target.clone()],
                        compilation_options: Default::default(),
                    }),
                    multiview_mask: None,
                    cache: None,
                })
            };
            let cube_faces: Vec<wgpu::TextureView> = (0..6)
                .map(|f| {
                    cube_tex.create_view(&wgpu::TextureViewDescriptor {
                        label: Some("sky cube face"),
                        dimension: Some(wgpu::TextureViewDimension::D2),
                        base_mip_level: 0,
                        mip_level_count: Some(1),
                        base_array_layer: f,
                        array_layer_count: Some(1),
                        ..Default::default()
                    })
                })
                .collect();
            // per face and redraw round: the round picks where the clouds' steps start
            let cube_bind_groups: Vec<wgpu::BindGroup> = (0..6 * SKY_CUBE_ROUNDS)
                .map(|k| {
                    let (f, round) = (k / SKY_CUBE_ROUNDS, k % SKY_CUBE_ROUNDS);
                    let buf = buffer_init(
                        &device,
                        &queue,
                        Some("sky cube face"),
                        bytemuck::cast_slice(&[f as f32, round as f32, SKY_CUBE_SIZE as f32, 0.0]),
                        wgpu::BufferUsages::UNIFORM,
                    );
                    device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("sky cube"),
                        layout: &probe_layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: camera_buf.as_entire_binding(),
                            },
                            wgpu::BindGroupEntry {
                                binding: 11,
                                resource: enh_buf.as_entire_binding(),
                            },
                            wgpu::BindGroupEntry {
                                binding: 13,
                                resource: wgpu::BindingResource::Sampler(&lin_sampler),
                            },
                            wgpu::BindGroupEntry {
                                binding: 14,
                                resource: wgpu::BindingResource::TextureView(&sky_lut_view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 15,
                                resource: buf.as_entire_binding(),
                            },
                            wgpu::BindGroupEntry {
                                binding: 16,
                                resource: wgpu::BindingResource::TextureView(&dummy_view),
                            },
                        ],
                    })
                })
                .collect();
            let cube_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("sky cube"),
                layout: Some(&probe_pl),
                vertex: wgpu::VertexState {
                    module: &sky_shader,
                    entry_point: Some("vs_probe"),
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
                    module: &sky_shader,
                    entry_point: Some("fs_sky_cube"),
                    // a redraw is blended into what the face holds (the blend constant is
                    // the old picture's share): the clouds' grain averages out
                    targets: &[Some(wgpu::ColorTargetState {
                        format: hdr_format,
                        blend: Some(wgpu::BlendState {
                            color: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::OneMinusConstant,
                                dst_factor: wgpu::BlendFactor::Constant,
                                operation: wgpu::BlendOperation::Add,
                            },
                            alpha: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::OneMinusConstant,
                                dst_factor: wgpu::BlendFactor::Constant,
                                operation: wgpu::BlendOperation::Add,
                            },
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                multiview_mask: None,
                cache: None,
            });
            Probe {
                view,
                faces,
                bind_groups,
                sky_pipeline: probe_pipeline("fs_probe_sky"),
                filter_pipeline: probe_pipeline("fs_probe_filter"),
                age: u32::MAX,
                scale: 1.0,
                cube_view,
                cube_faces,
                cube_bind_groups,
                cube_pipeline,
                cube_next: 0,
                cube_filled: false,
                cube_round: 0,
                cube_wait: 0,
                cube_eye: None,
                cube_recapture: false,
            }
        };
        let (overlay_pipeline_1x, xr_ui_pipeline) =
            overlay_1x_pipelines(&device, format, &overlay_pl, &overlay_shader, premul);
        let (upscale_layout, upscale_pipeline, copy_pipeline, panel_pipeline, upscale_buf) =
            upscale_pipelines(&device, format);
        let gpu_timers = [GpuTimers::new(&device), GpuTimers::new(&device)];
        // GLES cannot reliably read the depth buffer (the same limitation as SSAO).
        let puddles = (!gl && !leave_out_enhanced)
            .then(|| puddles::Pipelines::new(&device, &shader, &camera_layout, &material_layout));
        let rt = options.ray_tracing.then(|| rt::RayTracer::new(&device));
        Renderer {
            gpu: GpuContext {
                _device_poller: DevicePoller::start(&device),
                format,
                gpu_error,
                rt_error,
                out_of_memory,
                device_lost,
                encoding_pool: if omsi_cfg::env::var_os("OMSI_NO_RENDER_POOL").is_some() {
                    None
                } else {
                    let workers = std::thread::available_parallelism()
                        .map(|n| n.get() / 2)
                        .unwrap_or(2)
                        .clamp(2, 8);
                    rayon::ThreadPoolBuilder::new()
                        .num_threads(workers)
                        .thread_name(|i| format!("omsi-render-{i}"))
                        .build()
                        .ok()
                },
            },
            presenter: Presenter {
                upscale_pipeline,
                copy_pipeline,
                panel_pipeline,
                upscale_layout,
                upscale_buf,
                scale_targets: HashMap::new(),
                triple_targets: None,
                overlay_pipeline_1x,
                xr_ui_pipeline,
            },
            culling: CullHistory {
                triple_culling: Default::default(),
                flicker: std::cell::RefCell::new(HashMap::new()),
                cull_drawn: std::cell::RefCell::new(Vec::new()),
                object_sizes: Default::default(),
                object_sizes_scratch: Default::default(),
            },
            targets: TargetCache {
                glass_picture: None,
                target_use: HashMap::new(),
                hdr_targets: HashMap::new(),
                ao: None,
                depth: None,
                msaa_targets: HashMap::new(),
            },
            dynamic_scale: std::cell::Cell::new(1.0),
            shadows: ShadowResources {
                shadow_far_cache: std::cell::Cell::new((
                    Mat4::IDENTITY,
                    0,
                    DVec3::ZERO,
                    Vec3::ZERO,
                )),
                shadow_near_cache: std::cell::Cell::new((
                    Mat4::IDENTITY,
                    0,
                    DVec3::ZERO,
                    Vec3::ZERO,
                )),
                xr_shadow_cache: std::cell::Cell::new(None),
                shadow_clear_pipeline,
                shadow_view,
                shadow_view_far,
                shadow_sampler,
                shadow_layout,
                shadow_pipelines,
            },
            assets: AssetStore {
                mip_pipeline,
                mip_layout,
                mip_sampler,
                white_texture,
                black_texture,
                flat_normal_texture,
                pending_meshes: Default::default(),
                freed: std::cell::OnceCell::new(),
                mesh_pages: false,
            },
            samplers: Samplers {
                clamp_sampler,
                mirror_sampler,
                lin_sampler,
                sampler,
            },
            address_next: std::cell::Cell::new(TexAddressing::Wrap),
            light_map_next: std::cell::Cell::new(false),
            light_map: LightMapAtlas {
                lm_atlas_view: lm_atlas.create_view(&wgpu::TextureViewDescriptor::default()),
                lm_atlas,
                lm_uniform,
                lm_place: std::cell::Cell::new((0.0, 0.0, 0.0)),
            },
            puddles,
            post,
            post_layout,
            post_buf,
            post_sampler,
            adaptation: Exposure {
                meter_view,
                adapt_views,
                adapt_bg,
                adapt_front: 0,
                view_lamps: None,
                exposure: None,
            },
            diagnostics: Diagnostics {
                exposure_log: ExposureLog::new(&device),
                gpu_timers,
                profiling: omsi_cfg::env::var_os("OMSI_PROFILE").is_some(),
                draw_audit_at: std::time::Instant::now(),
            },
            enh_buf,
            sky: SkyResources {
                sky_lut,
                sky_lut_view,
                probe: Some(probe),
                sky_state: None,
                city_glow: None,
                sky_job: None,
                sky_sampler,
                cloud_shape_view,
                cloud_detail_view,
                cloud_sampler,
                cloud_shape_cpu,
                cloud_sun: None,
                sky_mesh,
            },
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
            ao_sampler,
            layouts: Layouts {
                ao_layout,
                camera_layout,
                material_layout,
                sky_layout,
            },
            ao_buf,
            prepass_pipelines,
            prepass_msaa_pipelines,
            ssao_pipeline,
            blur_pipeline,
            fog_lamps_pipeline,
            fog_lamps_layout,
            fog_lamps_buf,
            device,
            queue,
            adapter_name,
            pass,
            hdr_pass,
            sky_mirror_pipeline,
            reflection_pass,
            corona_bind_group,
            snow_buf,
            snow_bind_group,
            smoke_bind_group,
            corona_textures: Vec::new(),
            corona_layout,
            corona_sampler,
            overlay_pipeline,
            overlay_layout,
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
