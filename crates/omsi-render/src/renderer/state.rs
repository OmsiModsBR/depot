//! The renderer's state: the `Renderer` struct, the formats and sizes of its targets, and
//! the shadows' ranges.

use super::options::RenderOptions;
use crate::assets::{AssetStore, TexAddressing};
use crate::effects::puddles;
use crate::gpu::{GpuContext, Samplers};
use crate::lighting::LightMapAtlas;
use crate::output::{Diagnostics, Presenter};
use crate::passes::DrawItem;
use crate::passes::{
    Exposure, PassPipelines, PostPipelines, PrepassPipelines, ShadowResources, SpriteResources,
    SsaoPass,
};
use crate::scene::{Camera, CullHistory, Layouts, TextureId};
use crate::sky::SkyResources;
use crate::targets::TargetCache;

/// What is drawn over the window's finished picture: the device, the queue, the frame's
/// command encoder, the window's picture and its size in pixels. It runs on the render
/// thread, inside the frame: it must not block.
pub type OverlayHook = Box<
    dyn FnMut(
            &wgpu::Device,
            &wgpu::Queue,
            &mut wgpu::CommandEncoder,
            &wgpu::TextureView,
            (u32, u32),
        ) + Send,
>;

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter_name: String,
    pub pass: PassPipelines,
    /// What only the enhanced mode draws with (its pipelines, the sky of its mirrors, the
    /// lamps' light in the fog); none where it is left out.
    pub own_mode: Option<Box<dyn crate::interface::ModeResources>>,
    pub reflection_pass: Option<PassPipelines>,
    pub camera_buf: wgpu::Buffer,
    /// The settings this renderer was built with.
    pub options: RenderOptions,
    pub puddles: Option<puddles::Pipelines>,
    pub post: PostPipelines,
    /// Enhanced lighting: its uniform (the sky table, the probe and the sky last computed are
    /// in `sky`, the exposure as it follows it in `adaptation`).
    pub enh_buf: wgpu::Buffer,
    /// The projection's width over height while a picture is drawn into a texture (see
    /// `render_to_texture`), whatever the texture's own shape.
    pub texture_aspect: Option<f32>,
    /// The rain reflection drawn into a texture this frame for a second renderer (see
    /// [`Renderer::set_rain_reflection`]): its texture, mirrored camera and aspect.
    pub rain_reflection: Option<(TextureId, Camera, f32)>,
    pub last_frame: Option<std::time::Instant>,
    /// The next frame stands alone (an offscreen picture): the exposure is there at once.
    pub instant_exposure: bool,
    pub started: std::time::Instant,
    /// Windy trees: how far the wind has carried its gusts (m, modulo 1000) and when.
    pub tree_gust_drift: std::cell::Cell<(glam::DVec2, f64)>,
    /// The addressing of the next material's textures (`[matl_texadress_*]`).
    pub address_next: std::cell::Cell<TexAddressing>,
    /// The next material is lit at night by the tile light maps (`[LightMapMapping]`, the
    /// splines) instead of the map's lamps.
    pub light_map_next: std::cell::Cell<bool>,
    /// The headset's pictures: the heading (degrees) the `[matl_envmap]` sphere maps are
    /// laid out by instead of each eye's view (see `set_env_heading`).
    pub env_heading: std::cell::Cell<Option<f32>>,
    /// The game's frame-rate governor on top of the render scale (1 = none; see
    /// `set_dynamic_scale`).
    pub dynamic_scale: std::cell::Cell<f32>,
    /// Sort the blended draws by origin distance alone, as before the camera-enclosing
    /// objects were drawn last (only for before/after pictures, `OMSI_BLEND_AB`).
    pub blend_by_origin: bool,
    /// Draw the models' `[isshadow]` shadow blobs (see [`RenderOptions::shadow_blobs`]).
    /// Settable while the game runs, so the graphics list can switch it off at once.
    pub shadow_blobs: bool,
    /// Seconds spent per stage of `render_inner` since start (OMSI_PROFILE).
    pub stats: std::cell::RefCell<std::collections::BTreeMap<&'static str, f64>>,
    /// Drawn over the window's finished picture, before it is delivered (see `OverlayHook`).
    pub overlay_hook: Option<OverlayHook>,
    /// A second renderer's copy of the assets and of each window picture's plan (see
    /// `mirror.rs`).
    pub asset_mirror: super::MirrorSlot,
    pub plan_sink: Option<crate::interface::PlanSink>,
    /// What the window's pictures drew since start, summed (OMSI_PROFILE): instances that
    /// passed the culling and draws per pass.
    pub counts: std::cell::RefCell<std::collections::BTreeMap<&'static str, f64>>,
    pub sprites: SpriteResources,
    pub prepass: PrepassPipelines,
    pub ssao: SsaoPass,
    pub layouts: Layouts,
    pub samplers: Samplers,
    pub assets: AssetStore,
    pub sky: SkyResources,
    pub adaptation: Exposure,
    pub shadows: ShadowResources,
    pub light_map: LightMapAtlas,
    pub targets: TargetCache,
    pub culling: CullHistory,
    pub presenter: Presenter,
    pub diagnostics: Diagnostics,
    /// Vectors a picture reuses from the one before (their capacity, never their contents).
    pub scratch: FrameScratch,
    /// Last: it holds the device's poller, which must be dropped after everything else.
    pub gpu: GpuContext,
}

/// Sun shadow map resolution when nothing else is asked for.
pub const SHADOW_SIZE: u32 = 2048;

/// Interior lamps one mesh may be lit by (OMSI: four; a model may list more in its
/// `[illumination_interior]`), and the step of the lamp code sent to the shaders (`first *
/// stride + count`, exact in the f32 it travels in for a quarter of a million lamp slots).
pub const MAX_LAMPS_PER_MESH: u32 = 63;

pub const LAMP_CODE_STRIDE: u32 = 64;

/// The vectors kept from one picture to the next, so that a picture reuses their capacity
/// instead of allocating again. Each is cleared before it is used.
#[derive(Default)]
pub struct FrameScratch {
    /// The draw list: the instances' indices in draw order.
    pub list: Vec<u32>,
    /// The main pass's draw items, the vehicle's, and the prepass's.
    pub items: Vec<DrawItem>,
    pub cab_items: Vec<DrawItem>,
    pub prepass_items: Vec<DrawItem>,
}
