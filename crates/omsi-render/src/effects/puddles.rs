//! Bounded screen-space puddles, with a small planar capture of the player's vehicle
//! for its complete geometry. Both use at most 518400 pixels.

use crate::assets::AssetStore;
use crate::assets::MaterialUniform;
use crate::gpu::{
    fullscreen_pipeline, pass_timer, sampler_entry, target_2d, DEPTH_FORMAT, HDR_FORMAT,
};
use crate::lighting::Lighting;
use crate::passes::{
    encode_batches, encode_batches_filtered, one_sided_primitive, Batch, Exposure, PostPass,
    PostPipelines, PIPE_BLEND, PIPE_BLEND_NO_WRITE, PIPE_KINDS,
};
use crate::scene::{Camera, CameraUniform, Scene, Vertex};
use crate::sky::{SkyResources, PROBE_MIPS};
use crate::targets::{PuddleTargets, TargetCache};
use glam::{DVec3, Mat4, Vec3};

pub const MAX_TRACE_PIXELS: f32 = 960.0 * 540.0;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct VehicleBox {
    a: [f32; 4],
    b: [f32; 4],
    c: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VehicleUniform {
    plane: [f32; 4],
    parts: [VehicleBox; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Uniform {
    view_proj: [[f32; 4]; 4],
    inv_view_proj: [[f32; 4]; 4],
    eye_time: [f32; 4],
    origin_rain: [f32; 4],
    sky: [f32; 4],
    projection_trace: [f32; 4],
    vehicle_plane: [f32; 4],
    vehicle_info: [f32; 4],
    vehicle_parts: [VehicleBox; 4],
}

pub fn shader_source() -> String {
    [
        include_str!("../shaders/common/fullscreen.wgsl"),
        include_str!("../shaders/common/colour.wgsl"),
        include_str!("../shaders/common/math.wgsl"),
        include_str!("../shaders/common/puddle_common.wgsl"),
        include_str!("../shaders/effects/puddle_reflection.wgsl"),
    ]
    .join("\n")
}

pub struct Pipelines {
    layout: wgpu::BindGroupLayout,
    params: wgpu::Buffer,
    trace: wgpu::RenderPipeline,
    blur_x: wgpu::RenderPipeline,
    blur_y: wgpu::RenderPipeline,
    resolve: wgpu::RenderPipeline,
    glass: [wgpu::RenderPipeline; 2],
    vehicle: Vec<wgpu::RenderPipeline>,
    chassis: wgpu::RenderPipeline,
    vehicle_params: wgpu::Buffer,
    vehicle_bg: wgpu::BindGroup,
}

/// How a mode lights what the puddles reflect: the light around the surface, its exposure,
/// and whether the picture is in the mode's own (high range) shading.
#[derive(Debug, Clone, Copy)]
pub struct PuddleLook {
    pub surround: Vec3,
    pub exposure: f32,
    pub own: bool,
}

impl PuddleLook {
    /// The plain shading's: exactly the sky sheen replaced in shade_vanilla, with no exposure.
    pub fn plain(lighting: &Lighting) -> Self {
        Self {
            surround: lighting.secondary * 0.5 + lighting.sun_color * lighting.sun_intensity * 0.35,
            exposure: 1.0,
            own: false,
        }
    }
}

/// What the puddles' reflections take from the rest of the renderer: the device, the post
/// passes' layout and sampler, the placeholder textures, the sky's probe and state, the
/// adapted exposure, and the presenter's bind group for a picture.
pub struct PuddleParts<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub post: &'a PostPipelines,
    pub assets: &'a AssetStore,
    pub sky: &'a SkyResources,
    pub adaptation: &'a Exposure,
    pub picture_group: &'a dyn Fn(&wgpu::TextureView) -> wgpu::BindGroup,
}

impl Pipelines {
    pub fn new(
        device: &wgpu::Device,
        scene_shader: &wgpu::ShaderModule,
        camera_layout: &wgpu::BindGroupLayout,
        material_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let texture = |binding, sample_type, view_dimension| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type,
                view_dimension,
                multisampled: false,
            },
            count: None,
        };
        let float = wgpu::TextureSampleType::Float { filterable: true };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("puddle reflections"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<Uniform>() as u64
                        ),
                    },
                    count: None,
                },
                texture(1, float, wgpu::TextureViewDimension::D2),
                texture(2, float, wgpu::TextureViewDimension::D2),
                texture(
                    3,
                    wgpu::TextureSampleType::Depth,
                    wgpu::TextureViewDimension::D2,
                ),
                texture(4, float, wgpu::TextureViewDimension::D2),
                sampler_entry(5),
                texture(6, float, wgpu::TextureViewDimension::Cube),
                texture(
                    7,
                    wgpu::TextureSampleType::Depth,
                    wgpu::TextureViewDimension::D2,
                ),
                texture(8, float, wgpu::TextureViewDimension::D2),
            ],
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("puddle reflection params"),
            size: std::mem::size_of::<Uniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("puddle reflections"),
            source: wgpu::ShaderSource::Wgsl(shader_source().into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("puddle reflections"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = |entry| {
            fullscreen_pipeline(
                device,
                entry,
                &pipeline_layout,
                &shader,
                "vs_main",
                entry,
                wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                },
            )
        };
        let glass_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("puddle glass depth"),
            bind_group_layouts: &[Some(camera_layout), Some(material_layout)],
            immediate_size: 0,
        });
        let glass = [false, true].map(|cull| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("puddle glass depth"),
                layout: Some(&glass_layout),
                vertex: wgpu::VertexState {
                    module: scene_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Vertex::LAYOUT],
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
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: scene_shader,
                    entry_point: Some("fs_puddle_glass_depth"),
                    targets: &[],
                    compilation_options: Default::default(),
                }),
                multiview_mask: None,
                cache: None,
            })
        });
        let vehicle_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("puddle vehicle plane"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(
                        std::mem::size_of::<VehicleUniform>() as u64
                    ),
                },
                count: None,
            }],
        });
        let vehicle_params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("puddle vehicle plane"),
            size: std::mem::size_of::<VehicleUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let vehicle_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("puddle vehicle plane"),
            layout: &vehicle_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: vehicle_params.as_entire_binding(),
            }],
        });
        let capture_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("puddle vehicle capture"),
            bind_group_layouts: &[
                Some(camera_layout),
                Some(material_layout),
                Some(&vehicle_layout),
            ],
            immediate_size: 0,
        });
        let mut vehicle = Vec::new();
        for kind in 0..PIPE_KINDS {
            for cull in [false, true] {
                for _surface in [false, true] {
                    let mut primitive = one_sided_primitive(cull);
                    // Mirroring reverses winding. Keep authored one-sided geometry.
                    primitive.front_face = wgpu::FrontFace::Ccw;
                    vehicle.push(
                        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                            label: Some("puddle vehicle capture"),
                            layout: Some(&capture_layout),
                            vertex: wgpu::VertexState {
                                module: scene_shader,
                                entry_point: Some("vs_puddle_vehicle"),
                                buffers: &[Vertex::LAYOUT],
                                compilation_options: Default::default(),
                            },
                            primitive,
                            depth_stencil: Some(wgpu::DepthStencilState {
                                format: DEPTH_FORMAT,
                                depth_write_enabled: Some(kind != PIPE_BLEND_NO_WRITE),
                                depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
                                stencil: Default::default(),
                                bias: Default::default(),
                            }),
                            multisample: Default::default(),
                            fragment: Some(wgpu::FragmentState {
                                module: scene_shader,
                                entry_point: Some("fs_puddle_vehicle"),
                                targets: &[Some(wgpu::ColorTargetState {
                                    format: HDR_FORMAT,
                                    blend: (kind == PIPE_BLEND || kind == PIPE_BLEND_NO_WRITE)
                                        .then_some(wgpu::BlendState::ALPHA_BLENDING),
                                    write_mask: wgpu::ColorWrites::ALL,
                                })],
                                compilation_options: Default::default(),
                            }),
                            multiview_mask: None,
                            cache: None,
                        }),
                    );
                }
            }
        }
        let chassis = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("puddle closed chassis"),
            layout: Some(&capture_layout),
            vertex: wgpu::VertexState {
                module: scene_shader,
                entry_point: Some("vs_puddle_chassis"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: scene_shader,
                entry_point: Some("fs_puddle_chassis"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            multiview_mask: None,
            cache: None,
        });
        Self {
            trace: pipeline("fs_trace"),
            blur_x: pipeline("fs_blur_x"),
            blur_y: pipeline("fs_blur_y"),
            resolve: pipeline("fs_resolve"),
            glass,
            vehicle,
            chassis,
            vehicle_params,
            vehicle_bg,
            layout,
            params,
        }
    }
}

