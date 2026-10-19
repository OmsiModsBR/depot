//! The sky's GPU resources: the sky table, the clouds' noise, the reflection probe and the
//! sky last computed.

use super::probe::{Probe, PROBE_MIPS, PROBE_SIZE, SKY_CUBE_ROUNDS, SKY_CUBE_SIZE};
use super::{atmosphere, clouds};
use crate::gpu::{
    buffer_init, float_tex, fullscreen_pipeline, sampler_entry, target_2d, uniform_entry,
};

/// The sky's GPU resources and the sky last computed.
pub(crate) struct SkyResources {
    pub(crate) sky_sampler: wgpu::Sampler,
    /// The enhanced clouds' noise (clouds.rs): the shape map, the detail volume and their
    /// repeating, mip-mapped sampler (sky bind group bindings 6-8).
    pub(crate) cloud_shape_view: wgpu::TextureView,
    pub(crate) cloud_detail_view: wgpu::TextureView,
    pub(crate) cloud_sampler: wgpu::Sampler,
    /// The shape map's second level (RGBA8), for the clouds' shadow on the street, and how
    /// much of the sun the clouds let through over the camera now (smoothed).
    pub(crate) cloud_shape_cpu: Vec<u8>,
    pub(crate) cloud_sun: Option<f32>,
    pub(crate) sky_mesh: (wgpu::Buffer, wgpu::Buffer, u32),
    pub(crate) sky_lut: wgpu::Texture,
    pub(crate) sky_lut_view: wgpu::TextureView,
    pub(crate) probe: Option<Probe>,
    pub(crate) sky_state: Option<atmosphere::SkyState>,
    /// How much the lamps round the camera light the night sky, relative to a city's
    /// (`lamp_sky_glow`), as the sky is computed with it.
    pub(crate) city_glow: Option<f32>,
    /// A sky being computed on a helper thread, for this input.
    pub(crate) sky_job: Option<(
        atmosphere::SkyInput,
        std::sync::mpsc::Receiver<atmosphere::SkyState>,
    )>,
}

impl SkyResources {
    /// The sky's sampler, the clouds' noise, the dome, the sky table and the reflection probe
    /// (drawn by `sky_shader`).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        sky_layout: &wgpu::BindGroupLayout,
        sky_shader: &wgpu::ShaderModule,
        camera_buf: &wgpu::Buffer,
        enh_buf: &wgpu::Buffer,
        lin_sampler: &wgpu::Sampler,
        hdr_format: wgpu::TextureFormat,
    ) -> Self {
        let (cloud_shape_view, cloud_detail_view, cloud_sampler, cloud_shape_cpu) =
            cloud_noise_textures(device, queue);
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
            device,
            queue,
            Some("sky vb"),
            bytemuck::cast_slice(&sv),
            wgpu::BufferUsages::VERTEX,
        );
        let sky_ib = buffer_init(
            device,
            queue,
            Some("sky ib"),
            bytemuck::cast_slice(&si),
            wgpu::BufferUsages::INDEX,
        );
        let sky_mesh = (sky_vb, sky_ib, si.len() as u32);
        let sky_lut = target_2d(
            device,
            "sky table",
            wgpu::Extent3d {
                width: atmosphere::SKY_LUT_W,
                height: atmosphere::SKY_LUT_H,
                depth_or_array_layers: 1,
            },
            1,
            hdr_format,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );
        let sky_lut_view = sky_lut.create_view(&Default::default());
        let probe_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("probe"),
            entries: &[
                uniform_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT),
                uniform_entry(11, wgpu::ShaderStages::VERTEX_FRAGMENT),
                sampler_entry(13),
                float_tex(14),
                uniform_entry(15, wgpu::ShaderStages::VERTEX_FRAGMENT),
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
            let dummy = target_2d(
                device,
                "probe placeholder",
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 6,
                },
                1,
                hdr_format,
                wgpu::TextureUsages::TEXTURE_BINDING,
            );
            let dummy_view = dummy.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::Cube),
                ..Default::default()
            });
            let cube_tex = target_2d(
                device,
                "sky cube",
                wgpu::Extent3d {
                    width: SKY_CUBE_SIZE,
                    height: SKY_CUBE_SIZE,
                    depth_or_array_layers: 6,
                },
                1,
                hdr_format,
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            );
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
                            device,
                            queue,
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
                                    resource: wgpu::BindingResource::Sampler(lin_sampler),
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
                bind_group_layouts: &[Some(&probe_layout), Some(sky_layout)],
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
                        module: sky_shader,
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
                        module: sky_shader,
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
                        device,
                        queue,
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
                                resource: wgpu::BindingResource::Sampler(lin_sampler),
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
            let cube_pipeline = fullscreen_pipeline(
                device,
                "sky cube",
                &probe_pl,
                sky_shader,
                "vs_probe",
                "fs_sky_cube",
                // a redraw is blended into what the face holds (the blend constant is
                // the old picture's share): the clouds' grain averages out
                wgpu::ColorTargetState {
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
                },
            );
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
        Self {
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
        }
    }
}

/// The enhanced clouds' noise textures (clouds.rs), made once: the shape map (2-D RGBA8)
/// and the detail volume (3-D R8), both with their mip chains, and a repeating sampler.
fn cloud_noise_textures(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (wgpu::TextureView, wgpu::TextureView, wgpu::Sampler, Vec<u8>) {
    let t0 = std::time::Instant::now();
    let (shape, detail) = std::thread::scope(|s| {
        let a = s.spawn(clouds::shape_map);
        let b = s.spawn(clouds::detail_volume);
        (
            a.join().expect("cloud shape"),
            b.join().expect("cloud detail"),
        )
    });
    let make = |label: &str,
                size: u32,
                dim: wgpu::TextureDimension,
                format: wgpu::TextureFormat,
                bpp: u32,
                levels: &[Vec<u8>]| {
        let depth = if dim == wgpu::TextureDimension::D3 {
            size
        } else {
            1
        };
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: depth,
            },
            mip_level_count: levels.len() as u32,
            sample_count: 1,
            dimension: dim,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (m, data) in levels.iter().enumerate() {
            let e = (size >> m).max(1);
            let d = if dim == wgpu::TextureDimension::D3 {
                e
            } else {
                1
            };
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &tex,
                    mip_level: m as u32,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(e * bpp),
                    rows_per_image: Some(e),
                },
                wgpu::Extent3d {
                    width: e,
                    height: e,
                    depth_or_array_layers: d,
                },
            );
        }
        tex.create_view(&wgpu::TextureViewDescriptor::default())
    };
    let shape_view = make(
        "cloud shape",
        clouds::SHAPE_SIZE,
        wgpu::TextureDimension::D2,
        wgpu::TextureFormat::Rgba8Unorm,
        4,
        &shape,
    );
    let detail_view = make(
        "cloud detail",
        clouds::DETAIL_SIZE,
        wgpu::TextureDimension::D3,
        wgpu::TextureFormat::R8Unorm,
        1,
        &detail,
    );
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("cloud noise"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        address_mode_w: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        ..Default::default()
    });
    log::info!("cloud noise made in {:.2} s", t0.elapsed().as_secs_f32());
    // (the shape map's second level kept for the clouds' shadow, `cloud_sun_transmittance`)
    let cpu = shape.get(1).cloned().unwrap_or_default();
    (shape_view, detail_view, sampler, cpu)
}
