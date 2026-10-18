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
}
