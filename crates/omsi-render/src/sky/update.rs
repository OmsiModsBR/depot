//! Bringing the sky up to date: taking in a new sky state, the enhanced path's per-frame sky
//! and exposure, and the sky textures of the classic path.

/// The enhanced lighting (enhanced_common.wgsl `Enhanced`).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct EnhancedUniform {
    pub exposure: [f32; 4],
    pub sun: [f32; 4],
    pub sh: [[f32; 4]; 9],
    pub ground: [f32; 4],
    pub fog: [f32; 4],
    pub fog_color: [f32; 4],
    pub weather: [f32; 4],
    pub lights: [f32; 4],
    pub sun_disc: [f32; 4],
    pub debug: [f32; 4],
    /// xyz where the sky cube was drawn from, relative to the camera (the dome looks the
    /// clouds up through it with the parallax taken out)
    pub eye: [f32; 4],
    /// x how bright an LED panel's dots burn (`Lighting::led_glow`), y how much of the
    /// mip chain an LED panel is held at (`Lighting::led_mips`)
    pub led: [f32; 4],
    /// xyz towards the moon, w its angular radius
    pub moon: [f32; 4],
    /// rgb the moon disc's irradiance at the ground before the clouds, w how much of the
    /// starry sky shows (the city's lamps and the haze wash the faint stars out)
    pub moon_disc: [f32; 4],
    /// rgb the sun's irradiance at the clouds' heights (`atmosphere::CLOUD_SUN_HEIGHTS`)
    pub cloud_sun: [[f32; 4]; 4],
    /// rgb the moonlight on a surface facing the moon (after the clouds), w 1 while the
    /// shadow maps are the moon's (`Lighting::casts_moon_shadows`)
    pub moon_light: [f32; 4],
    /// `Lighting::condensation`
    pub condensation: [f32; 4],
    /// rgb the sun's irradiance before the clouds over the camera, for the scene's direct
    /// light: the clouds' shade map (below) says how much of it each point gets. Where the
    /// device has no slot for the map, the sun after the clouds over the camera, as `sun`.
    pub sun_open: [f32; 4],
    /// xy the shade map's centre in the clouds' frame, z the mean light through the clouds,
    /// w 1 where there is a map to read (`sky/cloud_shade.rs`)
    pub cloud_shade: [f32; 4],
    /// x, y the heights of the cloud layer's base and top, zw how far the clouds have
    /// drifted since the map was drawn (m)
    pub cloud_layer: [f32; 4],
}
