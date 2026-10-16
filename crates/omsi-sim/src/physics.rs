//! What moves a road vehicle, and what the rest of the game reads of its motion.
//!
//! A vehicle moves by its [`Dynamics`]: the rigid body on its wheels of `crate::rigid` (the
//! player's vehicle) or the [`KinematicModel`] (longitudinal forces, kinematic steering and
//! the ground followed under the wheels). Either way the physics writes the frame's
//! [`VehicleMotion`], which everything else reads, and hands the scripts their variables
//! (`vehicle/physics_bridge.rs`). The driver's [`Controls`] go in.

use crate::rigid::{AxleSpec, PhysicsSpec, RigidBody};
use glam::Vec3;
use omsi_vehicle::Vehicle;

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
    /// Standing still on the wheels of `def`.
    pub fn at_rest(def: &Vehicle) -> VehicleMotion {
        let wheels = def
            .axles
            .iter()
            .map(|a| {
                let a = AxleSpec::of(a);
                let mk = |lat: f32| WheelState { long: a.long, lat, radius: a.shown_radius, driven: a.driven, rotation_deg: 0.0, rpm: 0.0, suspension: 0.0 };
                [mk(-a.half_width), mk(a.half_width)]
            })
            .collect();
        VehicleMotion { speed: 0.0, steer_deg: 0.0, accel: Vec3::ZERO, a_trans: Vec3::ZERO, wheels }
    }

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
    /// The kinematic model: the player's vehicle with `--physics simple` or on rails, and the
    /// copies the AI and other games drive (their pose is set from outside, the model is not
    /// stepped).
    Kinematic(KinematicModel),
}

/// The vehicle's mass from its `[mass]` (see `rigid::mass_kg`).
pub use crate::rigid::mass_kg;

/// A deliberately simple model: the forces along the vehicle (drive, brakes, rolling
/// resistance), the front wheels' angle eased towards the driver's, and the heading turned by
/// the curvature tan(angle) / wheelbase. The vehicle follows the ground under its wheels.
#[derive(Debug, Clone, Copy, Default)]
pub struct KinematicModel {
    pub mass_kg: f32,
    pub rolling_resistance: f32,
    /// From the rearmost axle to the frontmost (m, at least 1).
    pub wheelbase: f32,
    pub max_steer_deg: f32,
}

impl KinematicModel {
    pub fn from_definition(def: &Vehicle) -> KinematicModel {
        // (the rigid body's lock: inv_min_turnradius = tan(alpha_max) / s → alpha_max, with s
        // from the front axle to `[rot_pnt_long]`)
        let spec = PhysicsSpec::from(def);
        KinematicModel { mass_kg: spec.mass_kg, rolling_resistance: spec.file_rolling_resistance, wheelbase: spec.wheelbase, max_steer_deg: spec.max_steer_deg }
    }

    /// Advance `m` one step, the driver steering at `steering`. `drive_torque` is `M_Wheel`,
    /// `brake_forces` the per-wheel brake forces in the same order as `m.wheels`. Returns the
    /// displacement along the vehicle's forward axis and the heading change in degrees.
    pub fn step(&self, m: &mut VehicleMotion, steering: f32, dt: f32, drive_torque: f32, brake_forces: &[f32]) -> (f32, f32) {
        let dt = dt.clamp(0.0, crate::rigid::KINEMATIC_DT_MAX);
        // steering
        let target = steering.clamp(-1.0, 1.0) * self.max_steer_deg;
        let rate = self.max_steer_deg * 2.5 * dt;
        m.steer_deg += (target - m.steer_deg).clamp(-rate, rate);

        // forces along the forward axis
        let driven_radius = m.wheels.iter().find(|w| w[0].driven).or(m.wheels.first()).map(|w| w[0].radius).unwrap_or(0.5);
        let f_drive = drive_torque / driven_radius;
        let f_brake: f32 = brake_forces.iter().sum::<f32>().max(0.0);
        let f_roll = self.rolling_resistance.max(0.0);
        let v = m.speed;
        let resist = f_brake + f_roll;
        let mut f = f_drive;
        // resistances oppose motion; at rest they can only cancel the drive force
        if v.abs() > 0.05 {
            f -= resist * v.signum();
        } else if f_drive.abs() <= resist {
            f = 0.0;
            m.speed = 0.0;
        } else {
            f -= resist * f_drive.signum();
        }
        let a = f / self.mass_kg;
        let new_speed = v + a * dt;
        // resistances must not reverse the direction of travel
        if v.abs() > 0.05 && new_speed.signum() != v.signum() && f_drive.abs() <= resist {
            m.speed = 0.0;
        } else {
            m.speed = new_speed;
        }
        m.accel = Vec3::new(0.0, a, 0.0);
        let ds = m.speed * dt;

        // kinematic turning about the rotation point
        let curvature = m.steer_deg.to_radians().tan() / self.wheelbase;
        let dheading = (ds * curvature).to_degrees();

        // wheels
        for axle in m.wheels.iter_mut() {
            for w in axle.iter_mut() {
                let omega = m.speed / w.radius; // rad/s
                w.rpm = omega * 60.0 / std::f32::consts::TAU;
                w.rotation_deg = (w.rotation_deg + omega.to_degrees() * dt).rem_euclid(360.0);
            }
        }
        (ds, dheading)
    }
}

/// A vehicle's physics as it stands - what moves it, its motion and the driver's controls -
/// to be put back with `VehicleInstance::set_physics_state` (runs tried from the same start).
#[derive(Debug, Clone)]
pub struct PhysicsState {
    pub(crate) dynamics: Dynamics,
    pub(crate) motion: VehicleMotion,
    pub(crate) controls: Controls,
}
