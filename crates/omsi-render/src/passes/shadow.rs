//! The shadow pass: the sun's cascades and the street lamps' maps, and their pipelines.

use super::draw::{encode_batches, Batch, PIPE_ALPHA_TEST, PIPE_OPAQUE};
use crate::gpu::{
    array_layout_entry, basic_pipelines, pass_timer, target_2d, uniform_entry, DEPTH_FORMAT,
};
use crate::lighting::{LampShadow, Lighting, FAR_MAP_ASPECT, LAMP_SHADOWS};
use crate::scene::Scene;
use glam::{DVec3, Mat4, Vec3};

/// Half size of the area around the camera covered by the near shadow cascade (m).
pub const SHADOW_RANGE: f32 = 140.0;

/// Half size of the far cascade (m): coarser, but reaches the whole visible street.
pub const SHADOW_RANGE_FAR: f32 = 700.0;

/// Half size of the close cascade around the camera (m). The near cascade's texel is
/// 280 m / 2048 = 14 cm, and a moving bus's shadow edge stepped from texel to texel: the
/// shadow trembled while driving. The close map (the right half of the near map's atlas)
/// has 3 cm texels for the bus and everything next to it.
pub const SHADOW_RANGE_CLOSE: f32 = 32.0;

/// The close cascade's map is drawn no bigger than this (texels a side), whatever the shadow
/// setting: 3 cm texels. At the 4096 setting it had been 1.6 cm, and drawing it was a third
/// of the shadow pass's 4.6 ms.
pub(crate) const SHADOW_CLOSE_MAX: u32 = 2048;

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
    let shadow_tex = target_2d(
        device,
        "shadow map",
        wgpu::Extent3d {
            // near cascade on the left, close cascade on the right
            width: shadow_size * 2,
            height: shadow_size,
            depth_or_array_layers: 1,
        },
        1,
        DEPTH_FORMAT,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
    );
    let shadow_view = shadow_tex.create_view(&Default::default());
    let shadow_tex_far = target_2d(
        device,
        "shadow map far",
        // (and the street lamps' tiles under it, see `LAMP_SHADOWS`)
        wgpu::Extent3d {
            width: shadow_size,
            height: (shadow_size as f32 * FAR_MAP_ASPECT) as u32,
            depth_or_array_layers: 1,
        },
        1,
        DEPTH_FORMAT,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
    );
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

impl ShadowResources {
    /// The sun's shadow maps (the near and far cascades, the close one beside the near) and
    /// the street lamps' maps under the far one.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn encode_shadow_maps(
        &self,
        shadow_size: u32,
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
                let sz = shadow_size as f32;
                pass.set_viewport(0.0, 0.0, sz, sz, 0.0, 1.0);
                encode_batches(&mut pass, scene, &shadow_batches[0], |pipe| {
                    &self.shadow_pipelines[pipe as usize]
                });
                let csz = shadow_size.min(SHADOW_CLOSE_MAX) as f32;
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
                let sz = shadow_size as f32;
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
            let sz = shadow_size as f32;
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

/// The sun's shadow maps, their pipelines, and the cascades as last drawn.
pub(crate) struct ShadowResources {
    pub(crate) shadow_view: wgpu::TextureView,
    pub(crate) shadow_view_far: wgpu::TextureView,
    pub(crate) shadow_sampler: wgpu::Sampler,
    pub(crate) shadow_layout: wgpu::BindGroupLayout,
    /// [near opaque, near alpha-tested, far opaque, far alpha-tested, close opaque, close
    /// alpha-tested], then the street lamps' (6 + 2 k + kind; not with `basic_pipelines`)
    pub(crate) shadow_pipelines: Vec<wgpu::RenderPipeline>,
    /// The far shadow cascade as last drawn: its light matrix, frames since, the render
    /// origin and the sun it was drawn for.
    pub(crate) shadow_far_cache: std::cell::Cell<(Mat4, u32, DVec3, Vec3)>,
    /// The same for the near cascade, drawn every other frame (see `render_inner`).
    pub(crate) shadow_near_cache: std::cell::Cell<(Mat4, u32, DVec3, Vec3)>,
    /// Shadow atlas matrices from the first OpenXR eye, reused by the second eye.
    pub(crate) xr_shadow_cache: std::cell::Cell<Option<(DVec3, Vec3, Mat4, Mat4, Mat4)>>,
    /// Depth-only pipeline that fills its viewport with the far depth: clears the close
    /// cascade's part of the atlas when the near part is kept from the frame before.
    pub(crate) shadow_clear_pipeline: wgpu::RenderPipeline,
}

impl ShadowResources {
    /// The shadow maps, their layout, sampler and pipelines.
    pub(crate) fn new(
        device: &wgpu::Device,
        shadow_size: u32,
        shader: &wgpu::ShaderModule,
        material_layout: &wgpu::BindGroupLayout,
        vertex_layout: &wgpu::VertexBufferLayout<'_>,
    ) -> Self {
        let shadow_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow camera"),
            entries: &[
                uniform_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT),
                array_layout_entry(1, wgpu::ShaderStages::VERTEX, true),
                array_layout_entry(2, wgpu::ShaderStages::VERTEX, true),
                array_layout_entry(10, wgpu::ShaderStages::VERTEX, false),
            ],
        });
        let (shadow_view, shadow_view_far, shadow_sampler, shadow_clear_pipeline, shadow_pipelines) =
            shadow_pipelines(
                device,
                shadow_size,
                shader,
                &shadow_layout,
                material_layout,
                vertex_layout,
            );
        Self {
            shadow_far_cache: std::cell::Cell::new((Mat4::IDENTITY, 0, DVec3::ZERO, Vec3::ZERO)),
            shadow_near_cache: std::cell::Cell::new((Mat4::IDENTITY, 0, DVec3::ZERO, Vec3::ZERO)),
            xr_shadow_cache: std::cell::Cell::new(None),
            shadow_clear_pipeline,
            shadow_view,
            shadow_view_far,
            shadow_sampler,
            shadow_layout,
            shadow_pipelines,
        }
    }
}

