//! What the rest of the game hands a vehicle's physics and reads of it: the driver's
//! [`Controls`], the frame's [`VehicleMotion`], what moves the vehicle ([`Dynamics`]) and its
//! physics as it stands ([`PhysicsState`]).

use super::{KinematicModel, RigidBody};
use glam::Vec3;

/// Driver inputs, all in `0..1` except steering in `-1..1` (positive = right).
#[derive(Debug, Clone, Copy, Default)]
pub struct Controls {
    pub throttle: f32,
    pub brake: f32,
    pub clutch: f32,
    pub steering: f32,
}

#[derive(Debug, Clone)]
pub struct WheelState {
    /// Longitudinal axle position (m, forward positive) and lateral offset (m, right positive).
    pub long: f32,
    pub lat: f32,
    pub radius: f32,
    pub driven: bool,
    pub rotation_deg: f32,
    pub rpm: f32,
    pub suspension: f32,
}

/// The vehicle's motion this frame, as the rest of the game reads it (the dashboard, the
/// sounds, the passengers, the network ...), whichever model moves the vehicle: its rigid
/// body, the kinematic model or the AI. Read-only outside the physics: it writes it once a
/// frame, together with the scripts' variables (see `VehicleInstance::motion`).
#[derive(Debug, Clone)]
pub struct VehicleMotion {
    /// Longitudinal speed, m/s (forward positive).
    pub speed: f32,
    /// The front wheels' steering angle, degrees, positive = right.
    pub steer_deg: f32,
    /// The body's acceleration (m/s², x right, y forward, z up) as its model has it: the
    /// rigid body's carries gravity, as an accelerometer reads it.
    pub accel: Vec3,
    /// What the scripts read as `A_Trans_*`: the acceleration without gravity.
    pub a_trans: Vec3,
    /// Per axle: (left, right) wheels.
    pub wheels: Vec<[WheelState; 2]>,
}

impl VehicleMotion {
    pub fn velocity_kmh(&self) -> f32 {
        self.speed * 3.6
    }
}

/// What moves a vehicle. (A simplified body for the AI is to come as a third kind.)
// (the rigid body unboxed: there is one a vehicle, and it moves out for its step and back
// every frame, which a box would allocate anew)
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum Dynamics {
    /// A rigid body on its wheels: the player's vehicle, and the one the launcher shows (see
    /// `crate::rigid`).
    Rigid(RigidBody),
    /// The kinematic model: the player's vehicle on rails, and the copies the AI and other
    /// games drive (their pose is set from outside, the model is not stepped).
    Kinematic(KinematicModel),
}

/// A vehicle's physics as it stands - what moves it, its motion and the driver's controls -
/// to be put back with `VehicleInstance::set_physics_state` (runs tried from the same start).
#[derive(Debug, Clone)]
pub struct PhysicsState {
    pub(crate) dynamics: Dynamics,
    pub(crate) motion: VehicleMotion,
    pub(crate) controls: Controls,
}
