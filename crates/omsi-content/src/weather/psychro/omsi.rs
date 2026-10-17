//! Magnus in base 10 as OMSI 2 uses it: 7.5 and 237.3 °C over water, 7.6 and 240.7 °C over
//! supercooled water below 0 °C (not over ice, which would hold some 10% less at -10 °C).

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
