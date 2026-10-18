//! Small wgpu helpers the domains share.

/// A 2D texture with one mip level and no other view formats: a render target, or a
/// texture filled once (`size` may hold several layers).
pub(crate) fn target_2d(
    device: &wgpu::Device,
    label: &str,
    size: wgpu::Extent3d,
    sample_count: u32,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size,
        mip_level_count: 1,
        sample_count,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}

/// How many mip levels a full chain down to 1x1 has for an image this size.
pub(crate) fn mip_levels(width: u32, height: u32) -> u32 {
    (32 - width.max(height).leading_zeros()).max(1)
}
