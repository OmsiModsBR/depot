//! A vehicle's physics: what moves a road vehicle, and what the rest of the game reads of its
//! motion.
//!
//! A vehicle moves by its [`Dynamics`]: the rigid body on its wheels (the player's vehicle) or
//! the [`KinematicModel`] (longitudinal forces, kinematic steering and the ground followed
//! under the wheels). Either way the physics writes the frame's [`VehicleMotion`], which
//! everything else reads, and hands the scripts their variables (`vehicle/physics_bridge.rs`).
//! The driver's [`Controls`] go in.
//!
//! The rigid body follows OMSI's own (OMSI uses ODE only for the objects a crash knocks
//! over): a six-degree-of-freedom body with mass, `[momentofintertia]` and `[schwerpunkt]`,
//! wheels on spring/damper suspensions (`achse_feder`, `achse_daempfer`, `achse_maxforce`,
//! measured where the tyres stand between `achse_minwidth` and `achse_maxwidth`, pushing from
//! `achse_maxwidth`), every axle steered towards the turning centre on `[rot_pnt_long]` with
//! `[inv_min_turnradius]` as the full lock, OMSI's grip across the tyres (the bus follows its
//! wheels until the bend asks more than the road's grip, then slides at mu x load), drive
//! torque and brake forces from the scripts, pitch and roll damped as OMSI damps them,
//! gravity, ground contact per wheel and impulse responses against obstacles.
//! `examples/handling.rs` measures what a `.bus` file makes of a bus.
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
//! The package has three parts:
//!
//! - `domain`: what the physics is about - the objects, their fields and what reading them
//!   gives (accessors, no computation): the body and its wheels, the ground, a hit, the
//!   joints of a train, the `.bus` file's values in SI units, the vehicle's controls and
//!   motion, the kinematic model, and the constants (`domain::params`, each with where its
//!   value comes from).
//! - `interface`: what the rest of the game uses, and the only way in. Its inputs are the
//!   `.bus` file (`omsi_vehicle::Vehicle`), the driver's [`Controls`], the scripts' drive and
//!   brakes ([`BodyInputs`], the arguments of [`RigidBody::step`] and [`step_train`]) and the
//!   ground ([`Ground`]); its outputs the [`VehicleMotion`], the bodies as they stand
//!   ([`RigidBody`], [`RigidWheel`]) and their hits ([`Impact`]). Everything it names is
//!   re-exported here, so a caller writes `physics::RigidBody`.
//! - `implementation`: the procedures, one module per domain, in the order of a step:
//!   `spec` (the one place a `.bus` file's physics is read), `ground` (what a tyre stands
//!   on), `suspension` (the static loads and each wheel on its strut), `wheel` (each wheel's
//!   own turning), `tyre` (the tyres' forces along and across), `body` (the body and its
//!   motion), `collide` (hits against obstacles), `train` (the stages in order for one body
//!   or an articulated train, [`step_train`]) and `kinematic` (the kinematic model's step).
//!   The buffers a step works in (contacts, the solver's rows) stay with the procedures that
//!   fill them.

mod domain;
mod implementation;
mod interface;

pub use interface::*;

#[cfg(test)]
mod tests;
