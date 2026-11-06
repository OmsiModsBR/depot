//! The types of the player: the player's bus with its controls and cameras, the driver on
//! the bus's `[drivpos]`, the walk about the bus, a rail vehicle bound to its track. Their
//! procedures are in `implementation/`.

use glam::DVec3;

/// Where on the track the vehicle is.
#[derive(Debug, Clone)]
pub(crate) struct RailDrive {
    pub lane: usize,
    pub s: f32,
    /// The vehicle faces the lane's direction (else it drives it backwards).
    pub along: bool,
    /// The track the vehicle has come along: (distance travelled, the origin's position),
    /// oldest first - where its coupled parts are placed (see `VehicleInstance::retrail`).
    pub(crate) trail: std::collections::VecDeque<(f64, DVec3)>,
    /// Distance travelled along the track (forward positive).
    pub(crate) u: f64,
}
