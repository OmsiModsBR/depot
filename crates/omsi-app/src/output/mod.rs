//! The graphics output of the game window: the "DirectX9 (D3D9Ex)" device that draws the
//! window instead of wgpu, the PCVR session and the cockpit navigator placed in it, the copies
//! of the mirrors laid over the picture, and the cameras the picture is drawn from.
//!
//! In three parts: `domain/` holds the state and the plain data (the driver's idle head, the
//! mirror panels, the navigator's placements), `interface/` what the rest of the game calls
//! on the output and what the output takes from it, and `implementation/` the devices, the
//! cameras and the game's side of them.

pub(crate) mod domain;
pub(crate) mod implementation;
mod interface;

pub(crate) use interface::*;
