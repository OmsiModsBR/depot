//! Synthetic grounds, as a probe `(x, y, top)` reads them.

use omsi_sim::physics::GroundProbe;

/// The ground at height `z` for a probe reaching up to `top`: a face too high is above,
/// with the road at 0 still below it (a kerb, a platform).
pub fn step_at(z: f64, top: f64) -> GroundProbe {
    if z <= top {
        GroundProbe {
            below: Some(z),
            above: None,
        }
    } else {
        GroundProbe {
            below: Some(0.0),
            above: Some(z),
        }
    }
}

/// The road at 0 everywhere.
pub fn flat(_x: f64, _y: f64, _top: f64) -> GroundProbe {
    GroundProbe {
        below: Some(0.0),
        above: None,
    }
}

/// The road at 0 with a step of `step` metres from y = `at` on.
pub fn road(at: f64, step: f64) -> impl Fn(f64, f64, f64) -> GroundProbe {
    move |_x, y, top| step_at(if y >= at { step } else { 0.0 }, top)
}
