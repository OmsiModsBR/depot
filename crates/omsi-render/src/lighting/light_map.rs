//! The tile light map atlas: the light maps of the tiles around the camera.

/// The tile light maps around the camera.
pub(crate) struct LightMapAtlas {
    /// The tile light maps (`.map.LM.bmp`) of the 5x5 tiles around the camera, north up, and
    /// where that square lies (world x, y of its south-west corner, its side in metres).
    pub(crate) lm_atlas: wgpu::Texture,
    pub(crate) lm_atlas_view: wgpu::TextureView,
    pub(crate) lm_uniform: wgpu::Buffer,
    pub(crate) lm_place: std::cell::Cell<(f64, f64, f64)>,
}
