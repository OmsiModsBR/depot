//! wgpu renderer.

mod assets;
mod effects;
mod gpu;
mod lighting;
mod output;
mod passes;
mod renderer;
mod scene;
mod shaders;
mod sky;
mod targets;
// The public API, item by item: what the game (`omsi-app`) and the tests may use. A module
// moving inside the crate changes nothing here but the path; see ARCHITECTURE.md.
pub use assets::{
    fit_texture, prepare_mesh, prepare_meshes, prepare_texture, AlphaMode, GpuMesh, GpuTexture,
    Material, MaterialExtra, PbrMaps, PreparedMesh, PreparedTexture, TexAddressing,
};
pub use gpu::{catch, catching, gl_backend, wait_gpu, ADAPTER_TEXTURE_MB, ADAPTER_VRAM_MB};
pub use lighting::{
    Corona, LightMode, Lighting, PointLight, SmokeParticle, LM_ATLAS_TILES, LM_TILE_PX,
};
pub use output::{winit_window, SurfaceState};
pub use renderer::{
    RenderOptions, Renderer, AUTO_SCALE_PIXELS, DEPTH_FORMAT, LAMP_CODE_STRIDE, MAX_LAMPS_PER_MESH,
    MSAA, SHADOW_RANGE, SHADOW_RANGE_CLOSE, SHADOW_RANGE_FAR, SHADOW_SIZE,
};
pub use scene::{
    panel_width, Camera, Instance, MaterialId, MeshId, RenderPhase, Scene, ScreenView, TextureId,
    TripleScreen, Vertex,
};
pub use sky::{atmosphere, clouds};

// The crate-wide names the tests (`tests.rs`, `tests_units.rs`) see through `use super::*`.
// The domains import by name (ARCHITECTURE.md); these globs go when the tests move to their
// domains.
#[cfg(test)]
use {
    assets::*,
    effects::*,
    glam::{DVec3, Mat4, Vec3},
    gpu::*,
    lighting::*,
    omsi_geometry::MeshData,
    output::*,
    passes::*,
    renderer::*,
    scene::*,
    shaders::*,
    sky::*,
};

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_units;
