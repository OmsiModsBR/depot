//! The physics' procedures, one module per domain, in the order of a step: `spec` (what a
//! `.bus` file's values mean, the one place a file's physics is read), `ground` (what a tyre
//! stands on), `suspension` (the static loads and each wheel on its strut), `wheel` (each
//! wheel's own turning), `tyre` (the tyres' forces along and across), `body` (the body and its
//! motion), `collide` (hits against obstacles), `train` (the stages in order for one body or an
//! articulated train) and `kinematic` (the kinematic model's step).

pub(crate) mod body;
mod collide;
pub(crate) mod ground;
mod kinematic;
pub(crate) mod spec;
mod suspension;
pub(crate) mod train;
pub(crate) mod tyre;
mod wheel;

pub(crate) use spec::coupled_rolling_resistance;
pub use spec::{
    ai_rotation_point, kinematic_mass_kg, kinematic_rest_sag, kinematic_wheel_rest_loads, mass_kg,
    wheel_rest_loads, wheel_rest_loads_supported,
};
pub use train::{
    hang_followers, place_train, point_load_shares, step_train, step_train_with, support_rest_loads,
};
