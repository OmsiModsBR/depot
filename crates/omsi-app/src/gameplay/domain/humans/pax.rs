//! The types of the passengers as OMSI runs them: the passenger and its task, the stops it
//! waits at and asks for, the ride's comfort, the duty trips and the routes it takes. Their
//! procedures are in `implementation/humans/pax.rs`.

use super::BusId;
use glam::{DVec3, Vec3};
use hashbrown::HashSet;
use std::sync::Arc;

/// OMSI's tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Task {
    Nothing,
    WaitingForBus,
    /// Task 2: the bus comes, to the stop's gather point.
    ToBus,
    WalkingToBus,
    InBusToPlace,
    InBusToExit,
    WalkingToBusstop,
    SittingInBus,
}

/// One passenger's state: the fields of the original's human the tasks use.
#[derive(Debug, Clone)]
pub struct Pax {
    pub task: Task,
    /// Movement state: 0 stand, 1 to the target, 2 0.7 m short of it, 3 there, 5
    /// along the paths, 6 0.7 m short of the path's end, 7 at the path's end, 9 turning on
    /// the spot.
    pub st: u8,
    /// Inside a bus: `pos` and `yaw` are in its frame.
    pub inside: Option<BusId>,
    /// Where the feet are (the world, or the bus frame), and the heading (radians, 0 =
    /// forward, clockwise from above: Direct3D's yaw).
    pub pos: DVec3,
    pub yaw: f64,
    /// The bus dealt with, the stop, the waiting place there.
    pub bus: Option<BusId>,
    pub stop: Option<i64>,
    pub spot: Option<usize>,
    /// Where they stand when somebody of a neighbouring stop has their waiting place: that
    /// place moved along the kerb, on foot (see `take_spot`); `None` at the place itself.
    pub beside: Option<WaitSpot>,
    /// What state 1 walks to and whether it is a point of the bus (`bus`) or of
    /// the world; the heading to turn to there.
    pub target: DVec3,
    pub target_bus: bool,
    pub target_yaw: f64,
    /// The path point walked from / to and the one at the end.
    pub pt: Option<usize>,
    pub pt_target: Option<usize>,
    /// Stop 0.7 m short of the target.
    pub short: bool,
    /// Since when (`time`) the person has stood at the shut door of the bus they want
    /// (`DOOR_GIVE_UP`), and the bus they then left standing: not walked to again before it
    /// opens a door.
    pub door_since: Option<f64>,
    pub shunned: Option<BusId>,
    /// Walking to a door from outside: keep 0.5 m off the bus side
    /// (`clamp_x`) unless the door is open and they are level with it;
    /// a door on the left.
    pub clamp: bool,
    pub clamp_open: bool,
    pub clamp_left: bool,
    pub clamp_x: f64,
    /// Destination, the stop's line record it matched, the stop the line
    /// leaves the known route at and whether it was passed, the way on
    /// from there (m), the distance to ride (km) and the odometer at boarding
    /// (km).
    pub dest: Option<String>,
    pub line: Option<usize>,
    pub alt: Option<String>,
    pub alt_seen: bool,
    pub alt_m: f32,
    pub ride_km: f32,
    pub km_start: f64,
    /// Drawn once per passenger: where between departure and the approach they ask
    /// to get off. The stop and its request distance are settled when that leg begins.
    pub stop_request_random: f64,
    pub stop_request_at: Option<(i64, f64)>,
    /// The place reserved in the bus.
    pub seat: Option<usize>,
    /// The ticket kind, the ticket (1-based) and its price, what was paid,
    /// whether the change was wrong, whether the cash desk was free, and the ticket sale
    /// step.
    pub ticket: u8,
    pub ticket_id: u8,
    pub price: f32,
    pub paid: f32,
    pub bad_change: bool,
    pub sub: u8,
    /// The entry or exit asked for.
    pub door: Option<usize>,
    /// The validator they stamp at (an index into the cabin's: a bus may have several).
    pub stamper: Option<usize>,
    /// `HeightOfSeat` and `PAX_State` (0 stand, 1 walk, 2 sit).
    pub seat_h: f32,
    pub pax_state: f32,
    /// A countdown in seconds and one in metres walked.
    pub timer: f32,
    pub dist_timer: f32,
    /// The angles ease; talking to the driver; the right hand reaches
    /// for `reach_at` (bus frame); the head turns to the driver.
    pub smooth: bool,
    pub talking: bool,
    pub reach: bool,
    pub look_driver: bool,
    pub reach_at: Vec3,
    /// The room height of the link walked, its step sounds.
    pub room: f32,
    pub step_pack: Option<usize>,
    pub link: Option<usize>,
    /// Speed wanted, speed, walking pace.
    pub speed_des: f32,
    pub speed: f32,
    pub walk_speed: f32,
    /// Somebody in the way (1 behind, 2 in front facing them, 3 in front going
    /// the same way or busy) and on which sides there is room.
    pub block: u8,
    /// Seconds held up by somebody in front inside a bus, and seconds left passing them
    /// (see `pax_move`).
    pub jam: f32,
    pub squeeze: f32,
    pub free_r: bool,
    pub free_l: bool,
    /// How badly the ride has gone (0..1; see `ride_comfort`), the complaint said
    /// so far (1 TooBad_A, 2 TooBad_B, 3 TooBad_C - and off at the next stop) and
    /// where each one comes (drawn once, 0 not yet).
    pub discomfort: f32,
    pub complaint: u8,
    pub bad_at: [f32; 3],
    /// Distance moved this frame (`LastMovedDist`).
    pub moved: f32,
}

