//! What the weather does to a vehicle: the air's humidity and the road's grip.

pub use omsi_content::weather::psychro::relative_humidity;

/// The tyres' friction coefficient on the road under them: `street_cond` as the weather sets
/// it (0 dry … 1 wet, 1 … 2 snow from packed to fresh) and the air temperature (°C), below
/// which a wet road freezes (the stock "Ueberfrierende Naesse" weather). Dry asphalt 0.85,
/// wet 0.6, snow 0.3, black ice 0.12 - textbook values for truck tyres; OMSI keeps its
/// friction inside ode.dll and only hands `StreetCond` to the scripts, so these are not
/// read off it.
pub fn road_grip(street_cond: f32, temperature: f32) -> f32 {
    const DRY: f32 = 0.85;
    const WET: f32 = 0.6;
    const SNOW: f32 = 0.3;
    const ICE: f32 = 0.12;
    let c = street_cond.clamp(0.0, 2.0);
    if c <= 1.0 {
        let wet = DRY + (WET - DRY) * c;
        // a wet road at or below freezing turns to ice as the film freezes; a damp one is
        // black ice all over (the stock "Ueberfrierende Naesse": -6 degC, groundwet 0.31)
        if temperature <= 0.0 && c > 0.05 {
            let frozen = ((0.5 - temperature) / 3.0).clamp(0.0, 1.0) * (c / 0.25).min(1.0);
            wet + (ICE - wet) * frozen
        } else {
            wet
        }
    } else {
        WET + (SNOW - WET) * (c - 1.0)
    }
}
