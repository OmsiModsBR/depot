//! `D-1`: the weight shared about `[cog]`. Until the rule is written in (phase 7 of spec 002)
//! this is the Classic rule, so that splitting the engines moves no number.

use crate::physics::implementation::spec::{mass_kg, rest_loads};
use glam::Vec3;
use omsi_vehicle::Vehicle;

/// The static load on each wheel of each axle (N), as `classic::load_share` gives it for now.
pub(in crate::physics) fn wheel_rest_loads(def: &Vehicle) -> Vec<f32> {
    rest_loads(def, mass_kg(def), 0.0, 0.0)
}

/// What a strut's force `n` adds to the body's torque (body frame), as
/// `classic::load_share` gives it for now.
pub(in crate::physics) fn strut_reference_torque(cog: Vec3, n: f32) -> Vec3 {
    Vec3::new(cog.y * n, -cog.x * n, 0.0)
}
