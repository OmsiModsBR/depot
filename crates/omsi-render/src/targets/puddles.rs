//! The puddles' reflection targets, kept with the high-range targets of their size.

/// The puddles' reflection targets of one size (made by `effects::puddles`): the traced rays,
/// their filter passes and the picture with the reflections, with their bind groups.
pub struct PuddleTargets {
    pub size: (u32, u32),
    pub source_depth: wgpu::Texture,
    pub hit_depth: wgpu::Texture,
    pub hit_depth_view: wgpu::TextureView,
    pub vehicle_view: wgpu::TextureView,
    pub vehicle_depth: wgpu::TextureView,
    pub trace: wgpu::TextureView,
    pub blur: wgpu::TextureView,
    pub filtered: wgpu::TextureView,
    pub view: wgpu::TextureView,
    pub trace_bg: wgpu::BindGroup,
    pub blur_x_bg: wgpu::BindGroup,
    pub blur_y_bg: wgpu::BindGroup,
    pub resolve_bg: wgpu::BindGroup,
    pub down_bg: wgpu::BindGroup,
    pub tonemap_bg: [wgpu::BindGroup; 2],
    pub classic_bg: wgpu::BindGroup,
}