pub fn trace_size(w: u32, h: u32) -> (u32, u32) {
    let scale = 0.5_f32.min((MAX_TRACE_PIXELS / (w.max(1) as f32 * h.max(1) as f32)).sqrt());
    (
        ((w as f32 * scale).floor() as u32).max(1),
        ((h as f32 * scale).floor() as u32).max(1),
    )
}

impl PuddleTargets {
    /// `picture`, `mask` and `glow`: the size's high-range picture, its screen mask and its
    /// first glow level.
    #[allow(clippy::too_many_arguments)]
    fn new(
        ctx: &PuddleParts,
        pipelines: &Pipelines,
        ao_depth: &wgpu::TextureView,
        picture: &wgpu::TextureView,
        mask: &wgpu::TextureView,
        glow: &wgpu::TextureView,
        w: u32,
        h: u32,
    ) -> Self {
        let size = trace_size(w, h);
        let source_depth = ao_depth.texture().clone();
        let hit_depth = target_2d(
            &ctx.device,
            "puddle hit depth including glass",
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            1,
            DEPTH_FORMAT,
            wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let hit_depth_view = hit_depth.create_view(&Default::default());
        let target = |label, (width, height)| {
            target_2d(
                &ctx.device,
                label,
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                1,
                HDR_FORMAT,
                wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
            )
            .create_view(&Default::default())
        };
        let trace = target("puddle reflection rays", size);
        let vehicle_view = target("puddle player vehicle", size);
        let vehicle_depth = target_2d(
            &ctx.device,
            "puddle player vehicle depth",
            wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            1,
            DEPTH_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        )
        .create_view(&Default::default());
        let blur = target("puddle reflection horizontal filter", size);
        let filtered = target("puddle reflection filtered", size);
        let view = target("scene with puddle reflections", (w, h));
        let binding = |binding, view| wgpu::BindGroupEntry {
            binding,
            resource: wgpu::BindingResource::TextureView(view),
        };
        let group = |ray_view| {
            ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("puddle reflections"),
                layout: &pipelines.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: pipelines.params.as_entire_binding(),
                    },
                    binding(1, picture),
                    binding(2, mask),
                    binding(3, ao_depth),
                    binding(4, ray_view),
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::Sampler(&ctx.post.post_sampler),
                    },
                    binding(6, &ctx.sky.probe.as_ref().unwrap().view),
                    binding(7, &hit_depth_view),
                    binding(8, &vehicle_view),
                ],
            })
        };
        // The trace target must never appear in its own bind group, including unused slots.
        let trace_bg = group(&ctx.assets.black_texture.view);
        let blur_x_bg = group(&trace);
        let blur_y_bg = group(&blur);
        let resolve_bg = group(&filtered);
        let post_group = |base, adapt, screen_mask| {
            ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("post with puddle reflections"),
                layout: &ctx.post.post_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: ctx.post.post_buf.as_entire_binding(),
                    },
                    binding(1, &view),
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&ctx.post.post_sampler),
                    },
                    binding(3, base),
                    binding(4, adapt),
                    binding(5, screen_mask),
                ],
            })
        };
        let down_bg = post_group(
            mask,
            &ctx.assets.white_texture.view,
            &ctx.assets.black_texture.view,
        );
        let tonemap_bg = [
            post_group(glow, &ctx.adaptation.adapt_views[0], mask),
            post_group(glow, &ctx.adaptation.adapt_views[1], mask),
        ];
        let classic_bg = (ctx.picture_group)(&view);
        Self {
            size,
            source_depth,
            hit_depth,
            hit_depth_view,
            vehicle_view,
            vehicle_depth,
            trace,
            blur,
            filtered,
            view,
            trace_bg,
            blur_x_bg,
            blur_y_bg,
            resolve_bg,
            down_bg,
            tonemap_bg,
            classic_bg,
        }
    }
}

