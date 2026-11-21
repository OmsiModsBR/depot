//! Steerable axles (spec 004): `D-5` to `D-7` of `docs/physics/depot-model.md`. The pure rules
//! first, then what they come to on a body.

use crate::physics::domain::params::{SCRUB_TOLERANCE, SUBSTEP_DT};
use crate::physics::domain::{RigidBody, SteerAxle, SteerMode, SteerState};
use omsi_vehicle::Vehicle;

/// Closer than this (m) to the turning line, an axle stands on it.
const ON_LINE: f32 = 1e-3;
/// The shortest arm (m) from the turning line to the front axle the full lock is worked out
/// with: a line on the front axle would ask an endless curvature.
const MIN_ARM: f32 = 0.5;

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

/// The turning line (m along the body, `D-5`) of axles at `(y, load, weight)`: the mean of
/// their places weighted by `load · weight` (1 for a rigid axle or one the script holds,
/// `1 − e` for a steerable one); `fallback`, the `C-6` line, when nothing weighs.
pub fn turning_line(axles: &[(f32, f32, f32)], fallback: f32) -> f32 {
    let (sum_w, sum_wy) = axles.iter().fold((0.0, 0.0), |(w, wy), &(y, load, r)| {
        (w + load * r, wy + load * r * y)
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
/// `centring` per second, its engagement to 1 (basic mode) or 0 at most `centring / lock` per
/// second.
pub fn advance(axle: &SteerAxle, state: SteerState, sigma: f32, s: f32, h: f32) -> SteerState {
    let towards = |from: f32, to: f32, step: f32| from + (to - from).clamp(-step, step);
    let engaged = if state.mode == SteerMode::Basic {
        1.0
    } else {
        0.0
    };
    SteerState {
        angle: towards(
            state.angle,
            target_angle(axle, &state, sigma, s),
            axle.centring * h,
        ),
        engagement: towards(state.engagement, engaged, axle.centring / axle.lock * h),
        ..state
    }
}

/// How much an axle weighs in the turning line besides its load (`D-5`): 1 rigid or held by
/// the script, `1 − e` steering.
fn weight(state: Option<&SteerState>) -> f32 {
    match state {
        None => 1.0,
        Some(s) if s.mode == SteerMode::ScriptAngle => 1.0,
        Some(s) => 1.0 - s.engagement,
    }
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

    /// The section's turning line now (`D-5`), the leading front axle left out.
    fn depot_turning_line(&self) -> f32 {
        let front = self.front_axle_y();
        let axles: Vec<(f32, f32, f32)> = self
            .wheels
            .iter()
            .zip(&self.wheel_axle)
            .step_by(2)
            .filter(|(w, _)| w.attach.y != front)
            .map(|(w, &a)| (w.attach.y, w.rest_load, weight(self.steer_state(a))))
            .collect();
        turning_line(&axles, self.file_turn_line)
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
        w.scrubs = (w.steer - ackermann(kappa)).abs() > SCRUB_TOLERANCE;
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
