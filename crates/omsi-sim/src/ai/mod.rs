//! How the body of an AI vehicle follows the way its planner lays out
//! (`traffic::AiState::way_point`), by engine (spec 002): [`AiMotion`] holds the body of the
//! engine a vehicle runs. `depot` is Project Depot Physics' body (`DA-1` to `DA-6`): a road
//! vehicle is a bicycle model steered by pure pursuit and sitting on its springs, a rail
//! vehicle rides on its two outer axles, an aircraft flies along its path and banks into its
//! turns. `classic` is OMSI Classic's body for road vehicles (`KA-1` to `KA-11`); rail vehicles and
//! aircraft keep the `depot` body in both engines.
//!
//! Placing the vehicle *on* the lane every frame, turned to the lane's heading, is what made
//! the traffic look wrong: the heading jumped at every lane sample and joint (the steering
//! was worked out from those jumps), the body turned about its middle so that the rear
//! wheels slid sideways through every bend, and a lane change was a sideways glide with a
//! made-up yaw. In `depot` the heading only ever changes by speed × tan(steering) / wheelbase,
//! the rear axle never moves sideways, and the steering itself is rate limited.

mod classic;
mod depot;
mod sweep;

pub use classic::{
    integrate, target_speed, within_detail_radius, AiClassic, DriveForces, Pedals, Regime,
    TARGET_HORIZON,
};
pub use depot::{
    back_in_ramp, pull_out_ramps, pull_out_room, straight_pull_out, AiBody, AI_RAY_UP,
    BACK_IN_LAT_ACCEL, PULL_OUT_ACCEL, PULL_OUT_CLEARANCE, PULL_OUT_WAIT,
};

use crate::vehicle::VehicleInstance;
use glam::DVec3;
use omsi_cfg::PhysicsMode;
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
/// the body (see `physics::ai_rotation_point`).
pub use crate::physics::ai_rotation_point as rotation_point;

/// The body of an AI vehicle, of the engine the vehicle runs.
#[derive(Debug, Clone)]
pub enum AiMotion {
    /// Project Depot Physics' body (`DA-1` to `DA-6`), and every engine's rail and air body.
    Depot(AiBody),
    /// OMSI Classic's body of a road vehicle (`KA-1` to `KA-11`).
    Classic(AiClassic),
}

impl AiMotion {
    /// The body of a vehicle `def` of `mode`: OMSI Classic's for a Classic road vehicle, the
    /// Depot body otherwise.
    pub fn new(def: &Vehicle, kind: MotionKind, mode: PhysicsMode) -> AiMotion {
        match (mode, kind) {
            (PhysicsMode::Classic, MotionKind::Road) => AiMotion::Classic(AiClassic::new(def)),
            _ => AiMotion::Depot(AiBody::new(def, kind)),
        }
    }

    /// The Classic body, when the vehicle runs one.
    pub fn classic_mut(&mut self) -> Option<&mut AiClassic> {
        match self {
            AiMotion::Classic(b) => Some(b),
            AiMotion::Depot(_) => None,
        }
    }

    /// What kind of way the vehicle follows.
    pub fn kind(&self) -> MotionKind {
        match self {
            AiMotion::Depot(b) => b.kind,
            AiMotion::Classic(_) => MotionKind::Road,
        }
    }

    /// The front wheel angle (deg, + = right).
    pub fn steer(&self) -> f32 {
        match self {
            AiMotion::Depot(b) => b.steer,
            AiMotion::Classic(b) => b.steer,
        }
    }

    /// The lateral acceleration (m/s², right positive).
    pub fn a_lat(&self) -> f32 {
        match self {
            AiMotion::Depot(b) => b.a_lat,
            AiMotion::Classic(b) => b.a_lat,
        }
    }

    /// Put the body onto its way, standing still and straight (see [`AiBody::place`]).
    pub fn place(
        &mut self,
        way: &dyn Fn(f32) -> DVec3,
        ground: Option<&dyn Fn(f64, f64) -> Option<f64>>,
        contact: Option<&dyn crate::physics::Ground>,
        speed: f32,
    ) {
        match self {
            AiMotion::Depot(b) => b.place(way, ground, contact, speed),
            AiMotion::Classic(b) => b.place(way, ground, contact, speed),
        }
    }

    /// Follow the way for `dt` seconds at `speed` (see [`AiBody::step`]).
    pub fn step(
        &mut self,
        dt: f32,
        speed: f32,
        way: &dyn Fn(f32) -> DVec3,
        ground: Option<&dyn Fn(f64, f64) -> Option<f64>>,
        contact: Option<&dyn crate::physics::Ground>,
    ) {
        match self {
            AiMotion::Depot(b) => b.step(dt, speed, way, ground, contact),
            AiMotion::Classic(b) => b.step(dt, speed, way, ground, contact),
        }
    }

    /// Hand the pose to the vehicle (see [`AiBody::apply`]).
    pub fn apply(&self, v: &mut VehicleInstance) {
        match self {
            AiMotion::Depot(b) => b.apply(v),
            AiMotion::Classic(b) => b.apply(v),
        }
    }

    /// How close the body would come to `obstacles` on a way it has not taken yet (see
    /// [`AiBody::sweep_clearance`]).
    #[allow(clippy::too_many_arguments)]
    pub fn sweep_clearance(
        &self,
        way: &dyn Fn(f32) -> DVec3,
        speed: f32,
        wait: f32,
        accel: f32,
        v_max: f32,
        distance: f32,
        extent: (f32, f32, f32),
        obstacles: &[crate::collision::Obb],
    ) -> f64 {
        match self {
            AiMotion::Depot(b) => {
                b.sweep_clearance(way, speed, wait, accel, v_max, distance, extent, obstacles)
            }
            AiMotion::Classic(b) => sweep::sweep_clearance(
                b, way, speed, wait, accel, v_max, distance, extent, obstacles,
            ),
        }
    }
}
