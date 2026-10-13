//! The life cycle of the scene's resources: freeing meshes, textures and materials again
//! and recycling their slots.

/// The placeholders freed scene slots share: an empty vertex and index buffer and a plain
/// material bind group. Making new ones for every freed slot (two buffers per mesh, a
/// buffer and a bind group per material) was most of a tile unload's time.
pub(crate) struct Freed {
    pub(crate) vertex_buf: wgpu::Buffer,
    pub(crate) index_buf: wgpu::Buffer,
    pub(crate) bind_group: wgpu::BindGroup,
    pub(crate) buf: wgpu::Buffer,
}
