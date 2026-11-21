//! Steerable axles (spec 004): `D-5` to `D-7` of `docs/physics/depot-model.md`. The pure rules
//! first, then what they come to on a body.

use crate::physics::domain::params::{SCRUB_FULL_ANGLE, SUBSTEP_DT};
use crate::physics::domain::{RigidBody, SteerAxle, SteerMode, SteerState};
use omsi_vehicle::Vehicle;

/// Closer than this (m) to the turning line, an axle stands on it.
const ON_LINE: f32 = 1e-3;
/// The shortest arm (m) from the turning line to the front axle the full lock is worked out
/// with: a line on the front axle would ask an endless curvature.
const MIN_ARM: f32 = 0.5;
/// The least curvature (1/m) an axle's angle is set against to find where it points: straight
/// on, a steered axle would point at an endless distance (and the line then hardly matters).
const MIN_KAPPA: f32 = 1e-3;

/// The section's steerable axles, from the `achse_steer_*` parameters of its `[newachse]`
/// (`D-5`).
pub(in crate::physics) fn steer_axles(def: &Vehicle) -> Vec<SteerAxle> {
    def.axles
        .iter()
        .enumerate()
        .filter_map(|(axle, a)| {
            a.steer.map(|s| SteerAxle {
                axle,
                lock: s.lock_deg.to_radians(),
                centring: s.centring_deg_s.to_radians(),
            })
        })
        .collect()
}

/// The turning line (m along the body, `D-5`) of axles at `(y, load, angle)` on curvature
/// `kappa`: where each axle points on the body's centre line, `y − tan(angle) / κ` (its own
/// place for a straight axle), weighted by its load, so that the tyres' slip balances;
/// `fallback`, the `C-6` line, when nothing weighs.
pub fn turning_line(axles: &[(f32, f32, f32)], kappa: f32, fallback: f32) -> f32 {
    let k = if kappa.abs() < MIN_KAPPA {
        MIN_KAPPA.copysign(kappa)
    } else {
        kappa
    };
    let (sum_w, sum_wy) = axles.iter().fold((0.0, 0.0), |(w, wy), &(y, load, angle)| {
        (w + load, wy + load * (y - angle.tan() / k))
    });
    if sum_w > 0.0 {
        sum_wy / sum_w
    } else {
        fallback
    }
}

/// The curvature of the full lock (1/m, `D-5`): the front wheels at `lock_front` (rad) about
/// the turning line `y_turn`.
pub fn full_lock(lock_front: f32, y_front: f32, y_turn: f32) -> f32 {
    lock_front.tan() / (y_front - y_turn).abs().max(MIN_ARM)
}

/// Which way an axle at `y` turns in the basic mode (`D-6`): −1 behind the turning line (the
/// other way, a tag axle), +1 ahead of it (the same way, a twin-steer axle), 0 on it.
pub fn side(y: f32, y_turn: f32) -> f32 {
    if (y - y_turn).abs() < ON_LINE {
        0.0
    } else {
        (y - y_turn).signum()
    }
}

/// The angle (rad) a steerable axle turns to (`D-6`): in the basic mode `σ · s · lock`, with `s`
/// the steering's share of the full lock (−1..1); straight when locked; the script's angle,
/// clamped to the lock.
pub fn target_angle(axle: &SteerAxle, state: &SteerState, sigma: f32, s: f32) -> f32 {
    match state.mode {
        SteerMode::Basic => sigma * s.clamp(-1.0, 1.0) * axle.lock,
        SteerMode::Locked => 0.0,
        SteerMode::ScriptAngle => state.script_angle.clamp(-axle.lock, axle.lock),
    }
}

/// The axle's state a time `h` (s) on (`D-6`): its angle moves to its target at most
/// `centring` per second.
pub fn advance(axle: &SteerAxle, state: SteerState, sigma: f32, s: f32, h: f32) -> SteerState {
    let step = axle.centring * h;
    let to = target_angle(axle, &state, sigma, s);
    SteerState {
        angle: state.angle + (to - state.angle).clamp(-step, step),
        ..state
    }
}

/// The share of the road's grip (0..1) a tyre that points `off` (rad) off the centre of the
/// turn takes across while the tyres hold (`D-7`): growing with the angle, whole from
/// `SCRUB_FULL_ANGLE` on.
pub fn scrub_reach(off: f32) -> f32 {
    (off.abs() / SCRUB_FULL_ANGLE).min(1.0)
}

