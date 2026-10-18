//! Magnus's saturation curve in base 10 as OMSI 2 uses it: 7.5 and 237.3 °C over water, 7.6
//! and 240.7 °C over supercooled water below 0 °C (not over ice, which would hold some 10%
//! less at -10 °C), 6.1078 hPa at 0 °C. Within 0.15% of today's reference curve from 0 to
//! 40 °C, 0.35% at -20 °C.

/// Saturation vapour pressure at 0 °C (hPa).
pub const E0: f32 = 6.1078;

/// The curve's constants at `t` °C: over water from 0 °C, over supercooled water below.
pub fn constants(t: f32) -> (f32, f32) {
    if t >= 0.0 {
        (7.5, 237.3)
    } else {
        (7.6, 240.7)
    }
}

/// The saturation vapour pressure (hPa) at `t` °C.
pub fn saturation_vapour_pressure(t: f32) -> f32 {
    let (a, b) = constants(t);
    E0 * 10f32.powf(a * t / (b + t))
}

/// The temperature (°C) at which `e` hPa of vapour saturates the air: the dew point,
/// [`saturation_vapour_pressure`] the other way round (below 6.1078 hPa the dew point is
/// below 0 °C and the supercooled constants apply).
pub fn dew_point_of_pressure(e: f32) -> f32 {
    let x = (e.max(1e-4) / E0).log10();
    let (a, b) = if x >= 0.0 { (7.5, 237.3) } else { (7.6, 240.7) };
    b * x / (a - x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_curve_goes_both_ways() {
        assert_eq!(saturation_vapour_pressure(0.0), E0);
        // 23.39 hPa at 20 °C, 2.86 at -10 °C (over water)
        assert!((saturation_vapour_pressure(20.0) - 23.39).abs() < 0.03);
        assert!((saturation_vapour_pressure(-10.0) - 2.864).abs() < 0.01);
        for t in [-30.0, -10.0, -0.5, 0.0, 0.5, 15.0, 40.0] {
            let back = dew_point_of_pressure(saturation_vapour_pressure(t));
            assert!((back - t).abs() < 1e-3, "{t}: {back}");
        }
    }
}
