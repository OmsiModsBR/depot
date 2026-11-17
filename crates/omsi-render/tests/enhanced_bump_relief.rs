//! A `[matl_bumpmap]` shows its relief in the Enhanced picture's light: it bends the normal
//! the sun, the lamps and the sky light the surface by, not only the reflection's.

mod common;

use common::bump_board::{board, centre, debug_renderer};

/// How far the board's normal (`OMSI_DEBUG_ENHANCED=3`) varies over its centre: the
/// standard deviation of its x and z channels, summed.
fn normal_spread(img: &[u8]) -> f32 {
    [0, 2]
        .iter()
        .map(|&ch| {
            let v = centre(img, ch);
            let mean = v.iter().sum::<f32>() / v.len() as f32;
            (v.iter().map(|x| (x - mean) * (x - mean)).sum::<f32>() / v.len() as f32).sqrt()
        })
        .sum()
}

#[test]
#[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
fn a_bump_map_bends_the_shading_normal() {
    let mut renderer = debug_renderer("3");
    let flat = normal_spread(&board(&mut renderer, 0.1, false));
    let bumped = normal_spread(&board(&mut renderer, 0.1, true));
    eprintln!("normal spread: {flat} without the bump map, {bumped} with it");
    assert!(flat < 2.0, "the flat board's normal varies: {flat}");
    assert!(
        bumped > flat + 10.0,
        "the bump map does not bend the shading normal: {flat} without, {bumped} with"
    );
}
