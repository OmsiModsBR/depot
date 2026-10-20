//! Foundation of the legacy Direct3D 9 renderer.
//!
//! This crate is deliberately isolated: nothing depends on it, the game still renders through
//! `omsi-render` (wgpu), and it declares no hardware as supported. It holds
//!
//! * [`backend`]: the crate's internal drawing layer (textures, meshes, batches, frames),
//!   platform-agnostic, plus a recording [`backend::NullBackend`] that tests it without a GPU.
//!   The game will not call it directly: the seam with the game is planned at the level of
//!   the scene (the `Renderer` facade the game already uses), after the `omsi-render`
//!   refactor; see README.md;
//! * [`d3d9`] (Windows only): the Direct3D 9Ex implementation, off-screen or in a window,
//!   with HLSL Shader Model 3.0 shaders compiled at run time.

pub mod backend;
pub mod caps;

#[cfg(windows)]
pub mod d3d9;

pub use backend::{
    AlphaMode, DrawBatch, FrameDesc, MeshDesc, MeshHandle, NullBackend, RenderBackend, TextureDesc,
    TextureHandle,
};
pub use caps::{Caps, ShaderModel};

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
