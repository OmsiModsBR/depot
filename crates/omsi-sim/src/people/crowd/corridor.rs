//! Keeping a walker inside a corridor and turning it towards where it goes.

use glam::DVec2;
use omsi_geometry::util::angle_diff;

/// [`clamp_to_corridor`] a little at a time: a walker outside the corridor is brought back
/// at up to 0.6 m/s. Clamped outright, a stroller jumped 5-6 cm in one frame wherever one
/// pavement path took over from the next (their lines do not meet exactly) - the "micro
/// teleports" - and whenever a corridor changed under somebody standing off its line.
pub fn ease_into_corridor(p: DVec2, a: DVec2, b: DVec2, max_dev: f64, dt: f64) -> DVec2 {
    let q = clamp_to_corridor(p, a, b, max_dev);
    let d = q - p;
    let step = 0.6 * dt.max(0.0) + 0.002;
    if d.length() <= step {
        q
    } else {
        p + d * (step / d.length())
    }
}

/// Turn `from` towards `to` (degrees) at most `rate` degrees per second, easing in at the
/// end so that a turn does not stop with a jolt.
pub fn turn_towards(from: f64, to: f64, rate: f64, dt: f64) -> f64 {
    let diff = angle_diff(from, to);
    // proportional near the target (time constant 0.25 s), capped at `rate`
    let step = (diff * (dt / 0.25).min(1.0)).clamp(-rate * dt, rate * dt);
    let out = from + step;
    out.rem_euclid(360.0)
}

/// Heading (degrees clockwise from north) of a ground direction.
pub fn heading_of(d: DVec2) -> f64 {
    d.x.atan2(d.y).to_degrees()
}

/// Nearest point to `p` on the segment `a`-`b` and the parameter along it (0..1).
pub fn project_on_segment(p: DVec2, a: DVec2, b: DVec2) -> (DVec2, f64) {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-9)).clamp(0.0, 1.0);
    (a + ab * t, t)
}

/// Keep `p` within `max_dev` of the segment `a`-`b` (a walker in a narrow aisle).
pub fn clamp_to_corridor(p: DVec2, a: DVec2, b: DVec2, max_dev: f64) -> DVec2 {
    let (q, _) = project_on_segment(p, a, b);
    let d = p - q;
    let len = d.length();
    if len > max_dev {
        q + d * (max_dev / len)
    } else {
        p
    }
}
