//! The walking gait: how much of a stride a foot stands, the run factor and the foot pitches
//! through a step.

use omsi_geometry::util::smoothstep;

/// How far (m) the hips would have to sink to reach a planted foot before it steps up.
pub(super) const CATCH_UP: f32 = 0.13;

/// The most the hips sink to reach a planted foot (m).
pub(super) const MAX_DROP: f32 = 0.12;

/// A planted foot this far (m) above or below the floor the body stands on is on another
/// floor and steps over first.
pub(super) const STRANDED: f32 = 0.5;

/// Share of a gait cycle a foot is on the floor.
pub(super) fn stance_fraction(speed: f32) -> f32 {
    let walk = (0.64 - 0.05 * (speed - 0.8)).clamp(0.57, 0.7);
    // running: the foot is down for well under half the stride, with a flight between
    walk + (0.38 - walk) * run_factor(speed)
}

/// How much of a run the gait is (0 walking … 1 running), by the speed (m/s).
pub fn run_factor(speed: f32) -> f32 {
    smoothstep(2.0, 3.4, speed)
}

/// Foot pitch while planted during the walk (degrees, toes up positive): the heel strikes
/// with the toes up and rolls down, the foot lies flat, then the heel rises until the toes
/// push off.
pub(super) fn stance_pitch(u: f32, beta: f32, walk: f32) -> f32 {
    let heel = heel_pitch(walk) * (1.0 - smoothstep(0.0, 0.11, u));
    heel + toe_pitch(u, beta, walk)
}

/// Foot pitch at heel strike (degrees, toes up).
pub(super) fn heel_pitch(walk: f32) -> f32 {
    16.0 * walk.clamp(0.3, 1.0)
}

pub(super) fn toe_pitch(u: f32, beta: f32, walk: f32) -> f32 {
    let from = beta - 0.28;
    let t = ((u - from) / (beta - from)).clamp(0.0, 1.0);
    -48.0 * walk.clamp(0.25, 1.0) * t * t
}
