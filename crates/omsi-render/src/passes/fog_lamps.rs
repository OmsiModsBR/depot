//! The lamps' light in the fog: worked out at half size and added onto the picture.

use crate::gpu::{array_path, basic_pipelines, pass_timer, ArrayPath};
use crate::lighting::Lighting;
use crate::renderer::Renderer;
use crate::scene::Scene;
use crate::shaders::fog_lamps_shader_source;
use crate::sky::enhanced_weather_fog;
use glam::Mat4;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct FogLampUniform {
    pub(crate) inv_view_proj: [[f32; 4]; 4],
    pub(crate) size: [f32; 4],
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

impl Renderer {
    /// The lamps' light in the weather's fog and the sun's shafts between the shadows, over the
    /// picture before its glow and metering.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn encode_fog_lamps(
        &self,
        scene: &Scene,
        encoder: &mut wgpu::CommandEncoder,
        lighting: &Lighting,
        vp_mat: Mat4,
        width: u32,
        height: u32,
        enhanced: bool,
        masked_frame: bool,
        prepass_on: bool,
        puddles_on: bool,
        with_overlays: bool,
        tset: &Option<wgpu::QuerySet>,
        timed: &mut Vec<&'static str>,
    ) {
        // the lamps' light in the weather's fog and the sun's shafts between the shadows
        // (before the rain on the panes, which writes the prepass depth),
        // over the picture before its glow and metering (the window's picture; the mirrors
        // go without)
        let fog_lamps = enhanced_weather_fog(lighting) >= 2e-4 && !scene.lights.is_empty();
        let shafts = lighting.shadows && lighting.sun_dir.z > 0.0;
        let glare = lighting.sun_dir.z > -0.01
            && lighting.sun_intensity > 0.0
            && omsi_cfg::env::var_os("OMSI_NO_GLARE").is_none();
        if omsi_cfg::env::var_os("OMSI_DEBUG_FOG_LAMPS").is_some() {
            log::info!("fog lamps: enhanced {enhanced} prepass {prepass_on} overlays {with_overlays} masked {masked_frame} fog {:.5} lights {} shafts {shafts} glare {glare}", enhanced_weather_fog(lighting), scene.lights.len());
        }
        if enhanced && prepass_on && with_overlays && masked_frame && (fog_lamps || shafts || glare)
        {
            if let (Some(pipes), Some(ao), Some(cam_bg)) = (
                self.fog_lamps_pipeline.as_ref(),
                self.targets.ao.as_ref(),
                scene.camera_bind_group.as_ref(),
            ) {
                if let Some(fog_bg) = ao.fog_bg.as_ref() {
                    // (how much of the weather's extinction is mist and fog - droplets of some
                    // ten micrometres, which scatter a lamp's light into a halo and its beam
                    // into a cone - rather than rain: a raindrop of a millimetre sends what it
                    // scatters on within a few hundredths of a degree, no halo round a lamp,
                    // no cone of a headlight in a drizzle (the rain's share as lights.rs
                    // `apply_weather` lays it on))
                    let fog_all = enhanced_weather_fog(lighting);
                    let rain_part = if lighting.rain > 0.0 && lighting.snowfall <= 0.0 {
                        2.3 / (2500.0 - 1800.0 * lighting.rain.clamp(0.0, 1.0))
                    } else {
                        0.0
                    };
                    let droplets = if fog_all > 0.0 {
                        ((fog_all - rain_part) / fog_all).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    let u = FogLampUniform {
                        inv_view_proj: vp_mat.inverse().to_cols_array_2d(),
                        size: [
                            width as f32,
                            height as f32,
                            if glare { 1.0 } else { 0.0 },
                            droplets,
                        ],
                    };
                    self.queue
                        .write_buffer(&self.fog_lamps_buf, 0, bytemuck::bytes_of(&u));
                    let h = &self.targets.hdr_targets[&(width, height)];
                    let view = h
                        .puddles
                        .as_ref()
                        .filter(|_| puddles_on)
                        .map_or(&h.view, |p| &p.view);
                    for (k, (target, load)) in [
                        (&ao.fog_view, wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)),
                        (view, wgpu::LoadOp::Load),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("fog lamps"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: target,
                                depth_slice: None,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load,
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: None,
                            timestamp_writes: if k == 1 {
                                pass_timer(tset.as_ref(), timed, "fog lamps")
                            } else {
                                None
                            },
                            occlusion_query_set: None,
                            multiview_mask: None,
                        });
                        pass.set_pipeline(&pipes[k]);
                        pass.set_bind_group(0, cam_bg, &[]);
                        pass.set_bind_group(1, &fog_bg[k], &[]);
                        pass.draw(0..3, 0..1);
                    }
                }
            }
        }
    }
}
