//! Light: the light model the game describes, its records on the GPU, exposure, street lamps
//! and headlights (the light grid, the lamps' shadows), coronas and smoke sprites, and the tile
//! light map. Knows nothing of a frame's passes.

mod coronas;
mod exposure;
mod lamps;
mod model;

pub use model::{
    Corona, LightMode, Lighting, PointLight, SmokeParticle, LM_ATLAS_TILES, LM_TILE_PX,
};

#[cfg(test)]
pub(crate) use coronas::{smoke_ground_fade, smoke_sprite};
#[cfg(test)]
pub(crate) use exposure::half_to_f32;
pub(crate) use exposure::{meter_tuning, tone_contrast, ExposureLog};
#[cfg(test)]
pub(crate) use lamps::glare_veil;
pub(crate) use lamps::{lamp_sky_glow, view_lamp_light, LampShadow, FAR_MAP_ASPECT, LAMP_SHADOWS};
pub(crate) use model::{debug_view, GpuCorona, CABIN_E, LAMP_E, WINDOW_RADIANCE};
#[cfg(test)]
pub(crate) use model::{drawn_by, gpu_light, GpuPointLight};
