//! Rigid-body vehicle dynamics after OMSI's own (OMSI uses ODE only for the
//! objects a crash knocks over): a six-degree-of-freedom body with mass,
//! `[momentofintertia]` and `[schwerpunkt]`, wheels on spring/damper suspensions
//! (`achse_feder`, `achse_daempfer`, `achse_maxforce`, measured where the tyres stand
//! between `achse_minwidth` and `achse_maxwidth`, pushing from `achse_maxwidth`), every axle
//! steered towards the turning centre on `[rot_pnt_long]` with `[inv_min_turnradius]` as the
//! full lock, OMSI's grip across the tyres (the bus follows its wheels until the bend asks
//! more than the road's grip, then slides at mu x load), drive torque and brake forces from
//! the scripts, pitch and roll damped as OMSI damps them, gravity, ground contact per wheel
//! and impulse responses against obstacles. `examples/handling.rs` measures what a `.bus`
//! file makes of a bus.
//!
//! Ride height. A `.bus` file has no height for its axles: OMSI hangs each wheel so that
//! its spring is *unloaded* when the tyre stands on the model's ground plane (z = 0), and
//! the body then sinks by what the load compresses the springs - mass share over
//! `achse_feder` times the scripts' `Axle_Springfactor`. That sag is the `ai_deltaheight`
//! the same files give for their AI copies (-0.10 m for the SD202 and the NL202, whose
//! springs take 22-28 kN at 240-280 kN/m; -0.15 m for the F90 lorry, 55 kN at 370 kN/m),
//! and it is what `Axle_Suspension` reports: the model lifts the wheel into its arch by the
//! compression. Every vehicle therefore sits at its own height, set by its own data - the
//! NL202's level control even pumps its bellows until the compression reads 0.105 m.
//!
//! The code follows the stages of a step: `ground` (what a tyre stands on), `suspension` (the
//! static loads and each wheel on its strut), `wheel` (each wheel's own turning), `tyre` (the
//! tyres' forces along and across), `body` (the body and its motion) and `collide` (hits
//! against obstacles), with the constants in `params` and what a `.bus` file's values mean in
//! `spec` (the one place a file's physics is read).
//! `train` runs the stages in order for one body or an articulated train ([`step_train`]).

mod body;
mod collide;
mod ground;
mod params;
mod spec;
mod suspension;
mod train;
mod tyre;
mod wheel;

pub use body::{CoupledPart, RigidBody};
pub use collide::Impact;
pub use ground::{Ground, GroundProbe, WallFace};
pub use params::{BUMP, CLIMB, CRASH_SPEED, DEFAULT_INERTIA_INV, DROOP, HUB_REACH, WALL_HEIGHT};
pub(crate) use params::{AI_AXLE_STEER_MAX, AI_STEER_ARM_MIN, AI_STEER_RATE, AI_STEER_RATE_HEAVY, KINEMATIC_DT_MAX};
pub use spec::{ai_rotation_point, mass_kg, wheel_rest_loads, wheel_rest_loads_supported, AiSpec, AiWheelSpec, AxleSpec, PhysicsSpec};
pub(crate) use spec::{coupled_mass_kg, coupled_rolling_resistance};
pub use train::{hang_followers, place_train, support_rest_loads, point_load_shares, step_train, BodyInputs, Joint, JointKind};
pub use wheel::RigidWheel;

// (what `train` takes from the stages)
use body::body_damping;
use tyre::{lin, Lateral, Substep, Work};

#[cfg(test)]
mod tests;
