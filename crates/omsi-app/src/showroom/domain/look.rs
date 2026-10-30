//! What the showroom shows: the bus, its paint and the light it stands in.

use std::path::PathBuf;

/// What the showroom shows.
#[derive(Clone, PartialEq, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Look {
    pub root: PathBuf,
    pub map: String,
    pub bus: String,
    pub paint: String,
    pub weather: String,
    /// Minutes of the day, and the date.
    pub time: i32,
    pub date: String,
}
