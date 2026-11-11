//! How close a body would come to what stands about it on a way it has not taken yet: the
//! traffic planner's question before a vehicle pulls out round something (shared by both
//! engines' bodies).

use glam::{DVec2, DVec3};

/// A body the planner can drive ahead on a copy.
pub(crate) trait Sweep: Clone {
    /// Follow `way` for `dt` seconds at `speed`.
    fn advance(&mut self, dt: f32, speed: f32, way: &dyn Fn(f32) -> DVec3);
    /// The origin's place in plan and the heading (deg).
    fn footprint(&self) -> (DVec2, f64);
}

/// Drive a copy of `body` along a way it has not taken yet and report how close it comes to
/// `obstacles`: the smallest plan-view separation (m, negative = the bodies would touch)
/// between the vehicle's box (`extent`: front and rear from its origin, half width) and the
/// nearest of them while the vehicle covers `distance` metres. `way(d)` is the point `d` metres
/// along the new way from where the vehicle is now; the vehicle starts at `speed` (a standing
/// one first turns its wheels for `wait` seconds, as a driver does before moving off) and
/// speeds up at `accel` to at most `v_max`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn sweep_clearance<B: Sweep>(
    body: &B,
    way: &dyn Fn(f32) -> DVec3,
    speed: f32,
    wait: f32,
    accel: f32,
    v_max: f32,
    distance: f32,
    extent: (f32, f32, f32),
    obstacles: &[crate::collision::Obb],
) -> f64 {
    let mut body = body.clone();
    let (front, rear, half_width) = extent;
    let dt = 1.0 / 20.0;
    let mut v = speed.max(0.0);
    let mut x = 0.0f32;
    let gap = |b: &B| {
        let (at, heading) = b.footprint();
        let me = crate::collision::Obb::vehicle(
            at,
            heading,
            front as f64,
            rear as f64,
            half_width as f64,
        );
        obstacles
            .iter()
            .map(|o| me.separation(o))
            .fold(f64::MAX, f64::min)
    };
    let mut best = gap(&body);
    if v < 0.05 {
        for _ in 0..(wait / dt).round() as usize {
            body.advance(dt, 0.0, way);
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
        body.advance(dt, v, &|d| way(at + d));
        best = best.min(gap(&body));
    }
    best
}
