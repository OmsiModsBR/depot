//! The backends: the Direct3D 9Ex device (Windows) and the recording backend without a GPU.

#[cfg(windows)]
pub mod d3d9;
pub mod null;
