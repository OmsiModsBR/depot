//! Bringing the sky up to date: taking in a new sky state, the enhanced path's per-frame sky
//! and exposure, and the sky textures of the classic path.

/// The enhanced lighting (enhanced_common.wgsl `Enhanced`).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct EnhancedUniform {
    pub(crate) exposure: [f32; 4],
    pub(crate) sun: [f32; 4],
    pub(crate) sh: [[f32; 4]; 9],
    pub(crate) ground: [f32; 4],
    pub(crate) fog: [f32; 4],
    pub(crate) fog_color: [f32; 4],
    pub(crate) weather: [f32; 4],
    pub(crate) lights: [f32; 4],
    pub(crate) sun_disc: [f32; 4],
    pub(crate) debug: [f32; 4],
    /// xyz where the sky cube was drawn from, relative to the camera (the dome looks the
    /// clouds up through it with the parallax taken out)
    pub(crate) eye: [f32; 4],
    /// x how bright an LED panel's dots burn (`Lighting::led_glow`), y how much of the
    /// mip chain an LED panel is held at (`Lighting::led_mips`)
    pub(crate) led: [f32; 4],
    /// xyz towards the moon, w its angular radius
    pub(crate) moon: [f32; 4],
    /// rgb the moon disc's irradiance at the ground before the clouds, w how much of the
    /// starry sky shows (the city's lamps and the haze wash the faint stars out)
    pub(crate) moon_disc: [f32; 4],
    /// rgb the sun's irradiance at the clouds' heights (`atmosphere::CLOUD_SUN_HEIGHTS`)
    pub(crate) cloud_sun: [[f32; 4]; 4],
    /// rgb the moonlight on a surface facing the moon (after the clouds), w 1 while the
    /// shadow maps are the moon's (`Lighting::casts_moon_shadows`)
    pub(crate) moon_light: [f32; 4],
    /// `Lighting::condensation`
    pub(crate) condensation: [f32; 4],
}
