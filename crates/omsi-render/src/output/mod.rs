//! The frame's destinations, the only code that delivers a frame: the window, an image or a
//! texture, a headset's eyes, a triple screen's panels, and the diagnostic numbers.

pub mod destination;
pub mod diagnostics;
pub mod image;
pub mod triple;
pub mod window;
pub mod xr;

pub use window::{winit_window, SurfaceState};

pub use destination::Output;
pub use diagnostics::Diagnostics;
pub use image::read_texture;
pub use triple::swap_triple_culling;
#[cfg(feature = "test-support")]
pub use window::snap_rect;
pub use window::Presenter;
