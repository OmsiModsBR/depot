//! The Direct3D 9Ex renderer of the "DirectX9 (D3D9Ex)" render option.
//!
//! The game uses it only when that option is chosen (Windows, off by default); otherwise it
//! renders through `omsi-render` (wgpu) as before. This crate declares no hardware as
//! supported. It holds
//!
//! * [`backend`] (`interface/input.rs`): the crate's own drawing layer (textures, meshes,
//!   batches, frames), platform-agnostic, plus a recording [`NullBackend`]
//!   (`implementation/null.rs`) that tests it without a GPU;
//! * [`scene`] (with the rest of `domain/`): the game's scene as this renderer takes it
//!   (assets under the game's ids, and frames of batches), the seam with `omsi-render`'s frame
//!   plan;
//! * [`ui`]: the game's interface as it takes it (`omsi-ui`'s vertex lists, layers and draws,
//!   and the HUD's pictures over the scene);
//! * [`d3d9`] (Windows only, `implementation/d3d9/`): the Direct3D 9Ex implementation,
//!   off-screen or in a window, with HLSL Shader Model 3.0 shaders compiled at run time.

mod domain;
mod implementation;
mod interface;
#[cfg(feature = "test-support")]
pub mod testing;

// The modules keep the paths they always had.
pub use domain::{caps, glass_rain, render_scale, scene, ui, wet_road};
#[cfg(windows)]
pub use implementation::d3d9;
pub use interface::input as backend;

pub use caps::{Caps, ShaderModel};
pub use glass_rain::SceneRain;
pub use implementation::null::NullBackend;
pub use interface::input::{
    AlphaMode, DrawBatch, FrameDesc, MeshDesc, MeshHandle, RenderBackend, TextureDesc,
    TextureHandle,
};
pub use render_scale::Upscale;
pub use scene::{
    DrawKind, SceneBatch, SceneClouds, SceneCoronas, SceneFrame, SceneLamps, SceneLight,
    SceneMaterial, SceneSky, SceneSmoke, SceneTexture, TexAddress, TexFormat,
};
pub use ui::{UiDraw, UiFrame, UiLayer, UiOverlay, UI_VERTEX_FLOATS};

/// Errors a legacy backend reports. `DeviceLost` is recoverable through
/// [`RenderBackend::recover`]; the others are not.
#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error("the device is lost and must be recovered before further use")]
    DeviceLost,
    #[error("the device cannot be recovered yet; try again later")]
    DeviceNotReset,
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    #[error("backend failure: {0}")]
    Backend(String),
}

pub type Result<T> = std::result::Result<T, BackendError>;
