//! The frame's destinations, the only code that delivers a frame: the window, an image or a
//! texture, a headset's eyes, a triple screen's panels, and the diagnostic numbers.

mod destination;
mod diagnostics;
mod image;
mod triple;
mod window;
mod xr;

pub use window::{winit_window, SurfaceState};

pub(crate) use destination::Output;
pub(crate) use diagnostics::Diagnostics;
pub(crate) use image::read_texture;
pub(crate) use triple::swap_triple_culling;
#[cfg(test)]
pub(crate) use window::snap_rect;
pub(crate) use window::Presenter;
