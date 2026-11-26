//! Tests moved out of `src/sky/clouds.rs`.

use omsi_render::testing::internals::sky::clouds::*;

#[test]
fn noise_tiles_and_spans_its_range() {
    // the lattices repeat: one period on, the same value
    let a = worley([0.3 * 8.0, 0.7 * 8.0, 4.0], 8.0);
    let b = worley([0.3 * 8.0 + 8.0, 0.7 * 8.0, 4.0], 8.0);
    assert!((a - b).abs() < 1e-5);
    let a = value_noise([1.3, 2.7, 0.5], 4.0);
    let b = value_noise([5.3, 2.7, 0.5], 4.0);
    assert!((a - b).abs() < 1e-5);
    let shape = shape_map();
    assert_eq!(shape.len(), SHAPE_SIZE.trailing_zeros() as usize + 1);
    let r: Vec<u8> = shape[0].chunks(4).map(|p| p[0]).collect();
    let (lo, hi) = (
        r.iter().min().copied().unwrap_or(0),
        r.iter().max().copied().unwrap_or(0),
    );
    assert!(hi - lo > 120, "shape R spans {lo}..{hi}");
}
