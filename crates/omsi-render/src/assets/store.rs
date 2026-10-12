//! The renderer's asset store: its own textures, the mip maker, and the meshes and
//! materials waiting for the next frame or freed.

use super::lifecycle::Freed;
use super::texture::GpuTexture;
use crate::scene::MeshId;

/// The renderer's own textures, the mip maker, and the meshes and materials waiting or freed.
pub(crate) struct AssetStore {
    pub(crate) white_texture: GpuTexture,
    pub(crate) black_texture: GpuTexture,
    /// A tangent-space normal pointing straight out (the PBR normal map's stand-in).
    pub(crate) flat_normal_texture: GpuTexture,
    /// Mipmap generation on the GPU (a blit per level).
    pub(crate) mip_pipeline: wgpu::RenderPipeline,
    pub(crate) mip_layout: wgpu::BindGroupLayout,
    pub(crate) mip_sampler: wgpu::Sampler,
    /// Vertex data of changed meshes (skinned people, the driver) waiting for the next
    /// picture: (mesh, bytes). Written with one staging buffer and a copy each at the start
    /// of the frame - a `write_buffer` per mesh made wgpu create a staging buffer for every
    /// one of them, forty a frame with a crowd at a stop.
    pub(crate) pending_meshes: std::cell::RefCell<Vec<(MeshId, Vec<u8>)>>,
    /// What a freed mesh and a freed material hold (see `free_mesh`), made once.
    pub(crate) freed: std::cell::OnceCell<Freed>,
    /// Meshes share pages of buffers (the adapter draws with a base vertex).
    pub(crate) mesh_pages: bool,
}
