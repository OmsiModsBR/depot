//! How many vehicles and walkers the options and the map ask for.
//!
//! Vehicles without a timetable are not counted towards a target: every street lane brings
//! them on by itself, at intervals set by its own rate, and the only global limit is the
//! options' cap. What the rate is made of, and the first fill, are written down in
//! `docs/parity/traffic-people-density.md`.

/// The least rate a lane that carries traffic at all is given.
pub const MIN_RATE: f32 = 0.02;

/// Cars per metre of lane at a rate of 1 (one in 250 m), on average, in the first fill.
///
/// A calibration, not a measured value: the absolute scale is still to be set by the
/// parity registry's scenario A measurement.
pub const CARS_PER_METRE: f32 = 0.004;

/// Mean seconds between two arrivals on a lane of `rate` x `length` = 1: the arrival
/// interval of a 100 m lane at rate 1 is 25 s. Calibrated together with [`CARS_PER_METRE`].
pub const SECONDS_PER_RATE_METRE: f32 = 2500.0;

/// The rate of one lane: the lane's configured density for the group, times the day curve
/// (as it is, unclamped), times the road traffic factor, divided between the `lanes` of
/// the road. 0 stays 0; anything above 0 is at least [`MIN_RATE`].
pub fn lane_rate(density: f32, curve: f32, factor: f32, lanes: usize) -> f32 {
    let r = density * curve * factor / lanes.max(1) as f32;
    if r <= 0.0 || r.is_nan() {
        0.0
    } else {
        r.max(MIN_RATE)
    }
}

/// Mean seconds between arrivals on a lane (`None`: never).
pub fn mean_interval(rate: f32, length: f32) -> Option<f32> {
    (rate > 0.0 && length > 0.0).then(|| SECONDS_PER_RATE_METRE / (rate * length))
}

/// A random wait to the next arrival, `u` in 0..1: from half the mean to one and a half.
pub fn next_interval(rate: f32, length: f32, u: f32) -> Option<f32> {
    mean_interval(rate, length).map(|m| m * (0.5 + u.clamp(0.0, 1.0)))
}

/// How many vehicles a lane starts with when the scenario loads: proportional to its rate
/// and length, varied by `u_var` (0..1, from half to one and a half) and rounded up or
/// down by `u_round`, so that the mean stays proportional for short lanes too.
pub fn initial_count(rate: f32, length: f32, u_var: f32, u_round: f32) -> usize {
    if rate <= 0.0 || length <= 0.0 {
        return 0;
    }
    let want = rate * length * CARS_PER_METRE * (0.5 + u_var.clamp(0.0, 1.0));
    (want + u_round.clamp(0.0, 0.999)).floor() as usize
}

/// One lane's wait to its next arrival. A step is the real time of the frame (0 in the
/// pause), and a late clock fires once, not once for each interval it missed.
#[derive(Debug, Clone, Copy)]
pub struct SpawnClock {
    remaining: f32,
}

impl SpawnClock {
    pub fn new(wait: f32) -> SpawnClock {
        SpawnClock { remaining: wait }
    }

    /// Run the clock down by `dt`; true when it has run out.
    pub fn step(&mut self, dt: f32) -> bool {
        self.remaining -= dt.max(0.0);
        self.remaining <= 0.0
    }

    /// Start a new wait, whatever was left over.
    pub fn restart(&mut self, wait: f32) {
        self.remaining = wait;
    }
}

/// How many more vehicles without a timetable the cap lets on: the cap counts all of
/// them in circulation, wherever they are.
pub fn spawn_room(cap: usize, in_circulation: usize) -> usize {
    cap.saturating_sub(in_circulation)
}

/// How many people walk the pavements: the base number times the extent of the
/// pavements nearby (`crowd`) times the people curve for the hour, no more than half the
/// pool. The passengers factor does not enter.
pub fn walker_target(base: usize, crowd: f32, curve: f32, pool: usize) -> usize {
    let want = (base as f32 * crowd * curve.max(0.0)).round() as usize;
    want.min(pool / 2)
}
