//! `D-3`: whether the tyres still hold across, axle by axle, with the force it takes to start or
//! stop the body turning, as the model document `docs/physics/depot-model.md` has it.

use crate::physics::domain::params::RELAXATION_LENGTH;

/// Whether the tyres hold the bend. The tyres ahead of the centre of gravity and those behind
/// it must together take the bend's pull `across` (N, to the body's right) and turn the body
/// by `yaw_moment` (N m, about its up axis): with the two groups at their grip-weighted levers
/// `y_f` and `y_r` (m forward of the centre), `F_f + F_r = across` and
/// `y_f F_f + y_r F_r = -yaw_moment`. Each group holds while its force is within its grip.
/// `tyres` are each tyre's lever and grip (N); a body with tyres on one side only is held by
/// all of them together, as Classic holds every body.
pub(in crate::physics) fn holds_across(
    tyres: impl Iterator<Item = (f32, f32)>,
    across: f32,
    yaw_moment: f32,
) -> bool {
    let (mut ahead, mut behind) = ((0.0f32, 0.0f32), (0.0f32, 0.0f32));
    for (lever, grip) in tyres {
        let group = if lever >= 0.0 {
            &mut ahead
        } else {
            &mut behind
        };
        group.0 += grip;
        group.1 += grip * lever;
    }
    if ahead.0 <= 0.0 || behind.0 <= 0.0 {
        return across.abs() <= ahead.0 + behind.0;
    }
    let (y_f, y_r) = (ahead.1 / ahead.0, behind.1 / behind.0);
    let front = (-yaw_moment - y_r * across) / (y_f - y_r);
    let rear = across - front;
    front.abs() <= ahead.0 && rear.abs() <= behind.0
}

/// The yaw acceleration (rad/s²) the tyres are asked for after a substep of `h` s, from the
/// last one `before`, the body's yaw rate now and a substep ago and its forward speed (m/s):
/// the change of the yaw rate, followed with the lag of the tyres' relaxation (the time they
/// take to roll `RELAXATION_LENGTH`, at least as at 1 m/s).
pub(in crate::physics) fn yaw_accel(
    before: f32,
    rate: f32,
    last_rate: f32,
    speed: f32,
    h: f32,
) -> f32 {
    if h <= 0.0 {
        return before;
    }
    let raw = (rate - last_rate) / h;
    let lag = RELAXATION_LENGTH / speed.abs().max(1.0);
    before + (raw - before) * (h / (lag + h))
}
