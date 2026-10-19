//! The AI driver: where an AI vehicle is along its lanes, which way it goes and how fast
//! (the decision; the body that follows the way is `ai::AiBody`).

mod drive;
mod lane_change;
mod plan;
mod speed;

pub use lane_change::LaneChange;
pub use speed::{arrival_time, ramp_progress_for, MAX_BRAKE};

/// How far ahead (m) a car decides which way it goes: far enough to signal a turn in good
/// time and for the steering to look beyond the junction.
const PLAN_AHEAD: f32 = 90.0;

/// At most this many lanes are planned ahead (junction pieces can be a few metres long).
const PLAN_LANES: usize = 10;

/// Linked lanes may end up to 1.5 m apart; over this many metres on either side of a joint
/// the way is bent so that it has no step (half the gap on each side).
const JOINT_BLEND: f32 = 6.0;

/// Seconds of signalling before a lane change begins to move sideways.
const SIGNAL_BEFORE_CHANGE: f32 = 1.2;

/// One AI vehicle moving on the network: where it is along its lanes and which way it is
/// going. The body that follows this way is `ai::AiBody`.
#[derive(Debug, Clone)]
pub struct AiState {
    pub traffic_pool: Option<(usize, std::sync::Arc<Vec<i32>>)>,
    /// The vehicle's `[ai_veh_type]` (0 car, 1 taxi, 2 bus, 3 truck; -1 a timetable bus):
    /// which lanes it may take, see [`Lane::allows`].
    pub veh_type: i32,
    pub lane: usize,
    pub s: f32,
    pub speed: f32,
    pub max_speed_kmh: f32,
    pub accel: f32,
    pub decel: f32,
    pub length: f32,
    pub rng: u64,
    pub blinker: i32,
    pub braking: bool,
    /// Distance travelled, for wheel animation.
    pub odometer: f32,
    /// Lane chosen for after the current one.
    pub planned_next: Option<usize>,
    /// The lanes after `planned_next`, decided `PLAN_AHEAD` metres in advance.
    pub ahead: Vec<usize>,
    /// During a lane change: the way on from the lane the car is moving over to, chosen
    /// when the change begins, so that the steering and the speed for the bends already
    /// know it.
    pub change_plan: Vec<usize>,
    /// The lane the car came from (the way behind it, for the rear axle).
    pub prev_lane: Option<usize>,
    /// Seconds spent waiting to enter a crossing (deadlock breaker).
    pub yield_time: f32,
    /// Fixed lane sequence (timetable track); empty = wander randomly.
    pub route: Vec<usize>,
    /// Position in `route` of the current lane.
    pub route_index: usize,
    /// Lane change in progress.
    pub change: Option<LaneChange>,
    /// Seconds until the next lane change may start.
    pub change_cooldown: f32,
    /// Lateral offset from the lane (m, positive = right): bus bays, swerving round a
    /// parked car. It moves towards `lateral_target` along an S-curve over the distance
    /// driven (`lateral_ramp`), so that the car is straight again when it gets there and
    /// nothing moves while it stands.
    pub lateral: f32,
    pub lateral_target: f32,
    /// The S-curve in progress: (from, to, odometer at its start, its length in m).
    pub lateral_ramp: (f32, f32, f32, f32),
    /// A turn the car has taken a turn lane for (1 left, 2 right): the way out of the
    /// junction is chosen to match.
    pub turn_wish: i32,
    /// Indicator the driver sets for a manoeuvre of its own (pulling away from a stop),
    /// held while `signal_time` runs.
    pub signal: i32,
    pub signal_time: f32,
    /// Sideways acceleration the driver accepts in a bend (m/s²): the speed through a
    /// curve of radius r is at most sqrt(this × r).
    pub lat_accel: f32,
    /// The driver (`TPathInfo`'s rowdy_factor & co): how fast they like to go relative to
    /// the limit, the time gap they keep to the car ahead (s), the distance they stop
    /// behind it (m), the gap in the cross traffic they accept at a junction (s) and how
    /// long they take to move off when the way clears (s). `accel` is how hard they pull
    /// away and `decel` how hard they like to brake.
    pub desire: f32,
    pub headway: f32,
    pub min_gap: f32,
    pub accept_gap: f32,
    pub reaction: f32,
    /// From the vehicle's origin (the lane position `s`) to its front and rear bumper (m).
    pub front: f32,
    pub rear: f32,
    /// Standing still and held there (by a car ahead, a light, a stop): moving off again
    /// starts after `reaction`; `start_timer` counts it down.
    pub held: bool,
    pub start_timer: f32,
    /// Acceleration of the last step (m/s²).
    pub acc: f32,
    /// At most this much acceleration for now (m/s²): edging out round something standing
    /// close ahead.
    pub accel_cap: Option<f32>,
}

/// What a car keeps its distance to: the gap from its front bumper to the thing (m), how
/// fast that is moving along the car's way (m/s) and how it is speeding up (m/s²).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Lead {
    pub gap: f32,
    pub speed: f32,
    pub acc: f32,
}

impl Lead {
    /// The nearer (more constraining) of two.
    pub fn min(a: Option<Lead>, b: Option<Lead>) -> Option<Lead> {
        match (a, b) {
            (Some(x), Some(y)) => Some(if y.gap < x.gap { y } else { x }),
            (x, None) => x,
            (None, y) => y,
        }
    }
}

impl AiState {
    pub fn new(lane: usize, s: f32, seed: u64) -> AiState {
        AiState {
            traffic_pool: None,
            veh_type: 0,
            lane,
            s,
            speed: 0.0,
            max_speed_kmh: 50.0,
            accel: 1.2,
            decel: 3.0,
            length: 5.0,
            rng: seed | 1,
            blinker: 0,
            braking: false,
            odometer: 0.0,
            planned_next: None,
            ahead: Vec::new(),
            change_plan: Vec::new(),
            prev_lane: None,
            yield_time: 0.0,
            route: Vec::new(),
            route_index: 0,
            change: None,
            change_cooldown: 5.0,
            lateral: 0.0,
            lateral_target: 0.0,
            lateral_ramp: (0.0, 0.0, 0.0, 1.0),
            turn_wish: 0,
            signal: 0,
            signal_time: 0.0,
            lat_accel: 2.8,
            desire: 1.0,
            headway: 1.4,
            min_gap: 2.0,
            accept_gap: 4.0,
            reaction: 0.7,
            front: 2.5,
            rear: 2.5,
            held: false,
            start_timer: 0.0,
            acc: 0.0,
            accel_cap: None,
        }
    }

    fn rand(&mut self) -> u64 {
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// The lanes ahead, nearest first: `planned_next`, then the rest of the plan.
    pub fn upcoming(&self) -> impl Iterator<Item = usize> + '_ {
        self.planned_next
            .into_iter()
            .chain(self.ahead.iter().copied())
    }
}
