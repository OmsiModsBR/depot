//! The Classic AI's speed (`KA-2`, `KA-3`, `KA-9`, `KA-10`): within the detail radius about the
//! camera, while serving a stop, or for a model with `[noDistanceCheck]`, the leading section
//! integrates its speed from the scripts' drive torque and brakes, the AI driver setting the
//! throttle and brake the scripts read; beyond it the speed is the driver's target speed.

/// How far ahead (s) the shared traffic planner's acceleration is carried to make the driver's
/// target speed (the planner decides an acceleration, the Classic driver tracks a speed).
pub const TARGET_HORIZON: f32 = 1.0;

/// The brake a driver holds a standing vehicle on when it is asked to stop.
const STOP_BRAKE: f32 = 1.0;

/// Standard gravity (m/s²), the bound of a frame's speed change (`KA-3`).
const G: f32 = 9.81;

/// The driver's target speed (m/s) from the planner's speed and acceleration.
pub fn target_speed(speed: f32, acc: f32) -> f32 {
    (speed + acc * TARGET_HORIZON).max(0.0)
}

/// What the scripts left for the speed (`KA-2`): the drive force, the forces that resist the
/// motion (rolling resistance and every wheel's brakes) and the mass of all sections.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DriveForces {
    /// N, + = forward.
    pub drive: f32,
    /// N, never negative.
    pub resist: f32,
    pub mass_kg: f32,
}

/// The throttle and brake the AI driver hands the scripts (0..1).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Pedals {
    pub throttle: f32,
    pub brake: f32,
}

impl Pedals {
    /// The driver's pedals for the ratio of the speed to the target speed `r = v / target`:
    /// the brake from 5 % above the target, fully on at twice it; the throttle, when the
    /// brake is off, towards `10·(1 − r)`, rising by no more than 1 per second.
    pub fn update(&mut self, speed: f32, target: f32, dt: f32) {
        if target <= 1e-3 {
            // asked to stop: off the throttle, on the brake
            self.throttle = 0.0;
            self.brake = STOP_BRAKE;
            return;
        }
        let r = speed.max(0.0) / target;
        self.brake = if r > 2.0 {
            1.0
        } else {
            (2.0 * (r - 1.05)).clamp(0.0, 0.7)
        };
        self.throttle = if self.brake < 1e-3 {
            (self.throttle + dt).min((10.0 * (1.0 - r)).clamp(0.0, 1.0))
        } else {
            0.0
        };
    }
}

/// One frame of the speed from the forces (`KA-2`): standing, the resistance only holds the
/// vehicle against the drive; rolling, it acts against the motion. A frame's change is bounded
/// by `g·μ` (per frame, `KA-3`), and the resistance never turns the vehicle round.
pub fn integrate(speed: f32, f: DriveForces, mu: f32, dt: f32) -> f32 {
    let resist = f.resist.max(0.0);
    let net = if speed == 0.0 {
        let n = f.drive - f.drive.signum() * resist;
        if n.signum() != f.drive.signum() {
            0.0
        } else {
            n
        }
    } else {
        f.drive - speed.signum() * resist
    };
    let bound = G * mu.max(0.0);
    let dv = (dt * net / f.mass_kg.max(1.0)).clamp(-bound, bound);
    let v = speed + dv;
    if speed != 0.0 && v.signum() != speed.signum() {
        0.0
    } else {
        v
    }
}

/// The speed regime: physical within the detail radius (`KA-2`), while serving a stop
/// (`KA-10`) or with `[noDistanceCheck]`; the target speed beyond it (`KA-9`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Regime {
    Physical,
    Target,
}

impl Regime {
    pub fn of(near: bool, serving_stop: bool) -> Regime {
        if near || serving_stop {
            Regime::Physical
        } else {
            Regime::Target
        }
    }
}

/// Whether a tile is within the detail radius: the camera's tile and its eight neighbours.
pub fn within_detail_radius(tile: (i32, i32), camera_tile: (i32, i32)) -> bool {
    (tile.0 - camera_tile.0).abs() <= 1 && (tile.1 - camera_tile.1).abs() <= 1
}
