//! The Direct3D 9Ex renderer of the "DirectX9 (D3D9Ex)" render option.
//!
//! The game uses it only when that option is chosen (Windows, off by default); otherwise it
//! renders through `omsi-render` (wgpu) as before. This crate declares no hardware as
//! supported. It holds
//!
//! * [`backend`]: the crate's own drawing layer (textures, meshes, batches, frames),
//!   platform-agnostic, plus a recording [`backend::NullBackend`] that tests it without a GPU;
//! * [`scene`]: the game's scene as this renderer takes it (assets under the game's ids, and
//!   frames of batches), the seam with `omsi-render`'s frame plan;
//! * [`d3d9`] (Windows only): the Direct3D 9Ex implementation, off-screen or in a window,
//!   with HLSL Shader Model 3.0 shaders compiled at run time.

pub mod backend;
pub mod caps;
pub mod scene;

#[cfg(windows)]
pub mod d3d9;

pub use backend::{
    AlphaMode, DrawBatch, FrameDesc, MeshDesc, MeshHandle, NullBackend, RenderBackend, TextureDesc,
    TextureHandle,
};
pub use caps::{Caps, ShaderModel};
pub use scene::{
    DrawKind, SceneBatch, SceneFrame, SceneLight, SceneMaterial, SceneTexture, TexFormat,
};

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
