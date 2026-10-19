//! Small math shared by the crates that draw and simulate, by subject: angles, easing curves
//! and a fast pseudo-random sequence.

mod angle;
mod ease;
mod rng;

pub use angle::{angle_diff, wrap_deg, wrap_deg_f32};
pub use ease::smoothstep;
pub use rng::{unit_f32, xorshift64};
