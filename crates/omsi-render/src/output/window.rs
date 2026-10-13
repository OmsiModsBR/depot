//! The window: its surface, and presenting a frame to it with upscale, FXAA, the overlays
//! and the HUD.

use std::collections::HashMap;
use std::sync::Arc;

use crate::gpu::{buffer_init, pass_timer};
use crate::renderer::{Renderer, DEPTH_FORMAT};
use crate::scene::{Scene, TextureId};
use anyhow::{Context, Result};

/// Helper for windowed rendering.
pub struct SurfaceState<'w> {
    /// (let go of in `drop`, unless the device was lost - see there)
    pub surface: std::mem::ManuallyDrop<wgpu::Surface<'w>>,
    pub config: wgpu::SurfaceConfiguration,
    /// The renderer's `device_lost`.
    pub(crate) lost: Arc<std::sync::Mutex<Option<String>>>,
}

impl Drop for SurfaceState<'_> {
    fn drop(&mut self) {
        // After a lost device the frame it was drawing never finishes, and its swapchain
        // image with it: letting the surface go (or configuring it again) then tears the
        // swapchain down under that image - "Trying to destroy a SwapchainAcquireSemaphore
        // that is still in use by a SurfaceTexture" (Vulkan) ended the game instead of the
        // session ending in order. The window goes with the process anyway. So on a panic:
        // the frame being drawn is let go of while unwinding, and the same message then
        // took the place of the panic that ended the game in its report (#112: a sort).
        if !std::thread::panicking()
            && self
                .lost
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .is_none()
        {
            // SAFETY: dropped only here, once
            unsafe { std::mem::ManuallyDrop::drop(&mut self.surface) };
        }
    }
}

impl<'w> SurfaceState<'w> {
    pub fn new(
        instance: &wgpu::Instance,
        window: Arc<winit_window::Window>,
        renderer: &Renderer,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        Self::new_with(instance, window, renderer, width, height, true)
    }

    /// `vsync` false lets the frames go out as fast as they are drawn.
    pub fn new_with(
        instance: &wgpu::Instance,
        window: Arc<winit_window::Window>,
        renderer: &Renderer,
        width: u32,
        height: u32,
        vsync: bool,
    ) -> Result<Self> {
        let surface = instance.create_surface(window).context("create_surface")?;
        let mut config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: renderer.format(),
            width: width.max(1),
            height: height.max(1),
            present_mode: if vsync {
                wgpu::PresentMode::AutoVsync
            } else {
                wgpu::PresentMode::AutoNoVsync
            },
            // (two frames in flight keep the graphics chip busy while the next frame is
            // recorded: with one, the whole loop serialized behind the vsync'd drawable -
            // on Apple silicon a frame cost GPU + CPU instead of the larger of the two,
            // 50 ms where 24 of them were GPU. The price is one more frame of input
            // delay, 33 ms at 60 Hz.)
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
        };
        config.width = width.max(1);
        config.height = height.max(1);
        surface.configure(&renderer.device, &config);
        Ok(SurfaceState {
            surface: std::mem::ManuallyDrop::new(surface),
            config,
            lost: renderer.gpu.device_lost.clone(),
        })
    }

    pub fn resize(&mut self, renderer: &Renderer, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        // (not after a lost device: see `drop`)
        if renderer.device_lost().is_some() {
            return;
        }
        self.surface.configure(&renderer.device, &self.config);
    }

    pub fn set_vsync(&mut self, renderer: &Renderer, enabled: bool) {
        let mode = if enabled {
            wgpu::PresentMode::AutoVsync
        } else {
            wgpu::PresentMode::AutoNoVsync
        };
        if self.config.present_mode == mode || renderer.device_lost().is_some() {
            return;
        }
        self.config.present_mode = mode;
        // (two frames in flight: see `new_with`)
        self.config.desired_maximum_frame_latency = 2;
        self.surface.configure(&renderer.device, &self.config);
    }
}

