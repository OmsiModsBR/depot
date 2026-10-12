//! What the scene draws with on the GPU: meshes, textures and materials, and their life cycle
//! (freeing them and recycling their slots). Knows nothing of a frame or of the passes.

mod lifecycle;
mod material;
mod mesh;
mod texture;

pub use material::{AlphaMode, Material, MaterialExtra, PbrMaps, TexAddressing};
pub use mesh::{prepare_mesh, prepare_meshes, GpuMesh, PreparedMesh};
pub use texture::{fit_texture, prepare_texture, GpuTexture, PreparedTexture};

pub(crate) use lifecycle::Freed;
pub(crate) use material::{material_layout_entries, BindKey, MaterialUniform};
#[cfg(test)]
pub(crate) use mesh::{page_give, page_take};
pub(crate) use mesh::{MeshPage, RT_BUFFERS};
pub(crate) use texture::{fit_size, mip_pipeline, next_gen, texture_bytes, upload_texture};
