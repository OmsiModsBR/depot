//! Light: the light model the game describes, its records on the GPU, exposure, street lamps
//! and headlights (the light grid, the lamps' shadows), coronas and smoke sprites, and the tile
//! light map. Knows nothing of a frame's passes.

pub mod coronas;
pub mod exposure;
pub mod lamps;
pub mod light_map;
pub mod model;

pub use model::{
    Corona, LightMode, Lighting, PointLight, ShadingPath, SmokeParticle, LM_ATLAS_TILES, LM_TILE_PX,
};

#[cfg(feature = "test-support")]
pub use coronas::smoke_ground_fade;
pub use coronas::smoke_sprite;
pub use coronas::{SUN_GLARE_BRIGHTNESS, SUN_GLARE_DISTANCE, SUN_GLARE_SIZE};
#[cfg(feature = "test-support")]
pub use exposure::half_to_f32;
pub use exposure::{meter_tuning, tone_contrast, ExposureLog};
#[cfg(feature = "test-support")]
pub use lamps::glare_veil;
pub use lamps::LAMP_SHADOW_REACH;
pub use lamps::LIGHT_CELL;
pub use lamps::LIGHT_CELL_CAP;
pub use lamps::LIGHT_GRID_SIDE;
pub use lamps::{lamp_sky_glow, view_lamp_light, LampShadow, FAR_MAP_ASPECT, LAMP_SHADOWS};
pub use light_map::LightMapAtlas;
pub use model::{debug_view, smoothstep, GpuCorona, CABIN_E, LAMP_E, WINDOW_RADIANCE};
pub use model::{gpu_light, lights_up, GpuPointLight};
