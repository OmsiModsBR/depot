//! The enhanced post passes: glow, exposure metering and tone mapping.

use super::draw::post_pass;
use crate::assets::GpuTexture;
use crate::gpu::{
    float_tex, fullscreen_pipeline, pass_timer, sampler_entry, target_2d, uniform_entry,
};
use crate::lighting::{meter_tuning, tone_contrast, ExposureLog, Lighting};
use crate::scene::{Scene, TextureId};
use crate::sky::{atmosphere, SkyResources};
use crate::targets::TargetCache;

/// The enhanced path's post passes (post.wgsl `PostParams`).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct PostUniform {
    pub(crate) a: [f32; 4],
    pub(crate) b: [f32; 4],
    pub(crate) c: [f32; 4],
    pub(crate) d: [f32; 4],
}

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
    /// Their bind group layout, uniform buffer and sampler.
    pub(crate) post_layout: wgpu::BindGroupLayout,
    pub(crate) post_buf: wgpu::Buffer,
    pub(crate) post_sampler: wgpu::Sampler,
}

/// The enhanced path's post passes (post.wgsl): their bind group layout, uniform buffer,
/// sampler and pipelines.
pub(crate) fn post_pipelines(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    hdr_format: wgpu::TextureFormat,
) -> PostPipelines {
    // --- enhanced graphics: the post passes (glow, metering, adaptation, tone curve, FXAA)
    log::info!("renderer: compiling the post passes shaders");
    let post_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("post"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/post/post.wgsl").into()),
    });
    let post_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("post"),
        entries: &[
            uniform_entry(0, wgpu::ShaderStages::FRAGMENT),
            float_tex(1),
            sampler_entry(2),
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
        fullscreen_pipeline(
            device,
            entry,
            &post_pl,
            &post_shader,
            "vs_main",
            entry,
            wgpu::ColorTargetState {
                format: target,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            },
        )
    };
    PostPipelines {
        down_first: post_pipeline("fs_down_first", hdr_format),
        down: post_pipeline("fs_down", hdr_format),
        up: post_pipeline("fs_up", hdr_format),
        meter: post_pipeline("fs_meter", hdr_format),
        adapt: post_pipeline("fs_adapt", hdr_format),
        tonemap: post_pipeline("fs_tonemap", format),
        tonemap_encoded: post_pipeline("fs_tonemap_encoded", wgpu::TextureFormat::Rgba8Unorm),
        fxaa: post_pipeline("fs_fxaa", format),
        post_layout,
        post_buf,
        post_sampler,
    }
}

/// What the enhanced path's post passes work with: their pipelines, the targets, the
/// eye's adaptation and the sky it follows.
pub(crate) struct PostParts<'a> {
    pub(crate) queue: &'a wgpu::Queue,
    pub(crate) post: &'a PostPipelines,
    pub(crate) targets: &'a TargetCache,
    pub(crate) adaptation: &'a mut Exposure,
    pub(crate) sky: &'a SkyResources,
    /// The options' multisampling and FXAA.
    pub(crate) msaa: u32,
    pub(crate) fxaa: bool,
    pub(crate) instant_exposure: bool,
    /// The exposure log, when this picture is sampled into it (the window's, the device
    /// not lost).
    pub(crate) exposure_log: Option<&'a mut ExposureLog>,
    /// The presenter's pipeline for the HUD overlays at full size.
    pub(crate) overlay_pipeline_1x: &'a wgpu::RenderPipeline,
}

