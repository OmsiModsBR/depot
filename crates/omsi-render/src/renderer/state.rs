//! The renderer's state: the `Renderer` struct, the formats and sizes of its targets, and
//! the shadows' ranges.
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
use crate::assets::TexAddressing;
use crate::effects::{puddles, rt};
use crate::passes::{PassPipelines, PostPipelines};

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter_name: String,
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
    pub(crate) overlay_pipeline: wgpu::RenderPipeline,
    pub(crate) overlay_layout: wgpu::BindGroupLayout,
    pub(crate) camera_buf: wgpu::Buffer,
    pub(crate) ao_sampler: wgpu::Sampler,
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
    /// The settings this renderer was built with.
    pub options: RenderOptions,
    pub(crate) puddles: Option<puddles::Pipelines>,
    pub(crate) post: PostPipelines,
    pub(crate) post_layout: wgpu::BindGroupLayout,
    pub(crate) post_buf: wgpu::Buffer,
    pub(crate) post_sampler: wgpu::Sampler,
    /// Enhanced lighting: its uniform (the sky table, the probe and the sky last computed are
    /// in `sky`, the exposure as it follows it in `adaptation`).
    pub(crate) enh_buf: wgpu::Buffer,
    /// The projection's width over height while a picture is drawn into a texture (see
    /// `render_to_texture`), whatever the texture's own shape.
    pub(crate) texture_aspect: Option<f32>,
    pub(crate) last_frame: Option<std::time::Instant>,
    /// The next frame stands alone (an offscreen picture): the exposure is there at once.
    pub instant_exposure: bool,
    pub(crate) started: std::time::Instant,
    /// Windy trees: how far the wind has carried its gusts (m, modulo 1000) and when.
    pub(crate) tree_gust_drift: std::cell::Cell<(glam::DVec2, f64)>,
    /// The addressing of the next material's textures (`[matl_texadress_*]`).
    pub address_next: std::cell::Cell<TexAddressing>,
    /// The next material is lit at night by the tile light maps (`[LightMapMapping]`, the
    /// splines) instead of the map's lamps.
    pub light_map_next: std::cell::Cell<bool>,
    /// The headset's pictures: the heading (degrees) the `[matl_envmap]` sphere maps are
    /// laid out by instead of each eye's view (see `set_env_heading`).
    pub(crate) env_heading: std::cell::Cell<Option<f32>>,
    /// The game's frame-rate governor on top of the render scale (1 = none; see
    /// `set_dynamic_scale`).
    pub(crate) dynamic_scale: std::cell::Cell<f32>,
    /// Sort the blended draws by origin distance alone, as before the camera-enclosing
    /// objects were drawn last (only for before/after pictures, `OMSI_BLEND_AB`).
    pub blend_by_origin: bool,
    /// Draw the models' `[isshadow]` shadow blobs (see [`RenderOptions::shadow_blobs`]).
    /// Settable while the game runs, so the graphics list can switch it off at once.
    pub shadow_blobs: bool,
    /// Seconds spent per stage of `render_inner` since start (OMSI_PROFILE).
    pub stats: std::cell::RefCell<std::collections::BTreeMap<&'static str, f64>>,
    /// What the window's pictures drew since start, summed (OMSI_PROFILE): instances that
    /// passed the culling and draws per pass.
    pub counts: std::cell::RefCell<std::collections::BTreeMap<&'static str, f64>>,
    /// Enhanced+: the ray tracer (on a device with ray queries, see `RenderOptions::ray_tracing`).
    pub(crate) rt: Option<rt::RayTracer>,
    pub(crate) layouts: Layouts,
    pub(crate) samplers: Samplers,
    pub(crate) assets: AssetStore,
    pub(crate) sky: SkyResources,
    pub(crate) adaptation: Exposure,
    pub(crate) shadows: ShadowResources,
    pub(crate) light_map: LightMapAtlas,
    pub(crate) targets: TargetCache,
    pub(crate) culling: CullHistory,
    pub(crate) presenter: Presenter,
    pub(crate) diagnostics: Diagnostics,
    /// Last: it holds the device's poller, which must be dropped after everything else.
    pub(crate) gpu: GpuContext,
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
