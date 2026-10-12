//! wgpu renderer.

mod api;
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
// The public API, item by item: what the game (`omsi-app`) and the tests may use. Everything
// public comes through `api`; see ARCHITECTURE.md.
pub use api::{
    atmosphere, catch, catching, clouds, fit_texture, gl_backend, panel_width, prepare_mesh,
    prepare_meshes, prepare_texture, wait_gpu, winit_window, AlphaMode, Camera, Corona, GpuMesh,
    GpuTexture, Instance, LightMode, Lighting, Material, MaterialExtra, MaterialId, MeshId,
    PbrMaps, PointLight, PreparedMesh, PreparedTexture, RenderOptions, RenderPhase, Renderer,
    Scene, ScreenView, SmokeParticle, SurfaceState, TexAddressing, TextureId, TripleScreen, Vertex,
    ADAPTER_TEXTURE_MB, ADAPTER_VRAM_MB, AUTO_SCALE_PIXELS, DEPTH_FORMAT, LAMP_CODE_STRIDE,
    LM_ATLAS_TILES, LM_TILE_PX, MAX_LAMPS_PER_MESH, MSAA, SHADOW_RANGE, SHADOW_RANGE_CLOSE,
    SHADOW_RANGE_FAR, SHADOW_SIZE,
};

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
