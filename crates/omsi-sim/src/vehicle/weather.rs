//! What the weather does to a vehicle: the air's humidity and the road's grip.

pub use omsi_content::weather::psychro::relative_humidity;

use crate::physics::{surface_grip, RoadWeather};

/// The ground an AI vehicle on its route takes for its one grip (`KA-3`): a road of the
/// plainest kind, 0 (a texture without `[surface]`), that gets wet.
const ROUTE_GROUND: u8 = 0x20;

/// The grip of an AI vehicle on its route (`KA-3`): that of a road (`C-5`) in the weather
/// `street_cond` (`StreetCond`: 0 dry … 1 wet, 1 … 2 snow) at `temperature` (°C).
pub fn route_grip(street_cond: f32, temperature: f32) -> f32 {
    surface_grip(
        ROUTE_GROUND,
        &RoadWeather::from_street(street_cond, temperature),
    )
}
