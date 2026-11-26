//! Tests moved out of `src/assets/texture.rs`.

use omsi_render::testing::internals::assets::texture::*;

#[test]
fn a_big_picture_is_halved_until_it_fits() {
    let data = omsi_texture::TextureData {
        width: 8,
        height: 4,
        format: omsi_texture::PixelFormat::Rgba8,
        levels: vec![vec![200; 8 * 4 * 4]],
        has_alpha: false,
        gpu_mips: true,
    };
    let small = fit_texture(&data, 2).unwrap();
    assert_eq!((small.width, small.height), (2, 1));
    assert_eq!(small.levels[0].len(), 2 * 4);
    assert!(fit_texture(&data, 8).is_none());
    // a picture of the game's own, and a size, the same way
    let img = omsi_texture::Image {
        width: 5000,
        height: 300,
        rgba: vec![9; 5000 * 300 * 4],
        has_alpha: true,
    };
    let small = fit_image(&img, 2048).unwrap();
    assert_eq!((small.width, small.height), (1250, 75));
    assert_eq!(small.rgba.len(), 1250 * 75 * 4);
    assert_eq!(fit_size(5000, 300, 2048), (1250, 75));
    assert_eq!(fit_size(2048, 16, 2048), (2048, 16));
    assert!(fit_image(&small, 2048).is_none());
}
