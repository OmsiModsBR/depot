//! The diagnostic and device switches of the renderer, read once from the environment when
//! the renderer is first opened (`Renderer::open_on` asks for them first) and shared from
//! then on. The renderer reads nothing else from the environment, so what a run was started
//! with is in one place, and a frame never looks a variable up.
//!
//! Each field is the variable of the same name (`no_bc` is `OMSI_NO_BC`); what a variable
//! does is said where its field is read and in `omsi_cfg::flags`.

use omsi_cfg::flags;
use std::sync::OnceLock;

/// The diagnostic and device switches (see the module).
#[derive(Debug, Default)]
pub struct DebugOptions {
    pub basic_pipelines: bool,
    pub debug_cones: bool,
    pub debug_draws: bool,
    pub debug_exposure: bool,
    pub debug_flicker: bool,
    pub debug_fog_lamps: bool,
    pub debug_light_grid: bool,
    pub debug_shadow_far: bool,
    pub debug_sky: bool,
    pub no_cloud_shade: bool,
    pub debug_view_lamps: bool,
    pub full_gpu: bool,
    pub gl_texture_units: bool,
    pub gpu_timers: bool,
    pub gpu_timers_raw: bool,
    pub intel_full_gpu: bool,
    pub no_bc: bool,
    pub no_bounds_cache: bool,
    pub no_bundles: bool,
    pub no_cull: bool,
    pub no_glare: bool,
    pub no_main_split: bool,
    pub no_msaa_prepass: bool,
    pub no_poll_thread: bool,
    pub no_puddle_glass_depth: bool,
    pub no_puddle_vehicle: bool,
    pub no_render_pool: bool,
    pub no_texcompress: bool,
    pub only_surfaces: bool,
    pub profile: bool,
    pub shadow_far_every_frame: bool,
    pub shadow_near_every_frame: bool,
    pub day_air: Option<String>,
    pub debug_cull: Option<String>,
    pub debug_shadow: Option<String>,
    pub gpu_arrays: Option<String>,
    pub gpu_limits: Option<String>,
    pub meter: Option<String>,
    pub render_clock: Option<String>,
    pub skip_pipe: Option<String>,
    pub tone_contrast: Option<String>,
    pub debug_enhanced: Option<f32>,
    pub debug_puddles: Option<f32>,
    pub env_photo: Option<f32>,
    pub puddle_f0: Option<f32>,
    pub puddle_thickness: Option<f32>,
    pub surface_bias: Option<i32>,
}

impl DebugOptions {
    /// The switches the environment holds now.
    fn from_env() -> Self {
        Self {
            basic_pipelines: flags::BASIC_PIPELINES.is_set(),
            debug_cones: flags::DEBUG_CONES.is_set(),
            debug_draws: flags::DEBUG_DRAWS.is_set(),
            debug_exposure: flags::DEBUG_EXPOSURE.is_set(),
            debug_flicker: flags::DEBUG_FLICKER.is_set(),
            debug_fog_lamps: flags::DEBUG_FOG_LAMPS.is_set(),
            debug_light_grid: flags::DEBUG_LIGHT_GRID.is_set(),
            debug_shadow_far: flags::DEBUG_SHADOW_FAR.is_set(),
            debug_sky: flags::DEBUG_SKY.is_set(),
            no_cloud_shade: flags::NO_CLOUD_SHADE.is_set(),
            debug_view_lamps: flags::DEBUG_VIEW_LAMPS.is_set(),
            full_gpu: flags::FULL_GPU.is_set(),
            gl_texture_units: flags::GL_TEXTURE_UNITS.is_set(),
            gpu_timers: flags::GPU_TIMERS.is_set(),
            gpu_timers_raw: flags::GPU_TIMERS_RAW.is_set(),
            intel_full_gpu: flags::INTEL_FULL_GPU.is_set(),
            no_bc: flags::NO_BC.is_set(),
            no_bounds_cache: flags::NO_BOUNDS_CACHE.is_set(),
            no_bundles: flags::NO_BUNDLES.is_set(),
            no_cull: flags::NO_CULL.is_set(),
            no_glare: flags::NO_GLARE.is_set(),
            no_main_split: flags::NO_MAIN_SPLIT.is_set(),
            no_msaa_prepass: flags::NO_MSAA_PREPASS.is_set(),
            no_poll_thread: flags::NO_POLL_THREAD.is_set(),
            no_puddle_glass_depth: flags::NO_PUDDLE_GLASS_DEPTH.is_set(),
            no_puddle_vehicle: flags::NO_PUDDLE_VEHICLE.is_set(),
            no_render_pool: flags::NO_RENDER_POOL.is_set(),
            no_texcompress: flags::NO_TEXCOMPRESS.is_set(),
            only_surfaces: flags::ONLY_SURFACES.is_set(),
            profile: flags::PROFILE.is_set(),
            shadow_far_every_frame: flags::SHADOW_FAR_EVERY_FRAME.is_set(),
            shadow_near_every_frame: flags::SHADOW_NEAR_EVERY_FRAME.is_set(),
            day_air: flags::DAY_AIR.text(),
            debug_cull: flags::DEBUG_CULL.text(),
            debug_shadow: flags::DEBUG_SHADOW.text(),
            gpu_arrays: flags::GPU_ARRAYS.text(),
            gpu_limits: flags::GPU_LIMITS.text(),
            meter: flags::METER.text(),
            render_clock: flags::RENDER_CLOCK.text(),
            skip_pipe: flags::SKIP_PIPE.text(),
            tone_contrast: flags::TONE_CONTRAST.text(),
            debug_enhanced: flags::DEBUG_ENHANCED.number(),
            debug_puddles: flags::DEBUG_PUDDLES.number(),
            env_photo: flags::ENV_PHOTO.number(),
            puddle_f0: flags::PUDDLE_F0.number(),
            puddle_thickness: flags::PUDDLE_THICKNESS.number(),
            surface_bias: flags::SURFACE_BIAS.number(),
        }
    }
}

/// The switches of this run: read from the environment the first time anything asks (the
/// renderer's opening does, before it builds anything).
pub fn debug() -> &'static DebugOptions {
    static OPTIONS: OnceLock<DebugOptions> = OnceLock::new();
    OPTIONS.get_or_init(DebugOptions::from_env)
}
