//! The passes of a frame, one module per pass: its pipelines, its uniform and its encoding,
//! with `draw` holding what the passes share (draw items, batches, pipeline codes).

pub mod corona;
pub mod depth;
pub mod draw;
pub mod features;
pub mod glass;
pub mod main;
pub mod pipelines;
pub mod post;
pub mod scene_pipelines;
pub mod shadow;
pub mod sky;
pub mod snow;

pub use corona::CoronaBuilder;
pub use corona::{additive, screen};
pub use depth::{ao_pipelines, prepass_pipelines, PrepassPipelines, SsaoPass};
pub use draw::{
    batch_items, culls_back_faces, depth_only_material, depth_prepass_kind, effective_render_phase,
    encode_batches, encode_batches_filtered, horizontal_sort_distance, in_scope,
    instance_depth_bias, nearest_by_origin, one_sided_primitive, origin_key, pipe_code,
    record_bundles, run_parts, surface_depth_coverage, surface_instance_code, world_surface_phase,
    Batch, DrawItem, PostPass, PIPE_ALPHA_TEST, PIPE_BLEND, PIPE_BLEND_NO_WRITE, PIPE_KINDS,
    PIPE_OPAQUE, PIPE_SURFACE_DEPTH, PIPE_TERRAIN_PAINT,
};
pub use features::FrameFeatures;
pub use glass::encode_glass;
pub use main::{encode_main_pass, MainParts, PassPipelines, SpriteResources};
pub use pipelines::{PathBuilders, PathShaders};
pub use post::{post_pipelines, Exposure, PostPipelines, PostUniform};
pub use scene_pipelines::SceneBuilder;
pub use shadow::{shadow_pipe, ShadowResources, ShadowSchedule, LAMP_CASCADE, SHADOW_CLOSE_MAX};
pub use shadow::{SHADOW_RANGE, SHADOW_RANGE_CLOSE, SHADOW_RANGE_FAR};
pub use sky::SkyBuilder;
pub use snow::{snow_layout, SnowBuilder};
