//! What the traffic planner asks of an AI body before it moves: the S-curves that take it
//! out round something standing ahead and back into its lane, and how close it would come
//! to what stands there (`AiBody::sweep_clearance`).

use super::{AiBody, MotionKind};
use glam::{DVec2, DVec3};
use omsi_vehicle::Vehicle;

/// Clearance a vehicle pulling out keeps from the corner of what it goes round (m).
pub const PULL_OUT_CLEARANCE: f64 = 0.25;
/// Seconds a driver standing behind something turns the wheels before moving off round it
/// (the shortest reaction time of the AI drivers).
pub const PULL_OUT_WAIT: f32 = 0.4;
/// Acceleration (m/s²) of a driver edging out round something standing close ahead, until
/// the front is past its corner: pulling away briskly, a car covered too much ground while
/// the wheels were still turning and came within 0.2 m of the corner.
pub const PULL_OUT_ACCEL: f32 = 1.0;

/// Lengths (m) of the S-curve that takes a vehicle out round something standing `real`
/// metres ahead of its front bumper, gentlest first (`front`: origin to front bumper).
/// From a standstill a driver turns out steeply; rolling up to a parked car they move over
/// well before it.
pub fn pull_out_ramps(real: f32, front: f32, rolling: bool) -> Vec<f32> {
    let base = real + 0.5 * front;
    let (factors, max): (&[f32], f32) = if rolling { (&[1.0, 0.8, 0.6, 0.45], 20.0) } else { (&[1.5, 1.2, 1.0, 0.8, 0.6], 12.0) };
    let mut out: Vec<f32> = Vec::new();
    for f in factors {
        let r = (base * f).clamp(4.0, max);
        if !out.iter().any(|&o| (o - r).abs() < 0.3) {
            out.push(r);
        }
    }
    out
}

/// Sideways acceleration (m/s²) a driver allows moving back into the lane after passing.
pub const BACK_IN_LAT_ACCEL: f32 = 2.5;

/// Length (m) of the S-curve that takes a vehicle `offset` metres back into its lane at
/// `speed` (m/s) after passing: a smoothstep of length L over the offset D turns at most
/// 6·D·v²/L², so L = v·√(6·D/a) keeps it to `lat_accel` - about 2.8 s of travel for a lane
/// width. A fixed 8-14 m cut back 3.3 m in under a second at 40 km/h (4-5.6 m/s²).
pub fn back_in_ramp(offset: f32, speed: f32, lat_accel: f32) -> f32 {
    let geometric = (offset.abs() * 3.5).clamp(8.0, 14.0);
    let dynamic = speed.max(0.0) * (6.0 * offset.abs() / lat_accel.max(0.5)).sqrt();
    geometric.max(dynamic).min(60.0)
}

/// A straight way north from the origin that moves `side` metres to the left along an
/// S-curve (smoothstep) `ramp` metres long, as `AiState::way_point` lays out a pull-out.
pub fn straight_pull_out(side: f32, ramp: f32) -> impl Fn(f32) -> DVec3 {
    move |d: f32| {
        let t = (d / ramp.max(0.1)).clamp(0.0, 1.0);
        let lat = -side * t * t * (3.0 - 2.0 * t);
        DVec3::new(lat as f64, d as f64, 0.0)
    }
}

/// How far behind something standing in its lane a vehicle has to stop to get out round
/// it later from a standstill (m, from its front bumper to the other's body), with its own
/// wheelbase, steering lock and steering rate: the shortest gap from which one of the
/// `pull_out_ramps` takes it past the corner of a standing bus (`obstacle_half_width`,
/// in the middle of the lane, the oncoming lane `side` metres over) with
/// `PULL_OUT_CLEARANCE` to spare, and half a metre more for where it comes to rest.
/// `extent` is (origin to front bumper, to rear bumper, half width).
pub fn pull_out_room(def: &Vehicle, extent: (f32, f32, f32), obstacle_half_width: f64, side: f32) -> f32 {
    let (front, _, _) = extent;
    let bus_half_len = 6.0;
    let mut gap = 0.75f32;
    while gap <= 12.0 {
        let obstacle = crate::collision::Obb::vehicle(DVec2::new(0.0, (front + gap) as f64 + bus_half_len), 0.0, bus_half_len, bus_half_len, obstacle_half_width);
        for ramp in pull_out_ramps(gap, front, false) {
            let way = straight_pull_out(side, ramp);
            let mut body = AiBody::new(def, MotionKind::Road);
            body.place(&way, None, None, 0.0);
            if body.sweep_clearance(&way, 0.0, PULL_OUT_WAIT, PULL_OUT_ACCEL, 8.0, gap + 6.0, extent, &[obstacle]) >= PULL_OUT_CLEARANCE {
                return gap + 0.5;
            }
        }
        gap += 0.25;
    }
    12.5
}

impl AiBody {
    /// Drive a copy of this body along a way it has not taken yet and report how close it
    /// comes to `obstacles`: the smallest plan-view separation (m, negative = the bodies
    /// would touch) between the vehicle's box (`front`/`rear` from its origin, `half_width`)
    /// and the nearest of them while the vehicle covers `distance` metres. `way(d)` is the point `d`
    /// metres along the new way from where the vehicle is now (the same way `step` is given,
    /// e.g. with a pull-out round something standing ahead in it); the vehicle starts at
    /// `speed` (a standing one first turns its wheels for `wait` seconds, as a driver does
    /// before moving off) and speeds up at `accel` to at most `v_max`. The steering is the real one -
    /// its rate limit, its lock and the pure pursuit that cuts in on a steep S-bend - which
    /// is what decides whether a car standing a few metres behind a bus can get its front
    /// corner past the bus's.
    #[allow(clippy::too_many_arguments)]
    pub fn sweep_clearance(&self, way: &dyn Fn(f32) -> DVec3, speed: f32, wait: f32, accel: f32, v_max: f32, distance: f32, extent: (f32, f32, f32), obstacles: &[crate::collision::Obb]) -> f64 {
        let mut body = self.clone();
        let (front, rear, half_width) = extent;
        let dt = 1.0 / 20.0;
        let mut v = speed.max(0.0);
        let mut x = 0.0f32;
        let gap = |b: &AiBody| {
            let me = crate::collision::Obb::vehicle(b.position.truncate(), b.heading, front as f64, rear as f64, half_width as f64);
            obstacles.iter().map(|o| me.separation(o)).fold(f64::MAX, f64::min)
        };
        let mut best = gap(&body);
        if v < 0.05 {
            for _ in 0..(wait / dt).round() as usize {
                body.drive(dt, 0.0, way);
            }
        }
        // (at most a minute of simulated time: a car that cannot move is not getting past)
        for _ in 0..1200 {
            if x >= distance {
                break;
            }
            v = (v + accel.max(0.3) * dt).min(v_max.max(0.5));
            let ds = v * dt;
            x += ds;
            let at = x;
            body.drive(dt, v, &|d| way(at + d));
            best = best.min(gap(&body));
        }
        best
    }
}