/// The enhanced path's post passes: glow, metering, adaptation, tone curve and FXAA.
#[allow(clippy::too_many_arguments)]
pub(crate) fn encode_enhanced_post(
    ctx: PostParts,
    scene: &Scene,
    encoder: &mut wgpu::CommandEncoder,
    lighting: &Lighting,
    scene_view: &wgpu::TextureView,
    overlays: &[(TextureId, [f32; 4])],
    width: u32,
    height: u32,
    dt: f32,
    enhanced: bool,
    lead_view: bool,
    puddles_on: bool,
    rt_frame: bool,
    scaled: bool,
    with_overlays: bool,
    tset: &Option<wgpu::QuerySet>,
    timed: &mut Vec<&'static str>,
) {
    if enhanced {
        // --- the post passes: glow, metering and adaptation, tone curve, FXAA
        let secs = |tau: f32| {
            if dt > 0.0 {
                1.0 - (-dt / tau).exp()
            } else {
                1.0
            }
        };
        // (the brightening is for what the light model cannot know by day - a dark cab,
        // an underpass; by night the picture is dark because the night is, and the eye's
        // adaptation to it is the light model's already: lifted by the meter on top, a
        // lamp-lit street came out most of a stop brighter than any eye sees it)
        let mut m = meter_tuning();
        let after_sunset = ctx.sky.sky_state.as_ref().map_or(0.0, |st| {
            ((0.052 - st.input.sun_dir.normalize_or_zero().z) / 0.157).clamp(0.0, 1.0)
        });
        m[3] *= 1.0
            - atmosphere::smoothstep(
                3.0,
                7.0,
                ctx.adaptation.exposure.unwrap_or(0.0) / std::f32::consts::LN_2,
            );
        let pu = PostUniform {
            // the metering may take a little off a bright picture and add a little to a
            // dark one: a night stays a night, snow stays white
            a: [
                // (the share of the light beyond the screen's white the eye scatters
                // further than a third of a degree, CIE 146 - see post.wgsl `fs_up`)
                0.2,
                m[2],
                m[3],
                if ctx.instant_exposure || dt <= 0.0 {
                    1.0
                } else {
                    0.0
                },
            ],
            // darker: the eye takes a few seconds; brighter: under one
            b: [
                secs(2.5),
                secs(0.6),
                m[1],
                m[4] + lighting.night_brightness * after_sunset,
            ],
            // (w: an LED panel's dots count for this much in the glow's source. The mix
            // the glow lands with is a few per cent - a lamp a hundred times brighter
            // than white spreads, a white wall does not - so the dots are multiplied up
            // there instead of being drawn burning: their halo shows, they don't bleach.
            // `Led glow`, 0 = not at all.)
            c: [
                m[0],
                m[5],
                ctx.adaptation.exposure.map(f32::exp).unwrap_or(1.0),
                lighting.led_glow * 10.0,
            ],
            // Enhanced+: its grade, the vignette and the sharpening; both: the tone
            // curve's contrast (post.wgsl `natural_tone`)
            d: {
                let contrast = tone_contrast(ctx.adaptation.exposure.unwrap_or(0.0));
                if rt_frame && !omsi_cfg::flags::NO_RT_GRADE.is_set() {
                    [1.0, 0.08, 0.0, contrast]
                } else {
                    [0.0, 0.0, 0.0, contrast]
                }
            },
        };
        ctx.queue
            .write_buffer(&ctx.post.post_buf, 0, bytemuck::bytes_of(&pu));
        // (a mirror's small picture goes without FXAA)
        let fxaa = with_overlays && ctx.fxaa && ctx.msaa <= 1 && !omsi_cfg::flags::NO_FXAA.is_set();
        if let Some(h) = ctx.targets.hdr_targets.get(&(width, height)) {
            let puddles = h.puddles.as_ref().filter(|_| puddles_on);
            let levels = h.down.len();
            for i in 0..levels {
                post_pass(
                    encoder,
                    &h.down[i],
                    None,
                    if i == 0 {
                        &ctx.post.down_first
                    } else {
                        &ctx.post.down
                    },
                    if i == 0 {
                        puddles.map(|p| &p.down_bg).unwrap_or(&h.down_bg[i])
                    } else {
                        &h.down_bg[i]
                    },
                );
            }
            // the exposure: meter the smallest level, move the adapted value towards it
            // (the window's picture only: a mirror is graded with the window's exposure,
            // as the eye that looks into it is adapted to the street)
            if lead_view {
                post_pass(
                    encoder,
                    &ctx.adaptation.meter_view,
                    None,
                    &ctx.post.meter,
                    &h.meter_bg,
                );
                let front = ctx.adaptation.adapt_front;
                post_pass(
                    encoder,
                    &ctx.adaptation.adapt_views[1 - front],
                    None,
                    &ctx.post.adapt,
                    &ctx.adaptation.adapt_bg[front],
                );
                ctx.adaptation.adapt_front = 1 - front;
            }
            if let Some(log) = ctx.exposure_log {
                let pre = ctx.adaptation.exposure.unwrap_or(0.0) / std::f32::consts::LN_2;
                log.sample(
                    encoder,
                    &ctx.adaptation.adapt_views[ctx.adaptation.adapt_front],
                    pre,
                    m,
                );
            }
            // (timed on its last pass: the glow chain with the metering, see GpuTimers)
            for i in (0..levels).rev() {
                let timer = if i == 0 {
                    pass_timer(tset.as_ref(), timed, "glow+meter")
                } else {
                    None
                };
                post_pass(encoder, &h.up[i], timer, &ctx.post.up, &h.up_bg[i]);
            }
            let final_view = if fxaa { &h.ldr } else { scene_view };
            let tonemap_bg = &puddles.map(|p| &p.tonemap_bg).unwrap_or(&h.tonemap_bg)
                [ctx.adaptation.adapt_front];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tone map"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: final_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: pass_timer(tset.as_ref(), timed, "tone map"),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(if fxaa {
                &ctx.post.tonemap_encoded
            } else {
                &ctx.post.tonemap
            });
            pass.set_bind_group(0, tonemap_bg, &[]);
            pass.draw(0..3, 0..1);
            if fxaa {
                drop(pass);
                pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("fxaa"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: scene_view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: pass_timer(tset.as_ref(), timed, "fxaa"),
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(&ctx.post.fxaa);
                pass.set_bind_group(0, &h.fxaa_bg, &[]);
                pass.draw(0..3, 0..1);
            }
            if !overlays.is_empty() && !scaled {
                pass.set_pipeline(ctx.overlay_pipeline_1x);
                for (k, _) in overlays.iter().enumerate() {
                    if let Some((_, _, bg, _)) = scene.overlay_res.get(k) {
                        pass.set_bind_group(0, bg, &[]);
                        pass.draw(0..6, 0..1);
                    }
                }
            }
        }
    }
}

