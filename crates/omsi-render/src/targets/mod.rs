//! The render targets and their per-size caches: the enhanced path's HDR chain, the
//! multisampled and depth attachments, and the ambient-occlusion textures.

pub mod ao;
pub mod cache;
pub mod hdr;
pub mod puddles;

pub use cache::TargetCache;
pub use puddles::PuddleTargets;

/// A pass's bind group layout and uniform buffer, for the bind groups a target is read through.
#[derive(Clone, Copy)]
pub struct GroupParts<'a> {
    pub layout: &'a wgpu::BindGroupLayout,
    pub buf: &'a wgpu::Buffer,
}
