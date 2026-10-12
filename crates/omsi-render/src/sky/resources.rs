//! The sky's GPU resources: the sky table, the clouds' noise, the reflection probe and the
//! sky last computed.

use super::atmosphere;
use super::probe::Probe;

/// The sky's GPU resources and the sky last computed.
pub(crate) struct SkyResources {
    pub(crate) sky_sampler: wgpu::Sampler,
    /// The enhanced clouds' noise (clouds.rs): the shape map, the detail volume and their
    /// repeating, mip-mapped sampler (sky bind group bindings 6-8).
    pub(crate) cloud_shape_view: wgpu::TextureView,
    pub(crate) cloud_detail_view: wgpu::TextureView,
    pub(crate) cloud_sampler: wgpu::Sampler,
    /// The shape map's second level (RGBA8), for the clouds' shadow on the street, and how
    /// much of the sun the clouds let through over the camera now (smoothed).
    pub(crate) cloud_shape_cpu: Vec<u8>,
    pub(crate) cloud_sun: Option<f32>,
    pub(crate) sky_mesh: (wgpu::Buffer, wgpu::Buffer, u32),
    pub(crate) sky_lut: wgpu::Texture,
    pub(crate) sky_lut_view: wgpu::TextureView,
    pub(crate) probe: Option<Probe>,
    pub(crate) sky_state: Option<atmosphere::SkyState>,
    /// How much the lamps round the camera light the night sky, relative to a city's
    /// (`lamp_sky_glow`), as the sky is computed with it.
    pub(crate) city_glow: Option<f32>,
    /// A sky being computed on a helper thread, for this input.
    pub(crate) sky_job: Option<(
        atmosphere::SkyInput,
        std::sync::mpsc::Receiver<atmosphere::SkyState>,
    )>,
}
