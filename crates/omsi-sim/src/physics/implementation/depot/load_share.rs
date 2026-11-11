//! `D-1`: the weight shared about `[cog]`, as the model document `docs/physics/depot-model.md`
//! has it. The struts turn the body about its centre of gravity, so a `[cog]` ahead of the
//! middle presses the front springs more.

use crate::physics::domain::params::MIN_LOAD_SHARE;
use crate::physics::implementation::spec::{kinematic_mass_kg, mass_kg, rest_loads};
use glam::Vec3;
use omsi_vehicle::Vehicle;

/// The static load on each wheel of each axle (N): the weight shared so that the axle loads
/// balance about `[cog]`, `Σ F_i (y_i - cog_y) = 0` (a least-squares split when there are more
/// than two axles). A wheel carries no less than nothing - an OMSI strut only pushes.
pub(in crate::physics) fn wheel_rest_loads(def: &Vehicle) -> Vec<f32> {
    let cog_y = def.cog.map(|c| c[1]).unwrap_or(0.0);
    rest_loads(def, mass_kg(def), 0.0, cog_y)
}

/// What a strut's force adds to the body's torque (body frame) beyond its own lever about
/// `[cog]`: nothing, the force acts at the wheel and turns the body about its centre of
/// gravity.
pub(in crate::physics) fn strut_reference_torque(_cog: Vec3, _n: f32) -> Vec3 {
    Vec3::ZERO
}

/// The static load on each wheel of each axle (N) as the kinematic model (the AI's copies and
/// their parts) stands on it: with its mass, each wheel taking at least a share of an even
/// split, the weight shared about `[cog]` (`DA-6`).
pub(in crate::physics) fn kinematic_wheel_rest_loads(def: &Vehicle) -> Vec<f32> {
    let cog_y = def.cog.map(|c| c[1]).unwrap_or(0.0);
    rest_loads(def, kinematic_mass_kg(def), MIN_LOAD_SHARE, cog_y)
}
