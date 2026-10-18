//! The sRGB transfer curve, on the CPU (the shaders have their own in WGSL).

/// An sRGB-encoded channel (0..1) in linear light.
pub fn to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// A linear-light channel (0..1) encoded as sRGB.
pub fn from_linear(l: f32) -> f32 {
    if l <= 0.003_130_8 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_curve_goes_both_ways() {
        assert_eq!(to_linear(0.0), 0.0);
        assert!((to_linear(1.0) - 1.0).abs() < 1e-6);
        assert!((to_linear(0.5) - 0.214).abs() < 1e-3);
        for i in 0..=255 {
            let c = i as f32 / 255.0;
            assert!((from_linear(to_linear(c)) - c).abs() < 1e-5, "{c}");
        }
    }
}
