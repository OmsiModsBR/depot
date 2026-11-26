use omsi_render_vanilla::testing::domain::render_scale::*;

#[test]
fn auto_and_full_size_draw_at_the_window_size() {
    for s in [0.0, -1.0, f32::NAN, f32::INFINITY, 1.0, 0.995, 1.7] {
        assert_eq!(effective_scale(s), None, "{s}");
    }
}

#[test]
fn scales_are_held_to_the_range() {
    assert_eq!(effective_scale(0.75), Some(0.75));
    assert_eq!(effective_scale(0.5), Some(0.5));
    assert_eq!(effective_scale(0.3), Some(MIN_SCALE));
}

#[test]
fn scaled_sizes_round_and_are_never_zero_or_larger() {
    assert_eq!(scaled_size(1920, 1080, 0.5), (960, 540));
    assert_eq!(scaled_size(1920, 1080, 0.75), (1440, 810));
    assert_eq!(scaled_size(1366, 768, 0.66), (902, 507));
    assert_eq!(scaled_size(1, 1, 0.5), (1, 1));
    assert_eq!(scaled_size(3, 1, 0.5), (2, 1));
    assert_eq!(scaled_size(0, 0, 0.5), (1, 1));
    let (w, h) = scaled_size(7, 5, 0.99);
    assert!(w <= 7 && h <= 5);
}

#[test]
fn the_mapping_puts_pixel_centres_on_pixel_centres() {
    // an output pixel centre at x + 0.5 lands on the input's x * r + 0.5 * r - 0.5 + ...:
    // for 2x, output 0 (centre 0.5) maps to input -0.25 (between input 0's centre 0.5 and
    // the edge), output 1 to 0.25
    let m = easu_mapping((4, 4), (8, 8));
    assert_eq!(m, [0.5, 0.5, -0.25, -0.25]);
    assert_eq!(0.0 * m[0] + m[2], -0.25);
    assert_eq!(1.0 * m[0] + m[2], 0.25);
}

#[test]
fn the_sharpening_factor_is_below_full_strength() {
    assert!((rcas_factor() - 0.25f32.exp2().recip()).abs() < 1e-6);
    assert!(rcas_factor() < 1.0 && rcas_factor() > 0.5);
}
