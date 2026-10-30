//! What the launcher's window calls to prepare a start.

pub use super::super::domain::choice::Choice;
pub use super::super::domain::season::{date_in_month, season_month, weather_fits};
pub use super::super::implementation::airport::nearest_airport;
pub use super::super::implementation::duty::{trip_index_at, without_bus};
pub use super::super::implementation::hof::default_hof;
pub use super::super::implementation::store::{
    favourite_key, parse_favourites, read_favourites, write_favourites,
};
