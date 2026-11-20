//! `C-5`: the grip of a tyre on the ground under it - the kind of surface its texture names,
//! the water on it and the weather - as OMSI works it out for each wheel.

use crate::physics::domain::RoadWeather;

/// How wet a road must be before it counts as wet, or can freeze.
const WET: f32 = 0.1;
/// How full a puddle must be before it costs a road its grip.
const PUDDLE: f32 = 0.2;

/// A straight line through (`x0`, `y0`) and (`x1`, `y1`), at `x` held between `x0` and `x1`.
fn ramp(x: f32, (x0, y0): (f32, f32), (x1, y1): (f32, f32)) -> f32 {
    let x = x.clamp(x0.min(x1), x0.max(x1));
    y0 + (x - x0) * (y1 - y0) / (x1 - x0)
}

/// The friction coefficient of a tyre on ground `ground` (the byte of the face it stands on:
/// its texture's `[surface]` kind in the low five bits, `0x20` where it gets wet, `0x40`
/// where it collects puddles; see `RigidWheel::surface`) in the weather `road`. Ice forms on
/// a wet road (wet past 0.1) in snow below 0 °C, and without snow below -5 °C; it takes the
/// kinds 0 to 2, 15 and 16 (and 9 always).
pub fn surface_grip(ground: u8, road: &RoadWeather) -> f32 {
    let kind = ground & 0x1f;
    let wet = if ground & 0x20 != 0 { road.wet } else { 0.0 };
    let puddle = if ground & 0x40 != 0 {
        road.puddles
    } else {
        0.0
    };
    let t = road.temperature;
    let icy = (road.snow && wet > WET && t < 0.0) || (wet > WET && t < -5.0);
    let paved = kind <= 2 || kind >= 15;
    match kind {
        6 | 7 if road.snow => 0.15,
        6 | 7 => ramp(t, (-5.0, 0.3), (5.0, 0.2)),
        8 if road.snow => 0.15,
        8 => ramp(t, (-5.0, 0.2), (5.0, 0.26)),
        _ if ground == 9 || (icy && paved) => ramp(t, (-10.0, 0.2), (0.0, 0.15)),
        2 | 15 if wet > WET => 0.4,
        _ if ground == 10 || (puddle > PUDDLE && (kind < 2 || kind == 16)) => 0.65,
        _ if wet > WET && (kind < 2 || kind == 16) => 0.73,
        2 | 15 => 0.7,
        1 | 16 => 0.85,
        0 => 0.78,
        5 => 0.65,
        _ => 0.5,
    }
}
