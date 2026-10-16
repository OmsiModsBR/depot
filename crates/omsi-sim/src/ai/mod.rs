//! How the body of an AI vehicle follows the way its planner lays out
//! (`traffic::AiState::way_point`): a road vehicle is a bicycle model steered by pure
//! pursuit and sitting on its springs, a rail vehicle rides on its two outer axles, an
//! aircraft flies along its path and banks into its turns.
//!
//! Placing the vehicle *on* the lane every frame, turned to the lane's heading, is what made
//! the traffic look wrong: the heading jumped at every lane sample and joint (the steering
//! was worked out from those jumps), the body turned about its middle so that the rear
//! wheels slid sideways through every bend, and a lane change was a sideways glide with a
//! made-up yaw. Here the heading only ever changes by speed × tan(steering) / wheelbase,
//! the rear axle never moves sideways, and the steering itself is rate limited.
//!
//! `road` drives and settles a road vehicle, `rail` and `air` the other two; `planner` is
//! what the traffic planner asks of the body before it moves (the curves out round
//! something standing ahead, and whether the vehicle gets past it).

mod air;
mod planner;
mod rail;
mod road;

pub use planner::{back_in_ramp, pull_out_ramps, pull_out_room, straight_pull_out, BACK_IN_LAT_ACCEL, PULL_OUT_ACCEL, PULL_OUT_CLEARANCE, PULL_OUT_WAIT};
pub use road::AI_RAY_UP;

use crate::rigid::{AiSpec, AiWheelSpec};
use crate::vehicle::VehicleInstance;
use glam::{DVec2, DVec3, Vec3};
use omsi_vehicle::Vehicle;

/// What kind of way the vehicle follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionKind {
    Road,
    Rail,
    Air,
}

/// The line the vehicle turns about (`[rot_pnt_long]`, normally the rear axle) and the
/// distance from it to the front axle: (rotation point, wheelbase), both in metres along
/// the body (see `rigid::ai_rotation_point`).
pub use crate::rigid::ai_rotation_point as rotation_point;

/// Unit vector (east, north) of a heading in degrees.
fn dir(h: f64) -> DVec2 {
    let r = h.to_radians();
    DVec2::new(r.sin(), r.cos())
}

fn wrap(d: f64) -> f64 {
    (d + 180.0).rem_euclid(360.0) - 180.0
}

#[derive(Debug, Clone)]
pub struct AiBody {
    pub kind: MotionKind,
    rot_long: f32,
    wheelbase: f32,
    front_long: f32,
    rear_long: f32,
    /// Largest front wheel angle (deg) and how fast the driver turns towards it (deg/s).
    max_steer: f32,
    steer_rate: f32,
    wheels: Vec<AiWheelSpec>,
    axle_count: usize,
    mass: f32,
    cog: f32,
    /// Moments of inertia about the lateral (pitch) and longitudinal (roll) axes (kg m²).
    inertia: [f32; 2],
    /// Stiffness and damping of the body in heave (N/m), pitch and roll (Nm/rad).
    spring: [f32; 3],
    damper: [f32; 3],
    started: bool,
    /// World position of the rotation point (x east, y north).
    rear: DVec2,
    /// Heading (deg, clockwise from north).
    pub heading: f64,
    /// Front wheel angle of the bicycle model (deg, + = right), rate limited and smoothed.
    pub steer: f32,
    steer_cmd: f32,
    last_speed: f32,
    /// Longitudinal and lateral acceleration (m/s², forward / right positive).
    pub a_long: f32,
    pub a_lat: f32,
    yaw_rate: f32,
    /// The body on its springs: height of the origin, pitch and roll (rad), their rates.
    z: f64,
    vz: f32,
    pitch: f32,
    vpitch: f32,
    roll: f32,
    vroll: f32,
    /// The plane the wheels stand on last frame (height, pitch, roll) and its rates.
    ground: Option<(f64, f32, f32)>,
    ground_rate: [f32; 3],
    /// Pose for the vehicle: origin, pitch and bank (deg, + = nose up / right side down).
    pub position: DVec3,
    pub pitch_deg: f32,
    pub bank_deg: f32,
    /// Per axle (left, right): how far the wheel hangs below its rest position against
    /// the body (m, the `Axle_Suspension_*` convention: negative = compressed).
    pub suspension: Vec<[f32; 2]>,
    /// Each wheel's contact height last frame (NaN until the first frame): where its next
    /// ground query starts from.
    contact_z: Vec<f64>,
}

