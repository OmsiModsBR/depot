//! The sky: the atmosphere and the clouds' noise (CPU only, no `wgpu`), the air between the
//! sun and the ground, and the sky's GPU resources (the sky table, the reflection probe and
//! the sky cube) kept up to date frame by frame.

mod air;
pub mod atmosphere;
pub mod clouds;
mod probe;
mod update;

pub(crate) use air::enhanced_weather_fog;
#[cfg(test)]
pub(crate) use air::{enhanced_sky_input, sky_input_differs};
pub(crate) use probe::{
    Probe, PROBE_MIPS, PROBE_SIZE, SKY_CUBE_EVERY, SKY_CUBE_HISTORY, SKY_CUBE_ROUNDS, SKY_CUBE_SIZE,
};
