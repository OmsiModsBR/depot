//! Small math shared by the crates that draw and simulate, by subject: easing curves and a
//! fast pseudo-random sequence.

mod ease;
mod rng;

pub use ease::smoothstep;
pub use rng::{unit_f32, xorshift64};
