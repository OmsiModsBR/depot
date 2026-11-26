//! The renderer's asset store: its own textures, the mip maker, and the meshes and
//! materials waiting for the next frame or freed.

use super::lifecycle::Freed;
use super::texture::{mip_pipeline, upload_texture, GpuTexture};
use super::MeshId;

/// The renderer's own textures, the mip maker, and the meshes and materials waiting or freed.
pub struct AssetStore {
    pub white_texture: GpuTexture,
    pub black_texture: GpuTexture,
    /// A tangent-space normal pointing straight out (the PBR normal map's stand-in).
    pub flat_normal_texture: GpuTexture,
    /// Mipmap generation on the GPU (a blit per level).
    pub mip_pipeline: wgpu::RenderPipeline,
    pub mip_layout: wgpu::BindGroupLayout,
    pub mip_sampler: wgpu::Sampler,
    /// Vertex data of changed meshes (skinned people, the driver) waiting for the next
    /// picture: (mesh, bytes). Written with one staging buffer and a copy each at the start
    /// of the frame - a `write_buffer` per mesh made wgpu create a staging buffer for every
    /// one of them, forty a frame with a crowd at a stop.
    pub pending_meshes: std::cell::RefCell<Vec<(MeshId, Vec<u8>)>>,
    /// What a freed mesh and a freed material hold (see `free_mesh`), made once.
    pub freed: std::cell::OnceCell<Freed>,
    /// Meshes share pages of buffers (the adapter draws with a base vertex).
    pub mesh_pages: bool,
}

impl AssetStore {
    /// The renderer's own textures and the mip maker.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let white = omsi_texture::Image::solid([255, 255, 255, 255]);
        let white_texture = upload_texture(device, queue, &white, false);
        let black_texture = upload_texture(
            device,
            queue,
            &omsi_texture::Image::solid([0, 0, 0, 255]),
            false,
        );
        let flat_normal_texture = upload_texture(
            device,
            queue,
            &omsi_texture::Image::solid([128, 128, 255, 255]),
            false,
        );
        let (mip_layout, mip_pipeline, mip_sampler) = mip_pipeline(device);
        Self {
            mip_pipeline,
            mip_layout,
            mip_sampler,
            white_texture,
            black_texture,
            flat_normal_texture,
            pending_meshes: Default::default(),
            freed: std::cell::OnceCell::new(),
            mesh_pages: false,
        }
    }
}
