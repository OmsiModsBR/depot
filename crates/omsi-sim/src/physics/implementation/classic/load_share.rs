//! `K-1`: the weight shared about the model's origin, as OMSI 2's struts turn the body.

use crate::physics::domain::params::MIN_LOAD_SHARE;
use crate::physics::domain::Payload;
use crate::physics::implementation::spec::{kinematic_mass_kg, mass_kg, rest_loads};
use glam::Vec3;
use omsi_vehicle::Vehicle;

/// The static load on each wheel of each axle (N): the weight shared so that the axle loads
/// balance about the model's origin - `[cog]` does not move it - (a least-squares split when
/// there are more than two axles). A wheel carries no less than nothing - an OMSI strut only
/// pushes -, however far the origin lies from its axle.
pub(in crate::physics) fn wheel_rest_loads(def: &Vehicle) -> Vec<f32> {
    rest_loads(def, mass_kg(def), 0.0, 0.0)
}

/// What a strut's force `n` adds to the body's torque (body frame) to turn it about the
/// model's origin and not about `[cog]`: the force acts at the wheel, about the centre of
/// gravity, so the lever of `cog` is put back.
pub(in crate::physics) fn strut_reference_torque(cog: Vec3, n: f32) -> Vec3 {
    Vec3::new(cog.y * n, -cog.x * n, 0.0)
}

/// The static load on each wheel of each axle (N) as the kinematic model (the AI's copies and
/// their parts) stands on it: with its mass, each wheel taking at least a share of an even
/// split, the weight shared about the model's origin as `K-1` shares it (`KA-4`: the static
/// sag the scripts read, worked out once when the vehicle appears).
pub(in crate::physics) fn kinematic_wheel_rest_loads(def: &Vehicle) -> Vec<f32> {
    rest_loads(def, kinematic_mass_kg(def), MIN_LOAD_SHARE, 0.0)
}

/// The body's centre of gravity and inertia with what it carries, from the empty body's
/// (`[cog]`, mass in kg, `[momentofintertia]`): the empty body's, wherever the payload sits -
/// the payload adds only its mass.
pub(in crate::physics) fn loaded_body(empty: (Vec3, f32, Vec3), _load: Payload) -> (Vec3, Vec3) {
    (empty.0, empty.2)
}

/// `K-5`: the body's pitch and roll frequency (1/s) with a payload: the empty body's.
pub(in crate::physics) fn loaded_body_freq(empty_freq: f32) -> f32 {
    empty_freq
}
