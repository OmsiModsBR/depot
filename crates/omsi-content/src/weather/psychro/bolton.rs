//! Magnus with Bolton's constants: 6.112 hPa, 17.67 and 243.5 °C.

/// The saturation vapour pressure (hPa) over water at `temp_c` °C.
pub fn saturation_vapour_pressure(temp_c: f32) -> f32 {
    6.112 * ((17.67 * temp_c) / (temp_c + 243.5)).exp()
}

/// The absolute humidity (g/m³) of air at `temp_c` °C and `relative` % humidity.
pub fn absolute_humidity(temp_c: f32, relative: f32) -> f32 {
    let vapour = saturation_vapour_pressure(temp_c) * relative.clamp(0.0, 100.0) / 100.0;
    (216.7 * vapour / (temp_c + 273.15).max(1.0)).max(0.0)
}

/// The relative humidity (%) of air at `temp_c` °C holding `absolute` g/m³.
pub fn relative_humidity(temp_c: f32, absolute: f32) -> f32 {
    let vapour = absolute.max(0.0) * (temp_c + 273.15).max(1.0) / 216.7;
    (vapour / saturation_vapour_pressure(temp_c).max(0.001) * 100.0).clamp(0.0, 100.0)
}

/// The dew point (°C) of air at `temp_c` °C and `relative` % humidity.
pub fn dew_point(temp_c: f32, relative: f32) -> f32 {
    let rh = (relative.clamp(0.1, 100.0) / 100.0).ln();
    let g = rh + 17.67 * temp_c / (243.5 + temp_c);
    243.5 * g / (17.67 - g)
}

/// Saturation specific humidity (g/kg) at a temperature (°C), at 1000 hPa.
pub fn q_sat(t: f32) -> f32 {
    let e = 6.112 * (17.67 * t / (t + 243.5)).exp();
    622.0 * e / (1000.0 - 0.378 * e)
}

/// Dew point (°C) of air with specific humidity q (g/kg), at 1000 hPa.
pub fn dew_point_q(q: f32) -> f32 {
    let e = (q.max(0.01) * 1000.0 / (622.0 + 0.378 * q.max(0.01))).max(1e-3);
    let g = (e / 6.112).ln();
    243.5 * g / (17.67 - g)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_humidities_go_both_ways() {
        for t in [-10.0, 0.0, 15.0, 30.0] {
            let abs = absolute_humidity(t, 60.0);
            assert!((relative_humidity(t, abs) - 60.0).abs() < 0.01, "{t}");
            assert!((dew_point(t, 100.0) - t).abs() < 0.01, "{t}");
            assert!((dew_point_q(q_sat(t)) - t).abs() < 0.05, "{t}");
        }
    }
}
