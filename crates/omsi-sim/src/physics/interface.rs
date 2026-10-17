//! What the rest of the game uses of the physics, and the only way in (re-exported by the
//! package, so a caller writes `physics::RigidBody`).
//!
//! In: the `.bus` file (`omsi_vehicle::Vehicle`, read by [`PhysicsSpec::from`],
//! [`RigidBody::from_definition`], [`KinematicModel::from_definition`] and [`AiSpec::from`]),
//! the driver's [`Controls`], what the scripts hand each body for a step ([`BodyInputs`], or
//! the arguments of [`RigidBody::step`]), the ground under the wheels ([`Ground`],
//! [`GroundProbe`]), the joints of a train ([`Joint`]) and the obstacles
//! ([`RigidBody::collide`]).
//!
//! Out: the frame's [`VehicleMotion`] (the kinematic model's [`KinematicModel::step`] writes
//! it; the rigid body's is read off [`RigidBody`] and its [`RigidWheel`]s), the hits
//! ([`Impact`]) and what a `.bus` file's values come to ([`PhysicsSpec`], [`AxleSpec`],
//! [`AiSpec`], the rest loads and masses below).

// The objects.
pub use super::domain::{
    AiSpec, AiWheelSpec, AxleSpec, BodyInputs, Controls, CoupledPart, Dynamics, Ground, GroundProbe, Impact, Joint,
    JointKind, KinematicModel, PhysicsSpec, PhysicsState, RigidBody, RigidWheel, VehicleMotion, WallFace, WheelState,
};

// What a `.bus` file's values come to.
pub use super::implementation::{
    ai_rotation_point, kinematic_mass_kg, kinematic_rest_sag, kinematic_wheel_rest_loads, mass_kg, wheel_rest_loads,
    wheel_rest_loads_supported,
};
pub(crate) use super::implementation::coupled_rolling_resistance;

// A train of bodies.
pub use super::implementation::{hang_followers, place_train, point_load_shares, step_train, step_train_with, support_rest_loads};

// The constants the rest of the game measures against.
pub use super::domain::params::{BUMP, CLIMB, CRASH_SPEED, DEFAULT_INERTIA_INV, DROOP, HUB_REACH, WALL_HEIGHT};
pub(crate) use super::domain::params::{AI_AXLE_STEER_MAX, AI_STEER_ARM_MIN, AI_STEER_RATE, AI_STEER_RATE_HEAVY, KINEMATIC_DT_MAX};