/// The sun's shadows of one picture (see `ShadowResources::schedule`).
#[derive(Clone, Copy)]
pub(crate) struct ShadowSchedule {
    /// By night the moon casts them (Enhanced).
    pub(crate) moon_shadows: bool,
    /// The picture has sun shadows at all, and draws its maps (not taken from the first eye).
    pub(crate) shadows: bool,
    pub(crate) draw_shadows: bool,
    /// The near and far cascades are drawn anew this frame.
    pub(crate) redraw_near: bool,
    pub(crate) redraw_far: bool,
    /// The near, far and close cascades' light matrices.
    pub(crate) light_view_proj: Mat4,
    pub(crate) light_view_proj_far: Mat4,
    pub(crate) light_view_proj_close: Mat4,
}

impl ShadowResources {
    /// Which cascades this picture draws and with which light matrices: the near one every
    /// other frame, the far one every fourth or when the camera has left it, the close one
    /// every frame; a headset's second eye takes the first eye's.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn schedule(
        &self,
        shadow_size: u32,
        render_origin: DVec3,
        lighting: &Lighting,
        cam_rel: Vec3,
        enhanced_frame: bool,
        with_overlays: bool,
        projection: Option<Mat4>,
        second_eye: bool,
    ) -> ShadowSchedule {
        // sun shadow map: an orthographic box around the camera, looking along the sun (by
        // night along the moon, see `Lighting::casts_moon_shadows`)
        let moon_shadows = enhanced_frame && lighting.casts_moon_shadows();
        let sun = if moon_shadows {
            lighting.moon_dir.normalize_or_zero()
        } else {
            lighting.sun_dir.normalize_or_zero()
        };
        let shadows = (with_overlays || projection.is_some())
            && (lighting.casts_sun_shadows() || moon_shadows);
        let shared_xr_shadows = if second_eye && shadows {
            self.xr_shadow_cache
                .get()
                .filter(|(origin, previous_sun, _, _, _)| {
                    *origin == render_origin && *previous_sun == sun
                })
        } else {
            None
        };
        let draw_shadows = shadows && shared_xr_shadows.is_none();
        let light_matrix = |range: f32| {
            // Snap the centre to whole texels so the map does not shimmer while driving.
            let texel = range * 2.0 / shadow_size as f32;
            let up = if sun.z.abs() > 0.95 { Vec3::Y } else { Vec3::Z };
            let raw = cam_rel;
            let view0 = Mat4::look_at_rh(sun * 900.0, Vec3::ZERO, up);
            let ls = view0.transform_point3(raw);
            let snapped = Vec3::new(
                (ls.x / texel).round() * texel,
                (ls.y / texel).round() * texel,
                ls.z,
            );
            let center = view0.inverse().transform_point3(snapped);
            let view = Mat4::look_at_rh(center + sun * 900.0, center, up);
            let proj = Mat4::orthographic_rh(-range, range, -range, range, 1.0, 2200.0);
            proj * view
        };
        // The near cascade (140 m, 4096 texels at the top setting: the costliest shadow
        // pass, a third of it the trees' leaf cards) is drawn every other frame and kept
        // for the next, with the light matrix it was drawn with; the close one - the bus
        // and everything within 30 m - every frame. Redrawn at once when the camera has
        // jumped, the sun has moved or the render origin has (its matrix is relative to it).
        let near_wanted = light_matrix(SHADOW_RANGE);
        let (near_m, near_age, near_origin, near_sun) = self.shadow_near_cache.get();
        let near_jumped =
            (near_m.project_point3(cam_rel) - near_wanted.project_point3(cam_rel)).length() > 0.03;
        let redraw_near = draw_shadows
            && (near_age >= 1
                || near_jumped
                || near_m == Mat4::IDENTITY
                || near_origin != render_origin
                || near_sun.dot(sun) < 0.99999
                || omsi_cfg::flags::SHADOW_NEAR_EVERY_FRAME.is_set());
        let light_view_proj = if let Some((_, _, near, _, _)) = shared_xr_shadows {
            near
        } else if !shadows {
            near_wanted
        } else if redraw_near {
            self.shadow_near_cache
                .set((near_wanted, 0, render_origin, sun));
            near_wanted
        } else {
            self.shadow_near_cache
                .set((near_m, near_age + 1, near_origin, near_sun));
            near_m
        };
        let light_view_proj_close = shared_xr_shadows
            .map(|(_, _, _, _, close)| close)
            .unwrap_or_else(|| light_matrix(SHADOW_RANGE_CLOSE));
        // The far cascade (700 m, metre-sized texels) is drawn every 4th frame, or at once
        // when the camera has left the middle of the one drawn, the sun has moved or the
        // render origin has jumped (its matrix is relative to that). Drawn every frame it
        // was 0.6 ms of GPU time for a picture that hardly changes; a car in it is a few
        // texels, and a tile streamed in waits three frames at most for its shadow.
        let far_wanted = light_matrix(SHADOW_RANGE_FAR);
        let (far_m, far_age, far_origin, far_sun) = self.shadow_far_cache.get();
        let far_moved =
            (far_m.project_point3(cam_rel) - far_wanted.project_point3(cam_rel)).length() > 0.12;
        let redraw_far = draw_shadows
            && (far_age >= 3
                || far_moved
                || far_m == Mat4::IDENTITY
                || far_origin != render_origin
                || far_sun.dot(sun) < 0.99999
                || omsi_cfg::flags::SHADOW_FAR_EVERY_FRAME.is_set());
        if redraw_far && omsi_cfg::flags::DEBUG_SHADOW_FAR.is_set() {
            log::info!(
                "far shadow redrawn: age {far_age} moved {far_moved} origin {} sun {:.6}",
                far_origin != render_origin,
                far_sun.dot(sun)
            );
        }
        // The desktop view and the first XR eye maintain this cache; mirrors and
        // the second XR eye leave it alone.
        let light_view_proj_far = if let Some((_, _, _, far, _)) = shared_xr_shadows {
            far
        } else if !shadows {
            far_wanted
        } else if redraw_far {
            self.shadow_far_cache
                .set((far_wanted, 0, render_origin, sun));
            far_wanted
        } else {
            self.shadow_far_cache
                .set((far_m, far_age + 1, far_origin, far_sun));
            far_m
        };
        if projection.is_some() && !second_eye && shadows {
            self.xr_shadow_cache.set(Some((
                render_origin,
                sun,
                light_view_proj,
                light_view_proj_far,
                light_view_proj_close,
            )));
        }
        ShadowSchedule {
            moon_shadows,
            shadows,
            draw_shadows,
            redraw_near,
            redraw_far,
            light_view_proj,
            light_view_proj_far,
            light_view_proj_close,
        }
    }
}
