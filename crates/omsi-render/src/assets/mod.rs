//! What the scene draws with on the GPU: meshes, textures and materials, and their life cycle
//! (freeing them and recycling their slots). Knows nothing of a frame or of the passes.

pub mod ids;
pub mod lifecycle;
pub mod material;
pub mod mesh;
pub mod store;
pub mod texture;

pub use ids::{MaterialId, MeshId, TextureId, Vertex};
pub use material::{AlphaMode, Material, MaterialDesc, MaterialExtra, PbrMaps, TexAddressing};
pub use mesh::{prepare_mesh, prepare_meshes, GpuMesh, PreparedMesh};
pub use texture::{fit_texture, prepare_texture, GpuTexture, PreparedTexture};

pub use lifecycle::Freed;
pub use material::{
    material_layout_entries, snow_texture_flag, BindKey, MaterialMaps, MaterialUniform,
};
#[cfg(feature = "test-support")]
pub use mesh::mesh_bounds;
pub use mesh::{
    make_meshes, mesh_page_bytes, mesh_vertices, MeshPage, MESH_PAGE_INDEX_BYTES,
    MESH_PAGE_VERTEX_BYTES,
};
#[cfg(feature = "test-support")]
pub use mesh::{page_give, page_take};
pub use store::AssetStore;

use std::sync::atomic::{AtomicUsize, Ordering};

/// How many renderers have an asset mirror. While any has, meshes and textures prepared on
/// other threads ([`prepare_mesh`], [`prepare_texture`]) keep their CPU data for it: those run
/// without a renderer, so the setting is the process's. It is a count, so one renderer turning
/// its mirror off does not take the copies away from another.
static MIRRORS: AtomicUsize = AtomicUsize::new(0);

pub fn keep_cpu_copies() -> bool {
    MIRRORS.load(Ordering::Relaxed) > 0
}

pub fn mirror_started() {
    MIRRORS.fetch_add(1, Ordering::Relaxed);
}

pub fn mirror_stopped() {
    let _ = MIRRORS.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_sub(1));
}
pub use texture::RgbaRef;
pub use texture::{
    drop_top_levels, fit_image, fit_size, next_gen, texture_bytes, upload_data_texture,
    upload_speed_mb_s, upload_texture,
};
