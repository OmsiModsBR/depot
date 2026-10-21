//! The crate's public API, name by name: everything the game (`omsi-app`) imports from
//! `omsi_render`. Moving code between the crate's modules must keep this compiling - a name
//! that disappears from the facade in `lib.rs` breaks this test before it breaks the game.

#[allow(unused_imports)]
use omsi_render::{
    catch, catching, fit_texture, gl_backend, panel_width, poll_gpu, prepare_mesh, prepare_meshes,
    prepare_texture, wait_gpu, winit_window, AlphaMode, AssetMirror, Camera, Corona, FramePlan,
    GpuMesh, GpuTexture, GpuWaitError, Instance, LightMode, Lighting, Material, MaterialExtra,
    MaterialId, MeshId, MirrorEvent, MirrorTexture, PbrMaps, PlanBatch, PlanKind, PlanSink,
    PointLight, PreparedMesh, PreparedTexture, RenderOptions, RenderPhase, Renderer, Scene,
    ScreenView, SmokeParticle, SurfaceState, TexAddressing, TextureId, TripleScreen, Vertex,
    ADAPTER_TEXTURE_MB, ADAPTER_VRAM_MB, AUTO_SCALE_PIXELS, DEPTH_FORMAT, LAMP_CODE_STRIDE,
    LM_ATLAS_TILES, LM_TILE_PX, MAX_LAMPS_PER_MESH, MSAA, SHADOW_RANGE, SHADOW_RANGE_CLOSE,
    SHADOW_RANGE_FAR, SHADOW_SIZE,
};

#[allow(unused_imports)]
use omsi_render::{atmosphere, clouds};

#[test]
fn the_public_api_keeps_its_names() {
    // The fields the game reads and writes directly stay fields of `Renderer`.
    fn _fields(r: &Renderer) {
        let _ = (&r.device, &r.queue, &r.adapter_name, &r.options);
        let _ = (&r.address_next, &r.light_map_next, &r.stats, &r.counts);
        let _ = (r.instant_exposure, r.shadow_blobs, r.blend_by_origin);
    }
}