impl Pipelines {
    /// This frame's puddle uniforms, and the size's reflection targets when they are new or
    /// stale. False when the frame cannot have puddle reflections.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        &self,
        ctx: &PuddleParts,
        targets: &mut TargetCache,
        w: u32,
        h: u32,
        camera: &Camera,
        aspect: f32,
        projection: Option<Mat4>,
        cu: &CameraUniform,
        lighting: &Lighting,
        look: Option<PuddleLook>,
        ro: DVec3,
    ) -> bool {
        // (no look: the mode cannot light the reflections yet, its sky is not there)
        let Some(look) = look else {
            return false;
        };
        if targets.ao.is_none() || ctx.sky.probe.is_none() {
            return false;
        }
        let Some(hdr) = targets.hdr_targets.get(&(w, h)) else {
            return false;
        };
        // A previously used size can outlive the AO depth after a resize/another view.
        // Rebind the current receiver texture instead of tracing against an old frame.
        if hdr
            .puddles
            .as_ref()
            .is_none_or(|t| t.source_depth != *targets.ao.as_ref().unwrap().depth_view.texture())
        {
            let made = PuddleTargets::new(
                ctx,
                self,
                &targets.ao.as_ref().unwrap().depth_view,
                &hdr.view,
                &hdr.mask,
                &hdr.up[0],
                w,
                h,
            );
            log::info!(
                "puddle reflections: {}x{} rays, at most 48 steps; resolve {w}x{h}",
                made.size.0,
                made.size.1
            );
            targets.hdr_targets.get_mut(&(w, h)).unwrap().puddles = Some(made);
        }
        let vp = Mat4::from_cols_array_2d(&cu.view_proj);
        let proj = projection.unwrap_or_else(|| {
            Mat4::perspective_rh(camera.fov_deg.to_radians(), aspect, camera.far, camera.near)
        });
        let PuddleLook {
            surround,
            exposure: pre,
            own: own_shading,
        } = look;
        let origins = vehicle_origins(lighting, camera);
        let mut parts = [<VehicleBox as bytemuck::Zeroable>::zeroed(); 4];
        for (part, (o, h, bb)) in parts
            .iter_mut()
            .zip(lighting.inside.iter().chain(lighting.puddle_parts.iter()))
        {
            let origin = (*o - ro).as_vec3();
            let heading = (*h as f32).to_radians();
            *part = VehicleBox {
                a: [origin.x, origin.y, origin.z, heading.sin()],
                b: [heading.cos(), bb[0] * 0.5, bb[1] * 0.5, bb[2] * 0.5],
                c: [bb[3], bb[4], bb[5], 1.0],
            };
        }
        let normal = lighting.puddle_normal.normalize_or_zero();
        let plane_normal = if normal.z > 0.3 { normal } else { Vec3::Z };
        let plane_point = lighting.inside.map_or(DVec3::ZERO, |(o, _, _)| {
            DVec3::new(o.x, o.y, lighting.puddle_ground.unwrap_or(o.z))
        });
        let plane = reflection_plane(plane_point, plane_normal, ro).to_array();
        let u = Uniform {
            view_proj: cu.view_proj,
            inv_view_proj: vp.inverse().to_cols_array_2d(),
            eye_time: [cu.cam_pos[0], cu.cam_pos[1], cu.cam_pos[2], cu.post[1]],
            origin_rain: [
                cu.world_origin[0],
                cu.world_origin[1],
                lighting.rain.clamp(0.0, 1.0) * (1.0 - lighting.snow.clamp(0.0, 1.0)),
                PROBE_MIPS as f32,
            ],
            sky: (surround * pre)
                .extend(ctx.sky.probe.as_ref().unwrap().scale * pre)
                .to_array(),
            projection_trace: [
                proj.z_axis.z,
                proj.w_axis.z,
                crate::gpu::debug().debug_puddles.unwrap_or(0.0),
                crate::gpu::debug().puddle_thickness.unwrap_or(0.12),
            ],
            vehicle_plane: plane,
            vehicle_info: [
                origins.len() as f32,
                w as f32,
                h as f32,
                if own_shading { 1.0 } else { -1.0 },
            ],
            vehicle_parts: parts,
        };
        ctx.queue.write_buffer(
            &self.vehicle_params,
            0,
            bytemuck::bytes_of(&VehicleUniform { plane, parts }),
        );
        ctx.queue
            .write_buffer(&self.params, 0, bytemuck::bytes_of(&u));
        true
    }

    #[allow(clippy::too_many_arguments)]
    pub fn encode(
        &self,
        targets: &TargetCache,
        encoder: &mut wgpu::CommandEncoder,
        w: u32,
        h: u32,
        scene: &Scene,
        batches: &[Batch],
        draw_list: &[u32],
        lighting: &Lighting,
        camera: &Camera,
        queries: Option<&wgpu::QuerySet>,
        timed: &mut Vec<&'static str>,
    ) {
        let pipelines = self;
        let targets = targets.hdr_targets[&(w, h)].puddles.as_ref().unwrap();
        // Select only this vehicle's existing draw entries. AI instances can share meshes
        // and batches with it; selecting a mesh alone would reflect all of them at one height.
        let origins = vehicle_origins(lighting, camera);
        let vehicle_batches = player_vehicle_batches(scene, batches, draw_list, &origins);
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("puddle player vehicle"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &targets.vehicle_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &targets.vehicle_depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: pass_timer(queries, timed, "puddle vehicle"),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, scene.camera_bind_group.as_ref().unwrap(), &[]);
            pass.set_bind_group(2, &pipelines.vehicle_bg, &[]);
            if !crate::gpu::debug().no_puddle_vehicle {
                if let Some(first) = vehicle_batches.first() {
                    pass.set_bind_group(
                        1,
                        &scene.materials[first.material as usize].bind_group,
                        &[],
                    );
                    pass.set_pipeline(&pipelines.chassis);
                    pass.draw(0..6, 0..origins.len() as u32);
                }
                encode_batches(&mut pass, scene, &vehicle_batches, |pipe| {
                    &pipelines.vehicle[pipe as usize]
                });
            }
        }
        let copy = |texture| wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::DepthOnly,
        };
        encoder.copy_texture_to_texture(
            copy(&targets.source_depth),
            copy(&targets.hit_depth),
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        if !crate::gpu::debug().no_puddle_glass_depth
            && batches
                .iter()
                .any(|b| reflection_glass(&scene.materials[b.material as usize].uniform))
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("puddle glass depth"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &targets.hit_depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: pass_timer(queries, timed, "puddle glass"),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, scene.camera_bind_group.as_ref().unwrap(), &[]);
            encode_batches_filtered(
                &mut pass,
                scene,
                batches,
                |b| reflection_glass(&scene.materials[b.material as usize].uniform),
                |pipe| &pipelines.glass[(pipe % 4 / 2) as usize],
            );
        }
        PostPass::new("post", &targets.trace)
            .timer(pass_timer(queries, timed, "puddle rays"))
            .draw(encoder, &pipelines.trace, &[&targets.trace_bg]);
        let filter_timer = pass_timer(queries, timed, "puddle filter");
        PostPass::new("post", &targets.blur)
            .timer(
                filter_timer
                    .as_ref()
                    .map(|t| wgpu::RenderPassTimestampWrites {
                        query_set: t.query_set,
                        beginning_of_pass_write_index: t.beginning_of_pass_write_index,
                        end_of_pass_write_index: None,
                    }),
            )
            .draw(encoder, &pipelines.blur_x, &[&targets.blur_x_bg]);
        PostPass::new("post", &targets.filtered)
            .timer(filter_timer.map(|t| wgpu::RenderPassTimestampWrites {
                query_set: t.query_set,
                beginning_of_pass_write_index: None,
                end_of_pass_write_index: t.end_of_pass_write_index,
            }))
            .draw(encoder, &pipelines.blur_y, &[&targets.blur_y_bg]);
        PostPass::new("post", &targets.view)
            .timer(pass_timer(queries, timed, "puddle resolve"))
            .draw(encoder, &pipelines.resolve, &[&targets.resolve_bg]);
    }
}

