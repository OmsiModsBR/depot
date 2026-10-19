//! The tile light map atlas: the light maps of the tiles around the camera.

use super::model::{LM_ATLAS_TILES, LM_TILE_PX};
use crate::gpu::target_2d;

/// The tile light maps around the camera.
pub(crate) struct LightMapAtlas {
    /// The tile light maps (`.map.LM.bmp`) of the 5x5 tiles around the camera, north up, and
    /// where that square lies (world x, y of its south-west corner, its side in metres).
    pub(crate) lm_atlas: wgpu::Texture,
    pub(crate) lm_atlas_view: wgpu::TextureView,
    pub(crate) lm_uniform: wgpu::Buffer,
    pub(crate) lm_place: std::cell::Cell<(f64, f64, f64)>,
}

impl LightMapAtlas {
    /// The atlas, empty, and where it lies.
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let lm_atlas = target_2d(
            device,
            "light map atlas",
            wgpu::Extent3d {
                width: LM_ATLAS_TILES * LM_TILE_PX,
                height: LM_ATLAS_TILES * LM_TILE_PX,
                depth_or_array_layers: 1,
            },
            1,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );
        let lm_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("light map atlas place"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            lm_atlas_view: lm_atlas.create_view(&wgpu::TextureViewDescriptor::default()),
            lm_atlas,
            lm_uniform,
            lm_place: std::cell::Cell::new((0.0, 0.0, 0.0)),
        }
    }
}
