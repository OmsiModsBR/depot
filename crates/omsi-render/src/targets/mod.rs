//! The render targets and their per-size caches: the enhanced path's HDR chain, the
//! multisampled and depth attachments, and the ambient-occlusion textures.

mod ao;
mod cache;
mod hdr;
mod puddles;

pub(crate) use cache::TargetCache;
pub(crate) use puddles::PuddleTargets;

/// A pass's bind group layout and uniform buffer, for the bind groups a target is read through.
#[derive(Clone, Copy)]
pub(crate) struct GroupParts<'a> {
    pub(crate) layout: &'a wgpu::BindGroupLayout,
    pub(crate) buf: &'a wgpu::Buffer,
}
