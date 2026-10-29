//! The devices, the cameras and the game's side of them (the `impl App` parts).

pub(crate) mod camera;
#[cfg(windows)]
pub(crate) mod d3d9;
pub(crate) mod mirror_hud;
pub(crate) mod xr;
