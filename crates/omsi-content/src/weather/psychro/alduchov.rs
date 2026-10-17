//! Magnus over water with Alduchov and Eskridge's constants: 6.1094 hPa, 17.625 and
//! 243.04 °C.

/// Saturation vapour density (g/m³) at `t` °C.
pub fn saturation(t: f32) -> f32 {
    let e = 6.1094 * (17.625 * t / (t + 243.04)).exp();
    216.7 * e / (t + 273.15)
}

/// The dew point (°C) of air at `t` °C holding `vapour` g/m³ of water: [`saturation`] the
/// other way round. A weather gives its humidity as that density (`Weather::temp.1`,
/// OMSI's `Weather_AbsHum`), not as a dew point.
pub fn dew_point(t: f32, vapour: f32) -> f32 {
    let e = vapour.max(1e-3) * (t + 273.15) / 216.7;
    let x = (e / 6.1094).ln();
    243.04 * x / (17.625 - x)
}
