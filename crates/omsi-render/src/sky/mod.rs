//! The sky: the atmosphere and the clouds' noise (CPU only, no `wgpu`), the air between the
//! sun and the ground, and the sky's GPU resources (the sky table, the reflection probe and
//! the sky cube) kept up to date frame by frame.

mod air;
pub mod atmosphere;
pub mod clouds;
mod probe;
mod resources;
mod update;

pub(crate) use air::cloud_sun_transmittance;
pub(crate) use air::CLOUD_SHADE_ADAPT;
pub(crate) use air::FOG_FALLOFF;
pub(crate) use air::MOON_RADIUS;
pub(crate) use air::SUN_RADIUS;
pub(crate) use air::{enhanced_sky_input, sky_input_differs};
pub(crate) use air::{enhanced_weather_fog, CLOUD_ORIGIN_PERIOD};
pub(crate) use probe::{PROBE_MIPS, SKY_CUBE_EVERY, SKY_CUBE_HISTORY, SKY_CUBE_ROUNDS};
pub(crate) use resources::SkyResources;
pub(crate) use update::EnhancedUniform;
