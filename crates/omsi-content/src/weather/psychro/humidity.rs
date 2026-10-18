//! Humidity in the units each part of the game keeps it in, all on OMSI 2's
//! [`magnus`](super::magnus) curve.

use super::magnus::{dew_point_of_pressure, saturation_vapour_pressure};

/// Grams of water per cubic metre for each hPa of vapour, over the temperature in kelvin
/// (the ideal gas law for water vapour; OMSI's 216.69).
const DENSITY_PER_HPA: f32 = 216.69;

/// The absolute humidity (g/m³) of air at `temp` °C with the dew point `dew` °C, as OMSI 2
/// makes `Weather_AbsHum` of a weather's `[temp]`: the second value there is the dew point
/// (Bodennebel 9/9 and Überfrierende Nässe -6/-6 are saturated, Starker Schneefall -6/-7.96),
/// not the humidity itself. Taken as it stood, a winter weather gave the scripts a negative
/// humidity - no breath of exhaust steam in the frost - and every weather a wrong one for the
/// heating's misting of the panes. The saturation pressure is Magnus's in base 10, with the
/// supercooled-water constants below 0 °C (chosen by the temperature, not the dew point, as
/// in the original).
pub fn absolute_humidity(temp: f32, dew: f32) -> f32 {
    let (a, b) = if temp < 0.0 {
        (7.6, 240.7)
    } else {
        (7.5, 237.3)
    };
    6.1078 * 216.69 * 10f32.powf(a * dew / (b + dew)) / (273.15 + temp)
}

/// The relative humidity (a fraction, 1 = saturated) of air at `t` °C holding `abs_hum` g/m³,
/// as OMSI computes `Cabinair_relHum` (the original, Magnus over water with 7.5/237.3,
/// over supercooled water with 7.6/240.7 below 0 °C). It was written as a percentage: 100
/// times what the scripts and the passengers (a humid saloon at over 0.9) expect.
pub fn relative_humidity(t: f32, abs_hum: f32) -> f32 {
    let (a, b) = if t >= 0.0 { (7.5, 237.3) } else { (7.6, 240.7) };
    let sat = 1323.480_3 * 10f32.powf(a * t / (b + t)) / (273.15 + t);
    if sat > 0.0 && sat.is_finite() {
        (abs_hum / sat).max(0.0)
    } else {
        0.0
    }
}

/// Saturation vapour density (g/m³) at `t` °C.
pub fn saturation_density(t: f32) -> f32 {
    DENSITY_PER_HPA * saturation_vapour_pressure(t) / (t + 273.15)
}

/// The dew point (°C) of air at `t` °C holding `vapour` g/m³ of water: [`saturation_density`]
/// the other way round. A weather gives its humidity as that density (`Weather::temp.1`,
/// OMSI's `Weather_AbsHum`), not as a dew point.
pub fn dew_point(t: f32, vapour: f32) -> f32 {
    dew_point_of_pressure(vapour.max(1e-3) * (t + 273.15) / DENSITY_PER_HPA)
}

/// The absolute humidity (g/m³) of air at `temp_c` °C and `relative` % humidity.
pub fn absolute_humidity_of_relative(temp_c: f32, relative: f32) -> f32 {
    let vapour = saturation_vapour_pressure(temp_c) * relative.clamp(0.0, 100.0) / 100.0;
    (DENSITY_PER_HPA * vapour / (temp_c + 273.15).max(1.0)).max(0.0)
}

/// The relative humidity (%) of air at `temp_c` °C holding `absolute` g/m³.
pub fn relative_humidity_percent(temp_c: f32, absolute: f32) -> f32 {
    let vapour = absolute.max(0.0) * (temp_c + 273.15).max(1.0) / DENSITY_PER_HPA;
    (vapour / saturation_vapour_pressure(temp_c).max(0.001) * 100.0).clamp(0.0, 100.0)
}

/// The dew point (°C) of air at `temp_c` °C and `relative` % humidity.
pub fn dew_point_of_relative(temp_c: f32, relative: f32) -> f32 {
    dew_point_of_pressure(saturation_vapour_pressure(temp_c) * relative.clamp(0.1, 100.0) / 100.0)
}

/// Saturation specific humidity (g/kg) at a temperature (°C), at 1000 hPa.
pub fn q_sat(t: f32) -> f32 {
    let e = saturation_vapour_pressure(t);
    622.0 * e / (1000.0 - 0.378 * e)
}

/// Dew point (°C) of air with specific humidity q (g/kg), at 1000 hPa.
pub fn dew_point_q(q: f32) -> f32 {
    let e = (q.max(0.01) * 1000.0 / (622.0 + 0.378 * q.max(0.01))).max(1e-3);
    dew_point_of_pressure(e)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_humidities_go_both_ways() {
        for t in [-10.0, 0.0, 15.0, 30.0] {
            let abs = absolute_humidity_of_relative(t, 60.0);
            assert!(
                (relative_humidity_percent(t, abs) - 60.0).abs() < 0.01,
                "{t}"
            );
            assert!((dew_point_of_relative(t, 100.0) - t).abs() < 0.01, "{t}");
            assert!((dew_point_q(q_sat(t)) - t).abs() < 0.05, "{t}");
            assert!(
                (dew_point(t, saturation_density(t)) - t).abs() < 0.01,
                "{t}"
            );
            // the cabin's relative humidity of saturated air is 1
            assert!(
                (relative_humidity(t, saturation_density(t)) - 1.0).abs() < 1e-3,
                "{t}"
            );
        }
        // a weather's dew point read back from the humidity it gives
        let abs = absolute_humidity(-6.0, -7.96);
        assert!(
            (dew_point(-6.0, abs) + 7.96).abs() < 0.01,
            "{}",
            dew_point(-6.0, abs)
        );
    }
}