/// Re-export so the app does not need to depend on winit's window type path.
pub mod winit_window {
    pub use winit::window::Window;
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
        source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/output/overlay.wgsl").into()),
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
        source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/output/xr_ui.wgsl").into()),
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
        source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/output/upscale.wgsl").into()),
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

/// An overlay's rectangle (physical pixels) moved onto whole pixels, its size kept. The
/// overlays are pictures drawn texel for pixel - a text, a plate - and the linear filter
/// blended every pixel of one placed between pixels with its neighbour: the interface's texts
/// were soft at every size whose layout fell between them (most but 100 %, and the timetable's
/// rows at that too). A line thinner than a pixel stays one pixel wide or high.
pub(crate) fn snap_rect(r: [f32; 4]) -> [f32; 4] {
    // (half up the same way left of the window as right of it: `round` goes away from zero,
    // and a rectangle across the left edge came out a pixel wider)
    let snap = |v: f32| (v + 0.5).floor();
    let (x0, y0) = (snap(r[0]), snap(r[1]));
    let x1 = if r[2] > r[0] {
        snap(r[2]).max(x0 + 1.0)
    } else {
        snap(r[2])
    };
    let y1 = if r[3] > r[1] {
        snap(r[3]).max(y0 + 1.0)
    } else {
        snap(r[3])
    };
    [x0, y0, x1, y1]
}

impl Renderer {
    pub(crate) fn prepare_overlays(&self, scene: &mut Scene, full_w: u32, full_h: u32) {
        let overlays = &scene.overlays;
        // the HUD's rect buffers and bind groups live on between frames: making them
        // anew for every overlay of every frame was a steady stream of GPU allocations
        scene.overlay_res.truncate(overlays.len());
        for (k, (tex, r)) in overlays.iter().copied().enumerate() {
            let r = snap_rect(r);
            let ndc = [
                r[0] / full_w as f32 * 2.0 - 1.0,
                1.0 - r[1] / full_h as f32 * 2.0,
                r[2] / full_w as f32 * 2.0 - 1.0,
                1.0 - r[3] / full_h as f32 * 2.0,
                scene.premultiplied.contains(&tex) as u8 as f32,
                scene.transposed.contains(&tex) as u8 as f32,
                0.0,
                0.0,
            ];
            if let Some((_, buf, _, last)) = scene.overlay_res.get_mut(k).filter(|o| o.0 == tex) {
                if *last != ndc {
                    self.queue.write_buffer(buf, 0, bytemuck::cast_slice(&ndc));
                    *last = ndc;
                }
                continue;
            }
            let buf = buffer_init(
                &self.device,
                &self.queue,
                Some("overlay rect"),
                bytemuck::cast_slice(&ndc),
                wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            );
            let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("overlay"),
                layout: &self.presenter.overlay_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&scene.textures[tex].view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.sky.sky_sampler),
                    },
                ],
            });
            if k < scene.overlay_res.len() {
                scene.overlay_res[k] = (tex, buf, bg, ndc);
            } else {
                scene.overlay_res.push((tex, buf, bg, ndc));
            }
        }
    }

    /// With a render scale: the smaller picture scaled up to the window, the overlays on top
    /// at full size.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn encode_present(
        &self,
        scene: &Scene,
        encoder: &mut wgpu::CommandEncoder,
        scene_target: &Option<(wgpu::TextureView, wgpu::BindGroup)>,
        target: &wgpu::TextureView,
        overlays: &[(TextureId, [f32; 4])],
        width: u32,
        height: u32,
        full_w: u32,
        vanilla_fxaa: bool,
        tset: &Option<wgpu::QuerySet>,
        timed: &mut Vec<&'static str>,
    ) {
        if let Some((_, bg)) = &scene_target {
            // --- the smaller picture scaled up to the window, the HUD on top at full size;
            // the smaller the picture, the more it is sharpened
            let sharpen = (1.0 - width as f32 / full_w as f32) * 2.0;
            self.queue.write_buffer(
                &self.presenter.upscale_buf,
                0,
                bytemuck::cast_slice(&[
                    width as f32,
                    height as f32,
                    sharpen.clamp(0.0, 0.8),
                    if vanilla_fxaa { 1.0 } else { 0.0 },
                ]),
            );
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("upscale"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: pass_timer(tset.as_ref(), timed, "upscale"),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.presenter.upscale_pipeline);
            pass.set_bind_group(0, bg, &[]);
            pass.draw(0..3, 0..1);
            if !overlays.is_empty() {
                pass.set_pipeline(&self.presenter.overlay_pipeline_1x);
                for (k, _) in overlays.iter().enumerate() {
                    if let Some((_, _, bg, _)) = scene.overlay_res.get(k) {
                        pass.set_bind_group(0, bg, &[]);
                        pass.draw(0..6, 0..1);
                    }
                }
            }
        }
    }

    pub(crate) fn picture_group(&self, view: &wgpu::TextureView) -> wgpu::BindGroup {
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene picture"),
            layout: &self.presenter.upscale_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.presenter.upscale_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.post.post_sampler),
                },
            ],
        })
    }
}

/// Delivering the picture: scaling it to the window, the triple screen's panels, the HUD and the
/// headset's UI.
pub(crate) struct Presenter {
    /// The HUD and overlays drawn into the multisampled picture, and their layout.
    pub(crate) overlay_pipeline: wgpu::RenderPipeline,
    pub(crate) overlay_layout: wgpu::BindGroupLayout,
    /// Overlay pipeline without multisampling, for drawing the HUD after the post pass.
    pub(crate) overlay_pipeline_1x: wgpu::RenderPipeline,
    pub(crate) xr_ui_pipeline: wgpu::RenderPipeline,
    /// Render scale: the pipeline that scales the 3D picture up to the window, its
    /// parameters, and the smaller targets per size (with their bind groups).
    pub(crate) upscale_pipeline: wgpu::RenderPipeline,
    pub(crate) copy_pipeline: wgpu::RenderPipeline,
    /// Triple screen: a panel's picture copied 1:1 to its place in the window.
    pub(crate) panel_pipeline: wgpu::RenderPipeline,
    pub(crate) upscale_layout: wgpu::BindGroupLayout,
    pub(crate) upscale_buf: wgpu::Buffer,
    pub(crate) scale_targets: HashMap<(u32, u32), (wgpu::TextureView, wgpu::BindGroup)>,
    pub(crate) triple_targets: Option<((u32, u32), Vec<(wgpu::TextureView, wgpu::BindGroup)>)>,
}

impl Presenter {
    /// The overlay, scaling and panel pipelines for the window's `format`.
    pub(crate) fn new(device: &wgpu::Device, format: wgpu::TextureFormat, msaa: u32) -> Self {
        let (overlay_layout, overlay_shader, overlay_pl, premul, overlay_pipeline) =
            overlay_pipeline(device, format, msaa);
        let (overlay_pipeline_1x, xr_ui_pipeline) =
            overlay_1x_pipelines(device, format, &overlay_pl, &overlay_shader, premul);
        let (upscale_layout, upscale_pipeline, copy_pipeline, panel_pipeline, upscale_buf) =
            upscale_pipelines(device, format);
        Self {
            overlay_pipeline,
            overlay_layout,
            upscale_pipeline,
            copy_pipeline,
            panel_pipeline,
            upscale_layout,
            upscale_buf,
            scale_targets: HashMap::new(),
            triple_targets: None,
            overlay_pipeline_1x,
            xr_ui_pipeline,
        }
    }
}
