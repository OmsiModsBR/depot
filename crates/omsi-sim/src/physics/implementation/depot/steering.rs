//! Steerable axles (spec 004): `D-5` to `D-7` of `docs/physics/depot-model.md`.

use crate::physics::domain::SteerAxle;
use omsi_vehicle::Vehicle;

/// The section's steerable axles, from the `achse_steer_*` parameters of its `[newachse]`
/// (`D-5`).
pub(in crate::physics) fn steer_axles(def: &Vehicle) -> Vec<SteerAxle> {
    def.axles
        .iter()
        .enumerate()
        .filter_map(|(axle, a)| {
            a.steer.map(|s| SteerAxle {
                axle,
                lock: s.lock_deg.to_radians(),
                centring: s.centring_deg_s.to_radians(),
            })
        })
        .collect()
}