/// The next stop of the route, independently of the local boarding range. A planned
/// stop can be known before its tile and its waiting passengers have been loaded.
#[derive(Debug, Clone)]
pub struct RequestStop {
    pub id: i64,
    pub name: String,
    pub alias: String,
    pub pos: DVec3,
}

/// A waiting place of a stop (a `[passpos]` of an object near it).
#[derive(Debug, Clone)]
pub struct WaitSpot {
    /// The `[passpos]` point (world): the feet, or a seated person's hip.
    pub pos: DVec3,
    /// Heading (degrees, the world's).
    pub face: f64,
    /// Seat height; a seat when not 0.
    pub height: f32,
}

/// What OMSI keeps of a bus stop for the people (the station record).
pub struct PaxStop {
    pub name: String,
    /// Its name in the timetable (empty without one), where the passengers' destinations
    /// come from: the object's label is the stop's name to OMSI, but a map whose
    /// labels and `Busstops.cfg` disagree - a stop renamed, or the two files written in
    /// different code pages - had riders whose stop never came, and who rode on for good.
    pub alias: String,
    pub pos: DVec3,
    /// The object's heading (degrees).
    pub heading: f64,
    /// Where people gather when a bus comes: a metre to the side and a metre
    /// along the stop.
    pub gather: DVec3,
    pub spots: Vec<WaitSpot>,
    /// Which places are taken.
    pub taken: Vec<bool>,
    /// pass_enter_max / _min and the length (30 by default).
    pub enter_max: f32,
    pub enter_min: f32,
    pub length: f32,
    /// The pavement next to it (where those getting off walk on).
    pub lane: Option<(usize, f32)>,
    /// In range of the player last time and now, the refill clock
    /// (ms), people wanted and there, the stop's factor, first fill
    /// done.
    pub was_near: bool,
    pub near: bool,
    pub clock_ms: f32,
    pub want: usize,
    pub factor: f32,
    /// The buses listed here this frame: (bus, standing in the stop's box).
    pub buses: Vec<(BusId, bool)>,
    /// The destinations: stop name, weight; and the line records: the
    /// stop name and the termini of the buses that go there.
    pub dests: Vec<(String, f32)>,
    pub lines: Vec<(String, HashSet<String>)>,
}

/// How the player's bus is driven, as OMSI watches it for the riders: the longitudinal acceleration eased over a tenth of a
/// second, the lateral one over a second (both weighed down below 1 m/s), and the swings
/// of the first between +0.2 and -0.2 m/s² (a jerky right foot).
#[derive(Debug, Clone, Default)]
pub struct RideComfort {
    /// The eased longitudinal and lateral accelerations (m/s²).
    pub(crate) fast_long: f32,
    pub(crate) slow_lat: f32,
    /// The last swing went up, when (ms) and how many came in a row.
    pub(crate) up: bool,
    pub(crate) swing_ms: f64,
    pub(crate) swings: u32,
    /// The last hard bend or braking (ms).
    pub(crate) hard_ms: f64,
}

/// What the stops say about a bus this frame: the stop ahead it is pulling
/// in to, the stops within 60 m, and whether it empties.
#[derive(Debug, Clone, Default)]
pub struct BusAtStops {
    pub next: Option<i64>,
    pub request_next: Option<RequestStop>,
    pub near: Vec<i64>,
    pub all_exit: bool,
    /// The player asked for this bus to be emptied (Alt bar, destination window): everyone on
    /// board gets off by an open door, wherever the bus is and whatever they were going to.
    pub forced: bool,
}

/// The trip the player's duty has the bus on, as the people at the stops see it.
#[derive(Debug, Clone, PartialEq)]
pub struct DutyTrip {
    /// Which trip it is (its name and departure), to notice the next one.
    pub name: String,
    pub departure: f64,
    /// Its terminus as the timetable has it: what the stops' line records list.
    pub terminus: String,
    /// It has a line: not a works trip to or from the depot (whose stations it passes).
    pub public: bool,
    /// Its stops in order: the object, its timetable name (what the destinations of the
    /// people waiting are made of) and whether the bus stops there.
    pub stops: Vec<(i64, String, bool)>,
}

/// Whom a bus takes on at the stops (see `at_stop` and `fit`).
#[derive(Debug, Clone)]
pub enum Takes {
    /// Those whose line record lists its terminus, as in OMSI: a timetable bus.
    Terminus,
    /// The player's bus on a duty: those as well whom its trip takes where they are going,
    /// however the bus's depot file spells the terminus. `next` is the stop of the trip the
    /// duty is due at, `done` that the trip has reached its last stop.
    Duty {
        trip: Arc<DutyTrip>,
        next: usize,
        done: bool,
    },
    /// Nobody waiting: another player's bus (their game boards it), a bus the player left
    /// standing (its riders get off as ever).
    Nobody,
}

/// What a bus near a stop does there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtStop {
    /// Everybody gets off and nobody on: the bus shows no destination (it is not in
    /// service), or it is at its terminus.
    Empties,
    /// It is listed at the stop: the people waiting there may take it.
    Serves,
    /// Only its riders get off there: the people waiting leave it alone.
    Passes,
}

/// Why somebody waiting takes a bus (`fit`); the better first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Fit {
    /// Its terminus is on their line record (OMSI's only test).
    Terminus,
    /// The player's duty takes them where they are going: its trip's terminus is on the
    /// record, or their destination is a later stop of the trip than theirs.
    Duty,
    /// They have no line record: the first bus listed.
    Any,
}

/// A point of the cabin's path network with its links in the file's order: the point at
/// the other end, the points reached through it, the link's index, its room
/// height and step sounds.
#[derive(Debug, Clone)]
pub struct RouteLink {
    pub to: usize,
    pub reach: Vec<usize>,
    pub link: usize,
    pub walk_back: bool,
}
