//! The shadow pass: the sun's cascades and the street lamps' maps, and their pipelines.

use super::draw::{encode_batches, Batch, PIPE_ALPHA_TEST, PIPE_OPAQUE};
use crate::gpu::{basic_pipelines, pass_timer};
use crate::lighting::{LampShadow, FAR_MAP_ASPECT, LAMP_SHADOWS};
use crate::renderer::{Renderer, DEPTH_FORMAT, SHADOW_CLOSE_MAX};
use crate::scene::Scene;

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

impl Renderer {
    /// The sun's shadow maps (the near and far cascades, the close one beside the near) and
    /// the street lamps' maps under the far one.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn encode_shadow_maps(
        &self,
        scene: &Scene,
        shadow_encoder: &mut wgpu::CommandEncoder,
        shadow_batches: &[Vec<Batch>; 4],
        lamp_shadows: &[LampShadow],
        draw_shadows: bool,
        redraw_near: bool,
        redraw_far: bool,
        tset: &Option<wgpu::QuerySet>,
        timed: &mut Vec<&'static str>,
    ) {
        for cascade in [0usize, 1] {
            if !draw_shadows || (cascade == 1 && !redraw_far) {
                continue;
            }
            let view = if cascade == 0 {
                &self.shadow_view
            } else {
                &self.shadow_view_far
            };
            let keep_near = cascade == 0 && !redraw_near;
            let mut pass = shadow_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view,
                    depth_ops: Some(wgpu::Operations {
                        load: if keep_near {
                            wgpu::LoadOp::Load
                        } else {
                            wgpu::LoadOp::Clear(1.0)
                        },
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: pass_timer(
                    tset.as_ref(),
                    timed,
                    if cascade == 0 {
                        "shadow near"
                    } else {
                        "shadow far"
                    },
                ),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, scene.shadow_bind_group.as_ref().unwrap(), &[]);
            if cascade == 0 {
                // the atlas: near cascade on the left half, close cascade on the right
                let sz = self.options.shadow_size as f32;
                pass.set_viewport(0.0, 0.0, sz, sz, 0.0, 1.0);
                encode_batches(&mut pass, scene, &shadow_batches[0], |pipe| {
                    &self.shadow_pipelines[pipe as usize]
                });
                let csz = self.options.shadow_size.min(SHADOW_CLOSE_MAX) as f32;
                pass.set_viewport(sz, 0.0, csz, csz, 0.0, 1.0);
                if keep_near {
                    // (the near half is last frame's: only the close part is cleared)
                    pass.set_pipeline(&self.shadow_clear_pipeline);
                    pass.draw(0..3, 0..1);
                    pass.set_bind_group(0, scene.shadow_bind_group.as_ref().unwrap(), &[]);
                }
                encode_batches(&mut pass, scene, &shadow_batches[2], |pipe| {
                    &self.shadow_pipelines[4 + pipe as usize]
                });
            } else {
                let sz = self.options.shadow_size as f32;
                pass.set_viewport(0.0, 0.0, sz, sz, 0.0, 1.0);
                encode_batches(&mut pass, scene, &shadow_batches[cascade], |pipe| {
                    &self.shadow_pipelines[cascade * 2 + pipe as usize]
                });
            }
        }
        // the street lamps' maps: tiles of a quarter of the shadow size under the far map,
        // each cleared and drawn every frame
        if !lamp_shadows.is_empty() {
            let mut pass = shadow_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("lamp shadows"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_view_far,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: pass_timer(tset.as_ref(), timed, "lamp shadows"),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let sz = self.options.shadow_size as f32;
            let tile = sz / LAMP_SHADOWS as f32;
            for k in 0..lamp_shadows.len() {
                pass.set_viewport(k as f32 * tile, sz, tile, tile, 0.0, 1.0);
                pass.set_pipeline(&self.shadow_clear_pipeline);
                pass.draw(0..3, 0..1);
                pass.set_bind_group(0, scene.shadow_bind_group.as_ref().unwrap(), &[]);
                encode_batches(&mut pass, scene, &shadow_batches[3], |pipe| {
                    &self.shadow_pipelines[6 + 2 * k + pipe as usize]
                });
            }
        }
    }
}
