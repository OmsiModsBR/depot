//! The frame's destinations, the only code that delivers a frame: the window, an image or a
//! texture, a headset's eyes, a triple screen's panels, and the diagnostic numbers.

mod diagnostics;
mod image;
mod triple;
mod window;
mod xr;

pub use window::{winit_window, SurfaceState};

pub(crate) use diagnostics::GpuTimers;
#[cfg(test)]
pub(crate) use window::snap_rect;
pub(crate) use window::{overlay_1x_pipelines, overlay_pipeline, upscale_pipelines};
