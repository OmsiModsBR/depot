//! The renderer's state: the `Renderer` struct, the formats and sizes of its targets, and
//! the shadows' ranges.

use super::options::RenderOptions;
use crate::assets::{Freed, GpuTexture, TexAddressing};
use crate::effects::{puddles, rt};
use crate::gpu::DevicePoller;
use crate::lighting::ExposureLog;
use crate::output::GpuTimers;
use crate::passes::{PassPipelines, PostPipelines};
use crate::scene::{MeshId, ViewCulling};
use crate::sky::{atmosphere, Probe};
use crate::targets::{AoTargets, HdrTargets};
use glam::{DVec3, Mat4, Vec3};
use std::collections::HashMap;
use std::sync::Arc;

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter_name: String,
    pub(crate) camera_layout: wgpu::BindGroupLayout,
    pub(crate) material_layout: wgpu::BindGroupLayout,
    pub(crate) pass: PassPipelines,
    pub(crate) hdr_pass: Option<PassPipelines>,
    /// The enhanced sky tone-mapped into the plain pass's target: the sky of the mirrors
    /// of an Enhanced picture (sky_enhanced.wgsl `fs_enhanced_mirror`).
    pub(crate) sky_mirror_pipeline: Option<wgpu::RenderPipeline>,
    pub(crate) reflection_pass: Option<PassPipelines>,
    pub(crate) corona_bind_group: wgpu::BindGroup,
    /// The snowfall's parameters (snow.wgsl `SnowParams`) and their bind group.
    pub(crate) snow_buf: wgpu::Buffer,
    pub(crate) snow_bind_group: wgpu::BindGroup,
    /// The smoke texture (`Texture/rauch.tga`, see [`Renderer::set_smoke_texture`]).
    pub(crate) smoke_bind_group: wgpu::BindGroup,
    /// The coronas' pictures besides the standard glow (index = `Corona::texture`; entry 0
    /// unused).
    pub(crate) corona_textures: Vec<Option<wgpu::BindGroup>>,
    pub(crate) corona_layout: wgpu::BindGroupLayout,
    pub(crate) corona_sampler: wgpu::Sampler,
    pub(crate) sky_layout: wgpu::BindGroupLayout,
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
    pub(crate) overlay_pipeline: wgpu::RenderPipeline,
    pub(crate) overlay_layout: wgpu::BindGroupLayout,
    pub(crate) sampler: wgpu::Sampler,
    pub(crate) camera_buf: wgpu::Buffer,
    pub(crate) white_texture: GpuTexture,
    pub(crate) black_texture: GpuTexture,
    /// A tangent-space normal pointing straight out (the PBR normal map's stand-in).
    pub(crate) flat_normal_texture: GpuTexture,
    pub(crate) format: wgpu::TextureFormat,
    pub(crate) depth: Option<(wgpu::Texture, wgpu::TextureView, u32, u32)>,
    /// Multisampled colour and depth targets per size (the window, each mirror).
    pub(crate) msaa_targets: HashMap<(u32, u32), (wgpu::TextureView, wgpu::TextureView)>,
    /// Screen-space ambient occlusion: depth prepass and AO textures of the window's size.
    pub(crate) ao: Option<AoTargets>,
    pub(crate) ao_sampler: wgpu::Sampler,
    pub(crate) ao_layout: wgpu::BindGroupLayout,
    pub(crate) ao_buf: wgpu::Buffer,
    /// Depth prepass pipelines (plain, alpha-tested; each two-sided and culled),
    /// single-sampled, camera projection.
    /// Plain opaque, alpha-tested, and the opaque portions of blended transmap materials;
    /// each has a two-sided and a culled variant.
    pub(crate) prepass_pipelines: [wgpu::RenderPipeline; 6],
    /// The same, multisampled: the enhanced main pass's own depth laid first (see
    /// `render_inner`), so that its costly shading runs once per visible surface.
    pub(crate) prepass_msaa_pipelines: Option<[wgpu::RenderPipeline; 6]>,
    /// Ambient occlusion and its blur; none on OpenGL (GLES), whose shading language cannot
    /// read a depth texture texel by texel - the pipelines failed there, AO off or not (#422).
    pub(crate) ssao_pipeline: Option<wgpu::RenderPipeline>,
    pub(crate) blur_pipeline: Option<wgpu::RenderPipeline>,
    /// Enhanced: the lamps' light the fog scatters, added over the drawn picture (none on
    /// OpenGL and without storage buffers), its bind group layout and parameters.
    pub(crate) fog_lamps_pipeline: Option<[wgpu::RenderPipeline; 2]>,
    pub(crate) fog_lamps_layout: wgpu::BindGroupLayout,
    pub(crate) fog_lamps_buf: wgpu::Buffer,
    pub(crate) shadow_view: wgpu::TextureView,
    pub(crate) shadow_view_far: wgpu::TextureView,
    pub(crate) shadow_sampler: wgpu::Sampler,
    pub(crate) shadow_layout: wgpu::BindGroupLayout,
    /// [near opaque, near alpha-tested, far opaque, far alpha-tested, close opaque, close
    /// alpha-tested], then the street lamps' (6 + 2 k + kind; not with `basic_pipelines`)
    pub(crate) shadow_pipelines: Vec<wgpu::RenderPipeline>,
    /// The settings this renderer was built with.
    pub options: RenderOptions,
    /// Enhanced path: the HDR targets per size, the post pipelines and their resources.
    pub(crate) hdr_targets: HashMap<(u32, u32), HdrTargets>,
    pub(crate) puddles: Option<puddles::Pipelines>,
    pub(crate) post: PostPipelines,
    pub(crate) post_layout: wgpu::BindGroupLayout,
    pub(crate) post_buf: wgpu::Buffer,
    pub(crate) post_sampler: wgpu::Sampler,
    /// The metered mean log luminance (1x1), and the adapted value in two textures drawn
    /// into by turns (`adapt_front` is the current one), with their bind groups.
    pub(crate) meter_view: wgpu::TextureView,
    pub(crate) adapt_views: [wgpu::TextureView; 2],
    pub(crate) adapt_bg: [wgpu::BindGroup; 2],
    pub(crate) adapt_front: usize,
    /// OMSI_DEBUG_EXPOSURE: the adapted metering read back now and then, and logged.
    pub(crate) exposure_log: Option<ExposureLog>,
    /// Enhanced lighting: its uniform, the sky table, the sampler both use, the reflection
    /// probe, the sky last computed and the exposure as it follows it.
    pub(crate) enh_buf: wgpu::Buffer,
    pub(crate) sky_lut: wgpu::Texture,
    pub(crate) sky_lut_view: wgpu::TextureView,
    pub(crate) lin_sampler: wgpu::Sampler,
    pub(crate) probe: Option<Probe>,
    pub(crate) sky_state: Option<atmosphere::SkyState>,
    /// How much the lamps round the camera light the night sky, relative to a city's
    /// (`lamp_sky_glow`), as the sky is computed with it.
    pub(crate) city_glow: Option<f32>,
    /// The lamps' and headlights' light on what the camera looks at (`view_lamp_light`),
    /// which the eye adapts to by night.
    pub(crate) view_lamps: Option<f32>,
    /// A sky being computed on a helper thread, for this input.
    pub(crate) sky_job: Option<(
        atmosphere::SkyInput,
        std::sync::mpsc::Receiver<atmosphere::SkyState>,
    )>,
    /// The pre-exposure (natural log) as it follows the sky's.
    pub(crate) exposure: Option<f32>,
    /// The projection's width over height while a picture is drawn into a texture (see
    /// `render_to_texture`), whatever the texture's own shape.
    pub(crate) texture_aspect: Option<f32>,
    pub(crate) last_frame: Option<std::time::Instant>,
    /// The next frame stands alone (an offscreen picture): the exposure is there at once.
    pub instant_exposure: bool,
    /// Overlay pipeline without multisampling, for drawing the HUD after the post pass.
    pub(crate) overlay_pipeline_1x: wgpu::RenderPipeline,
    pub(crate) xr_ui_pipeline: wgpu::RenderPipeline,
    pub(crate) started: std::time::Instant,
    /// Windy trees: how far the wind has carried its gusts (m, modulo 1000) and when.
    pub(crate) tree_gust_drift: std::cell::Cell<(glam::DVec2, f64)>,
    /// `[matl_texadress_clamp]` (and border, mirror-once) and `[matl_texadress_mirror]`.
    pub(crate) clamp_sampler: wgpu::Sampler,
    pub(crate) mirror_sampler: wgpu::Sampler,
    /// The addressing of the next material's textures (`[matl_texadress_*]`).
    pub address_next: std::cell::Cell<TexAddressing>,
    /// The next material is lit at night by the tile light maps (`[LightMapMapping]`, the
    /// splines) instead of the map's lamps.
    pub light_map_next: std::cell::Cell<bool>,
    /// The tile light maps (`.map.LM.bmp`) of the 5x5 tiles around the camera, north up, and
    /// where that square lies (world x, y of its south-west corner, its side in metres).
    pub(crate) lm_atlas: wgpu::Texture,
    pub(crate) lm_atlas_view: wgpu::TextureView,
    pub(crate) lm_uniform: wgpu::Buffer,
    pub(crate) lm_place: std::cell::Cell<(f64, f64, f64)>,
    /// Mipmap generation on the GPU (a blit per level).
    pub(crate) mip_pipeline: wgpu::RenderPipeline,
    pub(crate) mip_layout: wgpu::BindGroupLayout,
    pub(crate) mip_sampler: wgpu::Sampler,
    /// Set by the device's error handler when something failed with multisampling on.
    pub(crate) gpu_error: Arc<std::sync::atomic::AtomicBool>,
    /// A GPU error while Enhanced+ traces rays: its ray tracing is left out from the next
    /// frame on (a frame whose commands the device refused shows nothing at all - left to
    /// go on, the picture stood still on the loading screen while the game ran behind it).
    pub(crate) rt_error: Arc<std::sync::atomic::AtomicBool>,
    /// The headset's pictures: the heading (degrees) the `[matl_envmap]` sphere maps are
    /// laid out by instead of each eye's view (see `set_env_heading`).
    pub(crate) env_heading: std::cell::Cell<Option<f32>>,
    /// The card ran out of memory since the last `take_out_of_memory`.
    pub(crate) out_of_memory: Arc<std::sync::atomic::AtomicBool>,
    /// Why the graphics device was lost (a driver reset, the card removed), once it was.
    pub(crate) device_lost: Arc<std::sync::Mutex<Option<String>>>,
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
    pub(crate) triple_culling: [ViewCulling; 3],
    /// Full-resolution current scene before rain films, in its original colour format.
    pub(crate) glass_picture: Option<wgpu::TextureView>,
    /// When each size of the size-keyed targets (scale, MSAA, HDR) was last asked for.
    pub(crate) target_use: HashMap<(u32, u32), std::time::Instant>,
    /// The game's frame-rate governor on top of the render scale (1 = none; see
    /// `set_dynamic_scale`).
    pub(crate) dynamic_scale: std::cell::Cell<f32>,
    /// `OMSI_DEBUG_FLICKER`: which near instances in view were drawn last frame.
    pub(crate) flicker: std::cell::RefCell<HashMap<usize, bool>>,
    /// Instances the main view drew last frame (a bit each): they keep being drawn a little
    /// past the size and distance limits, so that an object right at a limit does not pop
    /// in and out as the camera sways.
    pub(crate) cull_drawn: std::cell::RefCell<Vec<u64>>,
    /// The screen size each object (origin and radius) was judged by in the main view's last
    /// frame: all its meshes and LOD levels take the same one, so exactly one level of an
    /// object is drawn and it does not flip between levels with the view's jitter.
    pub(crate) object_sizes: std::cell::RefCell<hashbrown::HashMap<[u64; 4], f32>>,
    /// Cleared and reused as the next main view's object-size history.
    pub(crate) object_sizes_scratch: std::cell::RefCell<hashbrown::HashMap<[u64; 4], f32>>,
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
    /// Sort the blended draws by origin distance alone, as before the camera-enclosing
    /// objects were drawn last (only for before/after pictures, `OMSI_BLEND_AB`).
    pub blend_by_origin: bool,
    /// Draw the models' `[isshadow]` shadow blobs (see [`RenderOptions::shadow_blobs`]).
    /// Settable while the game runs, so the graphics list can switch it off at once.
    pub shadow_blobs: bool,
    /// GPU time per pass (OMSI_GPU_TIMERS, when the device has timestamp queries): the
    /// mirrors and the window's picture apart, each timed on its own.
    pub(crate) gpu_timers: [Option<GpuTimers>; 2],
    /// Seconds spent per stage of `render_inner` since start (OMSI_PROFILE).
    pub stats: std::cell::RefCell<std::collections::BTreeMap<&'static str, f64>>,
    /// What the window's pictures drew since start, summed (OMSI_PROFILE): instances that
    /// passed the culling and draws per pass.
    pub counts: std::cell::RefCell<std::collections::BTreeMap<&'static str, f64>>,
    pub(crate) profiling: bool,
    pub(crate) draw_audit_at: std::time::Instant,
    /// Reuse a small set of encoding workers instead of creating OS threads for each
    /// main/mirror picture. Keep these separate from simulation's worker queue.
    pub(crate) encoding_pool: Option<rayon::ThreadPool>,
    pub(crate) _device_poller: Option<DevicePoller>,
    /// Vertex data of changed meshes (skinned people, the driver) waiting for the next
    /// picture: (mesh, bytes). Written with one staging buffer and a copy each at the start
    /// of the frame - a `write_buffer` per mesh made wgpu create a staging buffer for every
    /// one of them, forty a frame with a crowd at a stop.
    pub(crate) pending_meshes: std::cell::RefCell<Vec<(MeshId, Vec<u8>)>>,
    /// What a freed mesh and a freed material hold (see `free_mesh`), made once.
    pub(crate) freed: std::cell::OnceCell<Freed>,
    /// Enhanced+: the ray tracer (on a device with ray queries, see `RenderOptions::ray_tracing`).
    pub(crate) rt: Option<rt::RayTracer>,
    /// Meshes share pages of buffers (the adapter draws with a base vertex).
    pub(crate) mesh_pages: bool,
}

pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Samples per pixel of the scene passes when nothing else is asked for (the shadow maps
/// stay single-sampled). Four samples take the staircase off every edge; the multisampled
/// colour target resolves into the window or the mirror texture at the end of the main pass.
pub const MSAA: u32 = 4;

/// Sun shadow map resolution when nothing else is asked for.
pub const SHADOW_SIZE: u32 = 2048;

/// Interior lamps one mesh may be lit by (OMSI: four; a model may list more in its
/// `[illumination_interior]`), and the step of the lamp code sent to the shaders (`first *
/// stride + count`, exact in the f32 it travels in for a quarter of a million lamp slots).
pub const MAX_LAMPS_PER_MESH: u32 = 63;

pub const LAMP_CODE_STRIDE: u32 = 64;

/// The enhanced pass's second target: r is 1 where the bus's own screens are
/// (`MaterialExtra::screen`), 0 elsewhere - the glow takes no light from them and FXAA
/// passes them through; g is 1 on an LED panel's own dots (`MaterialExtra::led`), which the
/// glow's source keeps and multiplies up (see `post.wgsl`). b carries the puddle's
/// reflected-light weight; a is blend coverage. Sharing this attachment avoids another
/// geometry pass or a full normal/material buffer just for water.
pub(crate) const MASK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// Enhanced+: the enhanced pass's third and fourth targets, what the ray-traced reflections
/// start from (rt.wgsl): the surface's normal (xyz) and how much it reflects (w), and its
/// view depth and roughness (rg) - all weighted by that reflection (premultiplied), so that
/// a pane blended over a wet road leaves the stronger reflection's surface in them.
pub(crate) const GBUF_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

pub(crate) const AUX_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg16Float;

/// The renderer is built for Enhanced+: its HDR pipelines and targets have the two above.
pub(crate) static RT_GBUF: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub(crate) fn rt_gbuf() -> bool {
    RT_GBUF.load(std::sync::atomic::Ordering::Relaxed)
}

pub(crate) const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// The colour targets of a pipeline drawing into `format`: Enhanced and classic puddle
/// shading draw into `HDR_FORMAT` with the screen mask beside it,
/// written by the scene's own shader only (`mask`), coverage-blended where the colour is.
pub(crate) fn color_targets(
    format: wgpu::TextureFormat,
    blend: Option<wgpu::BlendState>,
    write: wgpu::ColorWrites,
    mask: bool,
    gbuf: bool,
) -> Vec<Option<wgpu::ColorTargetState>> {
    let mut v = vec![Some(wgpu::ColorTargetState {
        format,
        blend,
        write_mask: write,
    })];
    if format == HDR_FORMAT {
        v.push(Some(wgpu::ColorTargetState {
            format: MASK_FORMAT,
            blend: blend.map(|_| wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent::REPLACE,
            }),
            write_mask: if mask {
                wgpu::ColorWrites::ALL
            } else {
                wgpu::ColorWrites::empty()
            },
        }));
        if rt_gbuf() {
            // (premultiplied by the reflection's weight, the shader's alpha)
            let over = blend.map(|_| wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
            });
            let write = if gbuf {
                wgpu::ColorWrites::ALL
            } else {
                wgpu::ColorWrites::empty()
            };
            v.push(Some(wgpu::ColorTargetState {
                format: GBUF_FORMAT,
                blend: over,
                write_mask: write,
            }));
            v.push(Some(wgpu::ColorTargetState {
                format: AUX_FORMAT,
                blend: over,
                write_mask: write,
            }));
        }
    }
    v
}

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