impl RigidBody {
    /// The frontmost axle's place (m along the body).
    fn front_axle_y(&self) -> f32 {
        self.wheels
            .iter()
            .map(|w| w.attach.y)
            .fold(f32::MIN, f32::max)
    }

    /// The steerable axle `axle`'s state, if it is one.
    fn steer_state(&self, axle: usize) -> Option<&SteerState> {
        self.steer_axles
            .iter()
            .find(|(a, _)| a.axle == axle)
            .map(|(_, s)| s)
    }

    /// The section's turning line now (`D-5`), the leading front axle left out, on the
    /// curvature and the axles' angles of the substep before: between its rearmost axle and
    /// `MIN_ARM` behind the front one; the `C-6` line when every other axle steers.
    fn depot_turning_line(&self) -> f32 {
        let front = self.front_axle_y();
        let axles: Vec<(f32, f32, f32)> = self
            .wheels
            .iter()
            .zip(&self.wheel_axle)
            .step_by(2)
            .filter(|(w, _)| w.attach.y != front)
            .map(|(w, &a)| {
                let angle = self.steer_state(a).map_or(0.0, |s| s.angle);
                (w.attach.y, w.rest_load, angle)
            })
            .collect();
        if axles.len() == self.steer_axles.len() {
            return self.file_turn_line;
        }
        let rearmost = axles.iter().map(|a| a.0).fold(f32::MAX, f32::min);
        turning_line(&axles, self.kappa, self.file_turn_line)
            .min(front - MIN_ARM)
            .max(rearmost)
    }
}

/// The full-lock curvature (1/m) of a leading section with steerable axles (`D-5`).
pub(in crate::physics) fn body_full_lock(body: &RigidBody) -> f32 {
    full_lock(
        body.max_steer_deg.to_radians(),
        body.front_axle_y(),
        body.depot_turning_line(),
    )
}

/// The start of a substep for a leading section with steerable axles on curvature `kappa`
/// (`D-5` to `D-7`): its turning line, each steerable axle a substep on in its mode, the tyres
/// of the front and steerable axles square to the line from the centre of their turn
/// (Ackermann), the rigid axles straight, and which tyres scrub.
pub(in crate::physics) fn steer_to(body: &mut RigidBody, kappa: f32) {
    let y_turn = body.depot_turning_line();
    let s = kappa / body_full_lock(body);
    let front = body.front_axle_y();
    body.kappa = kappa;
    body.rot_pnt_long = y_turn;
    let longs: Vec<f32> = body
        .steer_axles
        .iter()
        .map(|(a, _)| body.wheels[2 * a.axle].attach.y)
        .collect();
    for ((axle, state), y) in body.steer_axles.iter_mut().zip(longs) {
        *state = advance(axle, *state, side(y, y_turn), s, SUBSTEP_DT);
    }
    let angles: Vec<Option<f32>> = body
        .wheel_axle
        .iter()
        .map(|&a| body.steer_state(a).map(|s| s.angle))
        .collect();
    for (w, angle) in body.wheels.iter_mut().zip(angles) {
        let arm = w.attach.y - y_turn;
        // the tyre square to the line from the centre of a turn of curvature `k` to itself
        let ackermann = |k: f32| (arm * k / (1.0 - w.attach.x * k).max(0.2)).atan();
        w.steer = if w.attach.y == front {
            ackermann(kappa)
        } else {
            match angle {
                // the axle's own turn: the centre its angle points at on the turning line
                Some(d) if arm.abs() >= ON_LINE => ackermann(d.tan() / arm),
                Some(d) => d,
                None => 0.0,
            }
        };
        w.steered = w.steer != 0.0;
        w.hold_reach = if w.attach.y == front {
            f32::INFINITY
        } else {
            scrub_reach(w.steer - ackermann(kappa))
        };
    }
    body.steer_deg = ((front - y_turn) * kappa).atan().to_degrees();
}

/// The angle (rad) of wheel `i`'s axle in a leading section with steerable axles, as the
/// scripts read it (`D-6`): the front axle's as `C-6`, a steerable axle's own, 0 for a rigid
/// one.
pub(in crate::physics) fn axle_angle(body: &RigidBody, i: usize) -> f32 {
    let Some(w) = body.wheels.get(i) else {
        return 0.0;
    };
    if w.attach.y == body.front_axle_y() {
        return ((w.attach.y - body.rot_pnt_long) * body.kappa).atan();
    }
    body.steer_state(body.wheel_axle[i])
        .map_or(0.0, |s| s.angle)
}
