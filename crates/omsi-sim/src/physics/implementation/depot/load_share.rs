//! `D-1`: the weight shared about `[cog]`, and `D-2`: the people aboard move it and add to the
//! inertia, as the model
//! document `docs/physics/depot-model.md` has them. The struts turn the body about its centre
//! of gravity, so a `[cog]` ahead of the middle - or a crowd at the front - presses the front
//! springs more.

use crate::physics::domain::params::MIN_LOAD_SHARE;
use crate::physics::domain::Payload;
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

/// `D-2`: the body's centre of gravity and inertia with what it carries, from the empty
/// body's (`[cog]` with `[schwerpunkt]`, mass in kg, `[momentofintertia]`): one rigid body
/// of the two. The centre is the mass-weighted mean of the empty body's and the payload's, so
/// that the people aboard press the axles and the side they stand nearer and raise or lower
/// it; the inertia about it is the empty body's and the payload's own spread, each moved to
/// the new centre (the parallel-axis rule, about the body's pitch, roll and yaw axes). Nothing
/// aboard, or a payload with no place, leaves the empty body as it is.
pub(in crate::physics) fn loaded_body(empty: (Vec3, f32, Vec3), load: Payload) -> (Vec3, Vec3) {
    let (empty_cog, empty_kg, empty_inertia) = empty;
    let total = empty_kg + load.kg;
    if load.kg <= 0.0 || total <= 0.0 || !load.centre.is_finite() {
        return (empty_cog, empty_inertia);
    }
    let cog = (empty_cog * empty_kg + load.centre * load.kg) / total;
    let spread = if load.spread.is_finite() {
        load.spread.max(Vec3::ZERO)
    } else {
        Vec3::ZERO
    };
    let inertia = empty_inertia
        + about_axes((empty_cog - cog) * (empty_cog - cog) * empty_kg)
        + about_axes((load.centre - cog) * (load.centre - cog) * load.kg + spread);
    (cog, inertia)
}

/// `D-4`: the body's pitch and roll frequency (1/s) with `mass` (kg) aboard: the empty body's
/// `empty_freq` on the same springs, slower by sqrt(empty mass / mass).
pub(in crate::physics) fn loaded_body_freq(empty_freq: f32, empty_mass: f32, mass: f32) -> f32 {
    if mass > 0.0 {
        empty_freq * (empty_mass / mass).max(0.0).sqrt()
    } else {
        empty_freq
    }
}

/// What second moments along x, y and z (kg m²) give about the body's x (pitch), y (roll) and
/// z (yaw) axes: about each axis, the two along the other axes.
fn about_axes(m: Vec3) -> Vec3 {
    Vec3::new(m.y + m.z, m.x + m.z, m.x + m.y)
}
