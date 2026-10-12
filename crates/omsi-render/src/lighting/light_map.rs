//! The tile light map atlas: the baked light of the map's tiles.

use super::model::LM_TILE_PX;
use crate::renderer::Renderer;

impl Renderer {
    /// Put a tile's light map into its place of the atlas (`slot`: column from the west, row
    /// from the north, 0..5); a picture of another size is scaled to 256 px.
    pub fn set_light_map_tile(&self, slot: (u32, u32), img: Option<&omsi_texture::Image>) {
        let n = LM_TILE_PX as usize;
        let mut px = vec![0u8; n * n * 4];
        if let Some(img) = img.filter(|i| i.width > 0 && i.height > 0) {
            for y in 0..n {
                for x in 0..n {
                    let sx = x * img.width as usize / n;
                    let sy = y * img.height as usize / n;
                    let o = (sy * img.width as usize + sx) * 4;
                    px[(y * n + x) * 4..(y * n + x) * 4 + 4].copy_from_slice(&img.rgba[o..o + 4]);
                }
            }
        }
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.lm_atlas,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: slot.0 * LM_TILE_PX,
                    y: slot.1 * LM_TILE_PX,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &px,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(LM_TILE_PX * 4),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: LM_TILE_PX,
                height: LM_TILE_PX,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Where the light map atlas lies: its south-west corner (world x, y) and its side (m).
    pub fn set_light_map_place(&self, x: f64, y: f64, side: f64) {
        self.lm_place.set((x, y, side));
    }
}
