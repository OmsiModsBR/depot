//! The humidity of the air: saturation, absolute and relative humidity, dew point.
//!
//! Grouped by the Magnus constants each part of the game was written with, which still
//! differ and give slightly different numbers; choosing one set for all is a separate fix.
//!
//! * [`omsi`]: the original's, in base 10, over ice below 0 °C (the weather files' humidity
//!   and the cabin's `Cabinair_relHum`);
//! * [`bolton`]: 6.112 hPa, 17.67 and 243.5 °C (the custom weather and the weather model);
//! * [`alduchov`]: 6.1094 hPa, 17.625 and 243.04 °C (the misting of the panes).

pub mod alduchov;
pub mod bolton;
pub mod omsi;
