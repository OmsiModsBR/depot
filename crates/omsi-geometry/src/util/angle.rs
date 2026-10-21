//! Angles in degrees.

/// `d` degrees brought into -180..180: the signed turn from one heading to another when
/// `d` is their difference.
pub fn wrap_deg(d: f64) -> f64 {
    (d + 540.0).rem_euclid(360.0) - 180.0
}

/// [`wrap_deg`] in single precision.
pub fn wrap_deg_f32(d: f32) -> f32 {
    (d + 540.0).rem_euclid(360.0) - 180.0
}

/// Signed difference `to - from` in degrees, the short way round (-180..180). It rounds
/// differently from [`wrap_deg`] of the difference, so the two are not interchangeable.
pub fn angle_diff(from: f64, to: f64) -> f64 {
    let mut diff = (to - from) % 360.0;
    if diff > 180.0 {
        diff -= 360.0;
    } else if diff < -180.0 {
        diff += 360.0;
    }
    diff
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turns_come_out_between_minus_and_plus_180() {
        assert_eq!(wrap_deg(10.0 - 350.0), 20.0);
        assert_eq!(wrap_deg(350.0 - 10.0), -20.0);
        assert_eq!(wrap_deg(180.0), -180.0);
        assert_eq!(wrap_deg(-725.0), -5.0);
        assert_eq!(wrap_deg_f32(370.0), 10.0);
    }

    #[test]
    fn the_difference_goes_the_short_way_round() {
        assert!((angle_diff(350.0, 10.0) - 20.0).abs() < 1e-9);
        assert!((angle_diff(10.0, 350.0) + 20.0).abs() < 1e-9);
        assert_eq!(angle_diff(90.0, 90.0), 0.0);
    }
}
