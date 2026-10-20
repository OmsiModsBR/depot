//! Foundation of the legacy Direct3D 9 renderer.
//!
//! This crate is deliberately isolated: nothing depends on it, the game still renders through
//! `omsi-render` (wgpu), and it declares no hardware as supported. It holds
//!
//! * [`backend`]: the small, platform-agnostic interface a renderer must satisfy, plus a
//!   recording [`backend::NullBackend`] used to test the interface without a GPU;
//! * [`d3d9`] (Windows only): opening a Direct3D 9 device, reading its capabilities, clearing
//!   an off-screen target and reading it back, and device-lost detection and reset.
//!
//! Drawing meshes, the OMSI alpha modes and the HLSL shaders are later steps; see README.md.

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
