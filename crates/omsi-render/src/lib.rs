//! wgpu renderer.

mod assets;
pub mod atmosphere;
mod camera;
pub mod clouds;
mod draw;
mod frame;
mod gpu;
mod lighting;
mod passes;
mod pipelines;
mod puddles;
mod render;
mod renderer;
mod rt;
mod scene;
mod shaders;
mod surface;
mod targets;
mod triple;
// The public API, item by item: what the game (`omsi-app`) and the tests may use. A module
// moving inside the crate changes nothing here but the path; see ARCHITECTURE.md.
pub use assets::{
    fit_texture, prepare_mesh, prepare_meshes, prepare_texture, AlphaMode, GpuMesh, GpuTexture,
    Material, MaterialExtra, PbrMaps, PreparedMesh, PreparedTexture, TexAddressing,
};
pub use camera::{Camera, Vertex};
pub use gpu::{catch, catching, gl_backend, wait_gpu, ADAPTER_TEXTURE_MB, ADAPTER_VRAM_MB};
pub use lighting::{
    Corona, LightMode, Lighting, PointLight, SmokeParticle, LM_ATLAS_TILES, LM_TILE_PX,
};
pub use renderer::{
    RenderOptions, Renderer, AUTO_SCALE_PIXELS, DEPTH_FORMAT, LAMP_CODE_STRIDE, MAX_LAMPS_PER_MESH,
    MSAA, SHADOW_RANGE, SHADOW_RANGE_CLOSE, SHADOW_RANGE_FAR, SHADOW_SIZE,
};
pub use scene::{Instance, MaterialId, MeshId, RenderPhase, Scene, TextureId};
pub use surface::{winit_window, SurfaceState};
pub use triple::{panel_width, ScreenView, TripleScreen};

// Crate-internal names, shared by the modules through `use super::*`. Temporary: each domain
// imports by name once it reaches its folder (ARCHITECTURE.md), and these globs go.
use assets::*;
use camera::*;
use draw::*;
#[cfg(test)]
use frame::*;
use gpu::*;
use lighting::*;
use pipelines::*;
use render::*;
use renderer::*;
use scene::*;
use shaders::*;
use targets::*;

use anyhow::{anyhow, Context, Result};
use glam::{DVec3, Mat4, Vec3, Vec4};
#[cfg(test)]
use omsi_geometry::MeshData;
use std::collections::HashMap;
use std::sync::Arc;

/// The period the sky's cloud patterns repeat with (m): 5 x the cloud field (14 km), 8 x
/// its billow detail (8.75 km) and 28 x the high layer's (2.5 km) - the render origin is
/// taken modulo this for them, which keeps centimetres of precision in 32 bits.
const CLOUD_ORIGIN_PERIOD: f64 = 70000.0;

/// The enhanced path's post passes (post.wgsl `PostParams`).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PostUniform {
    a: [f32; 4],
    b: [f32; 4],
    c: [f32; 4],
    d: [f32; 4],
}

/// The enhanced lighting (enhanced_common.wgsl `Enhanced`).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct EnhancedUniform {
    exposure: [f32; 4],
    sun: [f32; 4],
    sh: [[f32; 4]; 9],
    ground: [f32; 4],
    fog: [f32; 4],
    fog_color: [f32; 4],
    weather: [f32; 4],
    lights: [f32; 4],
    sun_disc: [f32; 4],
    debug: [f32; 4],
    /// xyz where the sky cube was drawn from, relative to the camera (the dome looks the
    /// clouds up through it with the parallax taken out)
    eye: [f32; 4],
    /// x how bright an LED panel's dots burn (`Lighting::led_glow`), y how much of the
    /// mip chain an LED panel is held at (`Lighting::led_mips`)
    led: [f32; 4],
    /// xyz towards the moon, w its angular radius
    moon: [f32; 4],
    /// rgb the moon disc's irradiance at the ground before the clouds, w how much of the
    /// starry sky shows (the city's lamps and the haze wash the faint stars out)
    moon_disc: [f32; 4],
    /// rgb the sun's irradiance at the clouds' heights (`atmosphere::CLOUD_SUN_HEIGHTS`)
    cloud_sun: [[f32; 4]; 4],
    /// rgb the moonlight on a surface facing the moon (after the clouds), w 1 while the
    /// shadow maps are the moon's (`Lighting::casts_moon_shadows`)
    moon_light: [f32; 4],
    /// `Lighting::condensation`
    condensation: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct FogLampUniform {
    inv_view_proj: [[f32; 4]; 4],
    size: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SsaoUniform {
    inv_proj: [[f32; 4]; 4],
    params: [f32; 4],
    /// xy: the projection's off-centre shift (0 for the window's own symmetric frustum;
    /// a headset eye or a triple screen's side panel has one)
    shift: [f32; 4],
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_units;