pub fn reflection_plane(point: DVec3, normal: Vec3, origin: DVec3) -> glam::Vec4 {
    normal.extend(normal.dot((point - origin).as_vec3()))
}

fn vehicle_origins(lighting: &Lighting, camera: &Camera) -> Vec<DVec3> {
    if lighting.puddle_ground.is_none()
        || crate::gpu::debug().no_puddle_vehicle
        || !lighting
            .inside
            .is_some_and(|(o, _, _)| o.distance(camera.position) < 60.0)
    {
        return Vec::new();
    }
    lighting
        .inside
        .iter()
        .chain(lighting.puddle_parts.iter())
        .take(4)
        .map(|(o, _, _)| *o)
        .collect()
}

fn player_vehicle_batches(
    scene: &Scene,
    batches: &[Batch],
    list: &[u32],
    origins: &[DVec3],
) -> Vec<Batch> {
    let entries: std::collections::HashSet<u32> = scene
        .instances
        .iter()
        .filter(|i| {
            i.visible
                && i.roof.is_some()
                && !i.blob
                && !i.surface
                && origins.iter().any(|o| i.origin.distance_squared(*o) < 0.01)
        })
        .flat_map(|i| i.base..i.base + i.materials.len() as u32)
        .collect();
    selected_entry_batches(batches, list, &entries)
}

