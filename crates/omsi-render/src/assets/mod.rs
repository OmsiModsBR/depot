//! What the scene draws with on the GPU: meshes, textures and materials, and their life cycle
//! (freeing them and recycling their slots). Knows nothing of a frame or of the passes.

mod ids;
mod lifecycle;
mod material;
mod mesh;
mod store;
mod texture;

pub use ids::{MaterialId, MeshId, TextureId, Vertex};
pub use material::{AlphaMode, Material, MaterialExtra, PbrMaps, TexAddressing};
pub use mesh::{prepare_mesh, prepare_meshes, GpuMesh, PreparedMesh};
pub use texture::{fit_texture, prepare_texture, GpuTexture, PreparedTexture};

pub(crate) use material::{
    material_layout_entries, snow_texture_flag, BindKey, MaterialMaps, MaterialUniform,
};
pub(crate) use mesh::{
    make_meshes, mesh_page_bytes, MeshPage, MESH_PAGE_INDEX_BYTES, MESH_PAGE_VERTEX_BYTES,
    RT_BUFFERS,
};
#[cfg(test)]
pub(crate) use mesh::{page_give, page_take};
pub(crate) use store::AssetStore;
pub(crate) use texture::{fit_image, fit_size, next_gen, texture_bytes, upload_texture};
