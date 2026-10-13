//! The render targets and their per-size caches: the enhanced path's HDR chain, the
//! multisampled and depth attachments, and the ambient-occlusion textures.

mod ao;
mod cache;
mod hdr;
mod puddles;

pub(crate) use cache::TargetCache;
pub(crate) use puddles::PuddleTargets;