fn selected_entry_batches(
    batches: &[Batch],
    list: &[u32],
    entries: &std::collections::HashSet<u32>,
) -> Vec<Batch> {
    let mut out = Vec::new();
    for b in batches {
        let mut start = None;
        for index in b.instances.clone() {
            if entries.contains(&list[index as usize]) {
                start.get_or_insert(index);
            } else if let Some(first) = start.take() {
                out.push(Batch {
                    pipe: b.pipe,
                    mesh: b.mesh,
                    first: b.first,
                    count: b.count,
                    material: b.material,
                    instances: first..index,
                });
            }
        }
        if let Some(first) = start {
            out.push(Batch {
                pipe: b.pipe,
                mesh: b.mesh,
                first: b.first,
                count: b.count,
                material: b.material,
                instances: first..b.instances.end,
            });
        }
    }
    out
}

// Match the enhanced shader's glass classification, excluding rain films, decals and
// unlit screens. Their colour is already composited over the actual window surface.
pub fn reflection_glass(u: &MaterialUniform) -> bool {
    u.extra[0] < 0.5
        && u.params[0] > 1.5
        && u.params[1] < 0.5
        && (u.bump[2] > 0.5 || u.emissive[3] > 0.5)
        && u.bump[3] < 0.5
        && u.emissive[3] < 1.5
        && (u.params2[1] > 0.0 || u.params[2] > 0.5 || u.emissive[3] > 0.5)
}

#[test]
fn vehicle_capture_splits_shared_batches_without_drawing_ai_entries() {
    let batch = Batch {
        pipe: 7,
        mesh: 3,
        first: 12,
        count: 24,
        material: 4,
        instances: 0..6,
    };
    let own = [11, 12, 13, 14].into_iter().collect();
    let selected = selected_entry_batches(&[batch], &[11, 12, 32, 13, 45, 14], &own);
    assert_eq!(
        selected
            .iter()
            .map(|b| b.instances.clone())
            .collect::<Vec<_>>(),
        vec![0..2, 3..4, 5..6]
    );
    assert!(selected
        .iter()
        .all(|b| (b.pipe, b.mesh, b.first, b.count, b.material) == (7, 3, 12, 24, 4)));
}
