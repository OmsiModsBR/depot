//! Vehicles.

pub mod cabin;
pub mod hof;
pub mod hof_check;
pub mod paths;
pub mod sound;
pub mod vehicle;

pub use cabin::PassengerCabin;
pub use hof::Hof;
pub use paths::VehiclePaths;
pub use sound::{SoundCfg, SoundEntry, VolCurve};
pub use vehicle::{Axle, Camera, Vehicle, VehicleKind};