/// The eye's adaptation: the metered luminance, the adapted value and the exposure as it follows
/// the sky.
pub(crate) struct Exposure {
    /// The metered mean log luminance (1x1), and the adapted value in two textures drawn
    /// into by turns (`adapt_front` is the current one), with their bind groups.
    pub(crate) meter_view: wgpu::TextureView,
    pub(crate) adapt_views: [wgpu::TextureView; 2],
    pub(crate) adapt_bg: [wgpu::BindGroup; 2],
    pub(crate) adapt_front: usize,
    /// The lamps' and headlights' light on what the camera looks at (`view_lamp_light`),
    /// which the eye adapts to by night.
    pub(crate) view_lamps: Option<f32>,
    /// The pre-exposure (natural log) as it follows the sky's.
    pub(crate) exposure: Option<f32>,
}

impl Exposure {
    /// The meter's and the adapted value's textures and their bind groups.
    pub(crate) fn new(
        device: &wgpu::Device,
        post: &PostPipelines,
        white_texture: &GpuTexture,
        hdr_format: wgpu::TextureFormat,
    ) -> Self {
        let one = wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        };
        let tiny = |label: &str| {
            target_2d(
                device,
                label,
                one,
                1,
                hdr_format,
                wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
            )
            .create_view(&Default::default())
        };
        let meter_view = tiny("exposure meter");
        let adapt_views = [tiny("exposure a"), tiny("exposure b")];
        // adapt_bg[k] reads the meter and adapt[k] (and draws into the other one)
        let adapt_bg = [0usize, 1].map(|k| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("exposure"),
                layout: &post.post_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: post.post_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&meter_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&post.post_sampler),
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
        Self {
            meter_view,
            adapt_views,
            adapt_bg,
            adapt_front: 0,
            view_lamps: None,
            exposure: None,
        }
    }
}
