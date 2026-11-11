//! How the body of an AI vehicle follows the way its planner lays out
//! (`traffic::AiState::way_point`), by engine (spec 002): [`AiMotion`] holds the body of the
//! engine a vehicle runs. `depot` is Project Depot Physics' body (`DA-1` to `DA-6`): a road
//! vehicle is a bicycle model steered by pure pursuit and sitting on its springs, a rail
//! vehicle rides on its two outer axles, an aircraft flies along its path and banks into its
//! turns. The Classic AI (`KA-*`) is built in phase 8: until then both engines run `depot`.
//!
//! Placing the vehicle *on* the lane every frame, turned to the lane's heading, is what made
//! the traffic look wrong: the heading jumped at every lane sample and joint (the steering
//! was worked out from those jumps), the body turned about its middle so that the rear
//! wheels slid sideways through every bend, and a lane change was a sideways glide with a
//! made-up yaw. In `depot` the heading only ever changes by speed × tan(steering) / wheelbase,
//! the rear axle never moves sideways, and the steering itself is rate limited.

mod depot;

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
    /// Project Depot Physics' body (`DA-1` to `DA-6`).
    Depot(AiBody),
}

impl AiMotion {
    /// The body of a vehicle `def` of `mode`. Both modes get the Depot body until the Classic
    /// AI is built (phase 8 of spec 002).
    pub fn new(def: &Vehicle, kind: MotionKind, mode: PhysicsMode) -> AiMotion {
        match mode {
            PhysicsMode::Classic | PhysicsMode::Depot => AiMotion::Depot(AiBody::new(def, kind)),
        }
    }

    fn body(&self) -> &AiBody {
        match self {
            AiMotion::Depot(b) => b,
        }
    }

    fn body_mut(&mut self) -> &mut AiBody {
        match self {
            AiMotion::Depot(b) => b,
        }
    }

    /// What kind of way the vehicle follows.
    pub fn kind(&self) -> MotionKind {
        self.body().kind
    }

    /// The front wheel angle (deg, + = right).
    pub fn steer(&self) -> f32 {
        self.body().steer
    }

    /// The lateral acceleration (m/s², right positive).
    pub fn a_lat(&self) -> f32 {
        self.body().a_lat
    }

    /// See [`AiBody::place`].
    pub fn place(
        &mut self,
        way: &dyn Fn(f32) -> DVec3,
        ground: Option<&dyn Fn(f64, f64) -> Option<f64>>,
        contact: Option<&dyn crate::physics::Ground>,
        speed: f32,
    ) {
        self.body_mut().place(way, ground, contact, speed)
    }

    /// See [`AiBody::step`].
    pub fn step(
        &mut self,
        dt: f32,
        speed: f32,
        way: &dyn Fn(f32) -> DVec3,
        ground: Option<&dyn Fn(f64, f64) -> Option<f64>>,
        contact: Option<&dyn crate::physics::Ground>,
    ) {
        self.body_mut().step(dt, speed, way, ground, contact)
    }

    /// See [`AiBody::apply`].
    pub fn apply(&self, v: &mut VehicleInstance) {
        self.body().apply(v)
    }

    /// See [`AiBody::sweep_clearance`].
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
        self.body()
            .sweep_clearance(way, speed, wait, accel, v_max, distance, extent, obstacles)
    }
}
