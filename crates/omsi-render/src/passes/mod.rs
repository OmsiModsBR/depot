//! The passes of a frame, one module per pass: its pipelines, its uniform and its encoding,
//! with `draw` holding what the passes share (draw items, batches, pipeline codes).

mod depth;
mod draw;
mod fog_lamps;
mod glass;
mod main;
mod post;
mod shadow;
mod sky;

pub(crate) use depth::{ao_pipelines, prepass_pipelines, PrepassPipelines, SsaoPass};
pub(crate) use draw::{
    batch_items, culls_back_faces, depth_only_material, depth_prepass_kind, effective_render_phase,
    encode_batches, encode_batches_filtered, horizontal_sort_distance, in_scope,
    instance_depth_bias, nearest_by_origin, one_sided_primitive, origin_key, pipe_code, post_pass,
    record_bundles, run_parts, surface_depth_coverage, surface_instance_code, world_surface_phase,
    Batch, DrawItem, PIPE_ALPHA_TEST, PIPE_BLEND, PIPE_BLEND_NO_WRITE, PIPE_KINDS, PIPE_OPAQUE,
    PIPE_SURFACE_DEPTH, PIPE_TERRAIN_PAINT,
};
#[cfg(test)]
pub(crate) use fog_lamps::FogLampUniform;
pub(crate) use fog_lamps::{fog_lamps_pipelines, FogLampsPass};
pub(crate) use glass::encode_glass;
pub(crate) use main::{encode_main_pass, MainParts, PassPipelines, SpriteResources};
#[cfg(test)]
pub(crate) use post::PostUniform;
pub(crate) use post::{encode_enhanced_post, post_pipelines, Exposure, PostParts, PostPipelines};
pub(crate) use shadow::{ShadowResources, ShadowSchedule, SHADOW_CLOSE_MAX};
pub use shadow::{SHADOW_RANGE, SHADOW_RANGE_CLOSE, SHADOW_RANGE_FAR};
