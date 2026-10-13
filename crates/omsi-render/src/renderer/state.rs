//! The renderer's state: the `Renderer` struct, the formats and sizes of its targets, and
//! the shadows' ranges.

use super::options::RenderOptions;
use crate::assets::{AssetStore, TexAddressing};
use crate::effects::{puddles, rt};
use crate::gpu::{GpuContext, Samplers};
use crate::lighting::LightMapAtlas;
use crate::output::{Diagnostics, Presenter};
use crate::passes::{
    Exposure, FogLampsPass, PassPipelines, PostPipelines, PrepassPipelines, ShadowResources,
    SpriteResources, SsaoPass,
};
use crate::scene::{CullHistory, Layouts};
use crate::sky::SkyResources;
use crate::targets::TargetCache;

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
    pub(crate) camera_buf: wgpu::Buffer,
    /// The settings this renderer was built with.
    pub options: RenderOptions,
    pub(crate) puddles: Option<puddles::Pipelines>,
    pub(crate) post: PostPipelines,
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
    pub(crate) sprites: SpriteResources,
    pub(crate) prepass: PrepassPipelines,
    pub(crate) ssao: SsaoPass,
    pub(crate) fog_lamps: FogLampsPass,
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

/// Sun shadow map resolution when nothing else is asked for.
pub const SHADOW_SIZE: u32 = 2048;

/// Interior lamps one mesh may be lit by (OMSI: four; a model may list more in its
/// `[illumination_interior]`), and the step of the lamp code sent to the shaders (`first *
/// stride + count`, exact in the f32 it travels in for a quarter of a million lamp slots).
pub const MAX_LAMPS_PER_MESH: u32 = 63;

pub const LAMP_CODE_STRIDE: u32 = 64;
