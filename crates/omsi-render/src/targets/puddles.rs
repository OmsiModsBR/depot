//! The puddles' reflection targets, kept with the high-range targets of their size.

/// The puddles' reflection targets of one size (made by `effects::puddles`): the traced rays,
/// their filter passes and the picture with the reflections, with their bind groups.
pub(crate) struct PuddleTargets {
    pub(crate) size: (u32, u32),
    pub(crate) source_depth: wgpu::Texture,
    pub(crate) hit_depth: wgpu::Texture,
    pub(crate) hit_depth_view: wgpu::TextureView,
    pub(crate) vehicle_view: wgpu::TextureView,
    pub(crate) vehicle_depth: wgpu::TextureView,
    pub(crate) trace: wgpu::TextureView,
    pub(crate) blur: wgpu::TextureView,
    pub(crate) filtered: wgpu::TextureView,
    pub(crate) view: wgpu::TextureView,
    pub(crate) trace_bg: wgpu::BindGroup,
    pub(crate) blur_x_bg: wgpu::BindGroup,
    pub(crate) blur_y_bg: wgpu::BindGroup,
    pub(crate) resolve_bg: wgpu::BindGroup,
    pub(crate) down_bg: wgpu::BindGroup,
    pub(crate) tonemap_bg: [wgpu::BindGroup; 2],
    pub(crate) classic_bg: wgpu::BindGroup,
}
