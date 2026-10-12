//! The public types, constants and free functions, re-exported from the domains that own
//! them.

pub use crate::assets::{
    fit_texture, prepare_mesh, prepare_meshes, prepare_texture, AlphaMode, GpuMesh, GpuTexture,
    Material, MaterialExtra, PbrMaps, PreparedMesh, PreparedTexture, TexAddressing,
};
pub use crate::gpu::{catch, catching, gl_backend, wait_gpu, ADAPTER_TEXTURE_MB, ADAPTER_VRAM_MB};
pub use crate::lighting::{
    Corona, LightMode, Lighting, PointLight, SmokeParticle, LM_ATLAS_TILES, LM_TILE_PX,
};
pub use crate::output::{winit_window, SurfaceState};
pub use crate::renderer::{
    RenderOptions, Renderer, AUTO_SCALE_PIXELS, DEPTH_FORMAT, LAMP_CODE_STRIDE, MAX_LAMPS_PER_MESH,
    MSAA, SHADOW_RANGE, SHADOW_RANGE_CLOSE, SHADOW_RANGE_FAR, SHADOW_SIZE,
};
pub use crate::scene::{
    panel_width, Camera, Instance, MaterialId, MeshId, RenderPhase, Scene, ScreenView, TextureId,
    TripleScreen, Vertex,
};
pub use crate::sky::{atmosphere, clouds};
