//! The humidity of the air: saturation, absolute and relative humidity, dew point.
//!
//! One saturation curve for the whole game, OMSI 2's own ([`magnus`]): the weather files'
//! humidity and the scripts' `Cabinair_relHum` come out as in the original, and the weather
//! model, the custom weather and the misting of the panes agree with them.

pub mod humidity;
pub mod magnus;

pub use humidity::*;
pub use magnus::{dew_point_of_pressure, saturation_vapour_pressure};
