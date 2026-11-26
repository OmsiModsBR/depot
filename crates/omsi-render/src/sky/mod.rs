//! The sky: the atmosphere and the clouds' noise (CPU only, no `wgpu`), the air between the
//! sun and the ground, and the sky's GPU resources (the sky table, the reflection probe and
//! the sky cube) kept up to date frame by frame.

pub mod air;
pub mod atmosphere;
pub mod clouds;
pub mod probe;
pub mod resources;
pub mod update;

pub use air::cloud_sun_transmittance;
pub use air::CLOUD_SHADE_ADAPT;
pub use air::FOG_FALLOFF;
pub use air::MOON_RADIUS;
pub use air::SUN_RADIUS;
pub use air::{enhanced_sky_input, sky_input_differs};
pub use air::{enhanced_weather_fog, CLOUD_ORIGIN_PERIOD};
pub use probe::{PROBE_MIPS, SKY_CUBE_EVERY, SKY_CUBE_HISTORY, SKY_CUBE_ROUNDS};
pub use resources::SkyResources;
pub use update::EnhancedUniform;