impl AiBody {
    pub fn new(def: &Vehicle, kind: MotionKind) -> AiBody {
        let AiSpec { rot_long, wheelbase, front_long, rear_long, max_steer, heavy, wheels, axle_count, mass, cog, inertia, spring, damper } = AiSpec::from(def);
        AiBody {
            kind,
            rot_long,
            wheelbase,
            front_long,
            rear_long,
            max_steer,
            steer_rate: if heavy { crate::rigid::AI_STEER_RATE_HEAVY } else { crate::rigid::AI_STEER_RATE },
            wheels,
            axle_count,
            mass,
            cog,
            inertia,
            spring,
            damper,
            started: false,
            rear: DVec2::ZERO,
            heading: 0.0,
            steer: 0.0,
            steer_cmd: 0.0,
            last_speed: 0.0,
            a_long: 0.0,
            a_lat: 0.0,
            yaw_rate: 0.0,
            z: 0.0,
            vz: 0.0,
            pitch: 0.0,
            vpitch: 0.0,
            roll: 0.0,
            vroll: 0.0,
            ground: None,
            ground_rate: [0.0; 3],
            position: DVec3::ZERO,
            pitch_deg: 0.0,
            bank_deg: 0.0,
            suspension: vec![[0.0; 2]; axle_count],
            contact_z: Vec::new(),
        }
    }

    /// Put the body onto its way, standing still and straight. `way(d)` is the point `d`
    /// metres along the way from the vehicle's origin.
    pub fn place(&mut self, way: &dyn Fn(f32) -> DVec3, ground: Option<&dyn Fn(f64, f64) -> Option<f64>>, contact: Option<&dyn crate::rigid::Ground>, speed: f32) {
        self.started = false;
        self.ground = None;
        self.contact_z.clear();
        self.last_speed = speed;
        self.step(0.0, speed, way, ground, contact);
    }

    /// Follow the way for `dt` seconds at `speed` (m/s, the planner's speed).
    /// `contact`: what the wheels stand on (the faces under a height, as the player's wheels
    /// ask it); without it the plain height sampler `ground`.
    pub fn step(&mut self, dt: f32, speed: f32, way: &dyn Fn(f32) -> DVec3, ground: Option<&dyn Fn(f64, f64) -> Option<f64>>, contact: Option<&dyn crate::rigid::Ground>) {
        if dt > 0.0 {
            let a = (speed - self.last_speed) / dt;
            self.a_long += (a - self.a_long) * (dt / 0.15).min(1.0);
        }
        self.last_speed = speed;
        match self.kind {
            MotionKind::Road => {
                self.drive(dt, speed, way);
                self.settle(dt, way, ground, contact);
            }
            MotionKind::Rail => self.ride(way),
            MotionKind::Air => self.fly(dt, speed, way),
        }
    }

    /// Hand the pose to the vehicle: position and attitude, the accelerations its scripts
    /// read and each wheel's travel.
    pub fn apply(&self, v: &mut VehicleInstance) {
        // A road vehicle's body stands on the contact plane of its wheels; its static pose
        // over that plane (`[ai_deltaheight]` and the model's wheel geometry) comes on top,
        // once: the springs here only add their travel about it. Rail and air vehicles
        // follow their path's own height.
        let (lift, delta) = if self.kind == MotionKind::Road { v.ai_rest_offset() } else { (0.0, 0.0) };
        v.position = self.position + DVec3::new(0.0, 0.0, lift as f64);
        v.heading = self.heading;
        v.pitch = self.pitch_deg;
        v.bank = self.bank_deg;
        v.motion.accel = Vec3::new(self.a_lat, self.a_long, 0.0);
        for (axle, s) in v.motion.wheels.iter_mut().zip(&self.suspension) {
            axle[0].suspension = s[0] + delta;
            axle[1].suspension = s[1] + delta;
        }
    }
}

#[cfg(test)]
mod tests;
