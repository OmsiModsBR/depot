//! The types of the AI road traffic: the vehicles on the path network, their state at
//! junctions and lights, the parked cars and the population around the player. Their
//! procedures are in `implementation/traffic.rs`.

use crate::gameplay::bus_service::BusService;
use crate::world::scene::VehicleRender;
use glam::{DVec2, DVec3};
use hashbrown::HashMap;
use omsi_sim::ai::AiMotion;
use omsi_sim::traffic::{AiState, LaneKind, Network, TrafficLightController};
use omsi_sim::{VehicleInstance, VehicleType};
use std::sync::Arc;
use std::time::Instant;

/// Pulling out onto the other half of the road round something standing in the lane.
#[derive(Debug, Clone, Copy)]
pub struct Passing {
    /// The lane of the oncoming traffic the car moves over onto.
    pub lane: usize,
    /// How far to the left that lane lies (m).
    pub side: f32,
    /// Odometer reading at which the car is past the obstacle and moves back.
    pub until: f32,
    /// Odometer reading at which the car's front would reach the obstacle (had it stayed in
    /// its lane): until then it can still give up and stop behind it.
    pub block: f32,
    /// Length of the S-curve back into the lane (m).
    pub back: f32,
    /// Given up because somebody came the other way: back in and stopping at the odometer
    /// reading `hold`.
    pub aborted: bool,
    pub hold: f32,
    /// Started from a standstill close behind the obstacle: the car edges out
    /// (`PULL_OUT_ACCEL`) until its front is past the obstacle's corner.
    pub creep: bool,
}

/// An ordinary car that makes way for an emergency vehicle coming up behind it
/// (`Traffic::update_way_makers`).
#[derive(Debug, Clone, Copy)]
pub struct WayMaker {
    /// When it began to make way (it began afresh after a second of not doing so).
    pub since: f32,
    /// The last time it was in an emergency vehicle's way.
    pub seen: f32,
    /// It does not make way before this time (it gave up after `MAKE_WAY_MAX_SECS`).
    pub ignore_until: f32,
    /// Making way now.
    pub active: bool,
    /// The gap from the emergency vehicle's front to this car's rear (m).
    pub behind: f32,
}

/// What makes a new car a timetable bus (`Traffic::create_car`).
pub struct BusSetup {
    /// The trip's lanes, as far as the loaded tiles have them.
    pub route: Vec<usize>,
    pub stops: Vec<crate::gameplay::bus_service::Stop>,
    /// Fleet number and registration (`number`, `ident` string variables).
    pub number: Option<(String, String)>,
    pub hof: Option<Arc<omsi_vehicle::Hof>>,
    pub timetable: crate::gameplay::bus_service::AiTimetable,
}

pub struct AiCar {
    /// Stable id for references from other systems (passengers).
    pub id: u64,
    /// The random seed it was made with and its paint scheme: a car that goes out of range
    /// and comes back is the same car (`DormantCar`).
    pub seed: u64,
    pub scheme: Option<usize>,
    pub state: AiState,
    pub vehicle: VehicleInstance,
    pub render: VehicleRender,
    pub trailer_renders: Vec<VehicleRender>,
    /// The body following the way `state` lays out.
    pub body: AiMotion,
    /// Seconds this car has been standing still without a stop of its own: a red light or
    /// a queue is seconds, a jam that never clears grows without bound.
    pub stopped: f32,
    /// The car it follows now (its id), when one is close ahead.
    pub lead_car: Option<u64>,
    /// A car it does not take for its lead until the time given: two that had each other
    /// for their lead (see `Traffic::break_lead_pairs`).
    pub ignore_lead: Option<(u64, f64)>,
    /// Seconds it has crept along below 1 m/s (a claim of one that crawls in a jam of its
    /// own is no car about to come either).
    pub crawl: f32,
    /// The odometer when it last got two metres further, and the seconds since: a car
    /// that creeps against something it never gets past stands as much as one that stops
    /// (`stopped` starts afresh with every centimetre it creeps).
    pub progress: (f32, f32),
    /// A timetable bus: its trip's stops, the doors, the layover, the people aboard (see
    /// `bus_service`). Everything else about it is this car's.
    pub bus: Option<Box<BusService>>,
    /// `[sound_ai]` set, created when the car comes near the listener.
    pub sounds: Option<omsi_audio::SoundSet>,
    /// Half the vehicle's width (m).
    pub half_width: f32,
    /// Waiting at a junction for someone with the right of way this frame.
    pub yielding: bool,
    /// Stopped by a red light this frame.
    pub light_hold: bool,
    /// Junction lanes this car has claimed to drive through (`TPathInfo::reservePaths`).
    pub reserved: Vec<usize>,
    /// The light (controller, lamp) the driver decided to pass on yellow.
    pub amber: Option<(usize, usize)>,
    pub passing: Option<Passing>,
    /// Finished (a dead end, the end of a timetable trip, given up): taken off the road as
    /// soon as nobody can see it.
    pub gone: bool,
    /// Seconds since it was put on the road are fewer than this: its speed was a guess.
    pub fresh: f32,
    /// The car it lets go first at the next merge (by id).
    pub merge_after: Option<u64>,
    /// What holds it (`OMSI_DEBUG_TRAFFIC`, for cars standing for long).
    pub holding: Option<String>,
    /// What held the car back this frame (for OMSI_TRACE_AI): the constraint nearest ahead
    /// ("lead", "light", "yield", "merge", "keep_back", "people", "pull_out", "service",
    /// "end", "" for none) and its distance ahead of the front (m).
    pub why: (&'static str, f32),
    /// Something made it wait this frame: a stop point, or a car or an obstacle close ahead.
    pub held: bool,
    /// The vehicle (by id) whose body stands in this car's way off its lanes this frame
    /// (`Traffic::body_in_way`).
    pub geo_block: Option<u64>,
    /// What it keeps behind (by id) and the gap to it, as of its last step.
    pub lead_info: Option<(u64, f32)>,
    /// What it waited for at its last junction (`OMSI_DEBUG_STUCK` only).
    pub junction_why: String,
    /// Giving way: where it waits (distance from its origin to the line).
    pub wait_at: Option<f32>,
    /// The vehicle (by id) standing half out of the lane that this car is squeezing past.
    pub squeeze: Option<u64>,
    /// How far behind something standing (a bus at its stop, the player's bus) this car
    /// stops, so that it can steer out round it later (m, front bumper to the other's body;
    /// from its own steering, `pull_out_room`).
    pub pass_room: f32,
    /// No new look at passing before this time (a pull-out that did not clear the corner is
    /// not tried again every frame).
    pub pass_retry: f32,
    /// The traffic light it waited for in the last frame: distance from its origin.
    pub light_at: Option<f32>,
    /// A car that was parked at the kerb: seconds it still stands there, indicating, before
    /// it pulls out (see `Traffic::pull_out_parked`).
    pub pull_out: f32,
    /// Parking: the free space it drives into (see `Traffic::park_in`).
    pub park: Option<ParkPlan>,
    /// A rail vehicle: the track it has come along, (odometer, point), oldest first -
    /// where its rear bogie and its coupled cars and sections run (see `rail_behind`).
    pub rail_trail: std::collections::VecDeque<(f64, DVec3)>,
    /// Seconds its body and script took last frame (heavy ones get an AI job of their own).
    pub ai_secs: f32,
    /// A train turned round as a whole (its last car leads now): what a trip's
    /// `[trainreverse]` is compared with.
    pub consist_reversed: bool,
}

/// A free parking space beside a lane that a car means to park in: the space of parked car
/// `key` that drove off (its object comes back when the car is in).
#[derive(Debug, Clone, Copy)]
pub struct ParkPlan {
    pub key: i64,
    pub lane: usize,
    /// The space's middle along the lane and its offset to the right of it (m).
    pub s: f32,
    pub lat: f32,
    /// Moving over into the space.
    pub ramped: bool,
    /// In the space and standing: the parked object takes its place at the next sync.
    pub done: bool,
}

/// Where a vehicle's body stands, for the checks that go by geometry rather than by lanes:
/// the car it belongs to (a trailer or rear section counts as its own footprint), centre,
/// forward and right unit vectors, half length, half width and speed along its heading.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Footprint {
    pub(crate) car: usize,
    pub(crate) center: DVec2,
    pub(crate) fwd: DVec2,
    pub(crate) right: DVec2,
    pub(crate) half_len: f64,
    pub(crate) half_w: f64,
    pub(crate) speed: f32,
    /// Height of the vehicle's origin (an aircraft overhead is not in a car's way).
    pub(crate) z: f64,
}

/// The player's vehicle as the traffic sees it: centre, heading (deg), half length, half
/// width, speed along the heading (m/s, negative when reversing).
pub type PlayerBox = (DVec3, f64, f32, f32, f32);

/// Where the player looks from, for putting cars on the road and taking them off only
/// where nobody sees it happen.
#[derive(Debug, Clone, Copy)]
pub struct Viewer {
    pub pos: DVec3,
    pub forward: DVec3,
    /// Tangents of half the horizontal and vertical field of view.
    pub tan_x: f64,
    pub tan_y: f64,
    /// Beyond this distance nothing shows (fog, or a car smaller than a pixel) (m).
    pub range: f64,
    /// What the renderer leaves out (see `omsi_render::RenderOptions`): objects smaller on
    /// the screen than `min_size` (the original's measure), and farther than `max_dist`
    /// (0 = no limit); `fov` is the vertical field of view (radians).
    pub min_size: f64,
    pub max_dist: f64,
    pub fov: f64,
}

/// A random car out of the player's range: still on the map and still driving, but without
/// a body, a script or a picture - a few numbers. It comes back as the same car (type,
/// paint, id) where it has got to when the player comes near, and it only ever leaves the
/// map at the end of the road network. Before, every car out of range was simply taken
/// away and new ones made up around the player: the traffic followed the player about, and
/// a car driven past was never seen again.
pub struct DormantCar {
    pub id: u64,
    pub ty: Arc<VehicleType>,
    pub kind: LaneKind,
    pub lane: usize,
    pub s: f32,
    pub speed: f32,
    pub seed: u64,
    pub scheme: Option<usize>,
    /// Its own dice for the turns it takes.
    pub walk: u64,
}

/// Where a population pass (`Traffic::populate_seen`) spent its time (s), and what it did.
#[derive(Debug, Clone, Copy, Default)]
pub struct PopulateSplit {
    /// Taking away the cars out of range or with no ground under them, and sending them
    /// to sleep.
    pub despawn: f64,
    /// Waking the sleeping cars that came into range again.
    pub wake: f64,
    /// The target and the sleeping cars of the rest of the map.
    pub fill: f64,
    /// Looking for places and putting new cars there.
    pub spawn: f64,
    /// Of `wake` and `spawn`: making the cars (body, scripts, render set).
    pub create: f64,
    /// Parked cars pulling out and cars parking.
    pub parked: f64,
    /// Cars put on the road (woken or new), and places tried for new ones.
    pub created: u32,
    pub tried: u32,
    /// The pass stopped at its budget with cars still missing.
    pub cut: bool,
}

pub struct Traffic {
    pub net: Network,
    /// Parked cars standing in or beside a lane: per lane, (distance along it, signed
    /// lateral offset of the car's centre, + = right). A car in the lane's middle is an
    /// obstacle to stop behind; one over the kerb side is passed with a swerve to the left.
    pub parked: HashMap<usize, Vec<(f32, f32)>>,
    /// Parked cars no lane has been found beside yet: the lane may come with a tile that is
    /// not loaded yet (a road spline often starts in the next tile). Most stand in car parks
    /// and stay here.
    pub parked_waiting: Vec<DVec3>,
    /// Tiles whose lanes the network has (lanes stay once they are in).
    pub lane_tiles: hashbrown::HashSet<(i32, i32)>,
    /// Counts the times tiles brought their lanes: whoever resolved something against the
    /// network and missed a part of it looks again when this changes.
    pub lanes_generation: u64,
    /// AI vehicle types with weight, the lane kind they run on (`[type]` 2 rail, 3 air)
    /// and their group in `groups`.
    pub types: Vec<(Arc<VehicleType>, f32, LaneKind, usize)>,
    /// The random traffic groups: name, `unsched_trafficdens.txt` factor and day curves.
    pub groups: Vec<omsi_map::ailists::UnschedGroup>,
    /// The map has an `unsched_trafficdens.txt` (else the global.cfg curve applies).
    pub group_curves: bool,
    /// Each group's place in `unsched_vehgroups.txt`, the number a path's `[rule]
    /// trafficdensity` names it by (None: the map has no such file, and every group drives
    /// wherever the lane's density lets traffic).
    pub group_uvg: Vec<Option<usize>>,
    /// The default density of every `unsched_vehgroups.txt` entry, in file order: 0 none, 1
    /// for the first entry its medium density, for any other that of the first entry, 2 of
    /// the second, and so on. It applies on the paths without a rule for the group.
    pub uvg_defaults: Arc<Vec<i32>>,
    pub cars: Vec<AiCar>,
    /// The random cars out of range (see `DormantCar`).
    pub dormant: Vec<DormantCar>,
    /// `time` when the dormant cars last moved on.
    pub dormant_time: f32,
    /// When the lanes' spawn clocks last ran down.
    pub spawn_time: f32,
    /// Each street lane's wait to its next arrival (None: nothing to wait for yet, its first
    /// fill is still to come).
    pub lane_clocks: Vec<Option<crate::gameplay::traffic_density::SpawnClock>>,
    /// Where the last spawn pass stopped when it ran out of budget.
    pub spawn_cursor: usize,
    pub rng: u64,
    /// Whether random street traffic runs at all (0: none; it no longer sets the density).
    pub target: usize,
    /// Made only so that the light programs run (no traffic, no timetable): nobody is put
    /// on the roads - no aircraft, no parked car pulling out - while `target` is 0.
    pub lights_only: bool,
    pub spawn_radius: f64,
    pub time: f32,
    /// Renders of cars that have gone, given back at the next `sync`.
    pub released: Vec<VehicleRender>,
    /// Where the camera is (the window sets it before `sync`): far cars show their script
    /// textures as stand-ins.
    pub camera: Option<DVec3>,
    /// Sound sets of despawned cars, stopped at the next audio update.
    pub orphan_sounds: Vec<omsi_audio::SoundSet>,
    pub lights: Vec<TrafficLightController>,
    pub controller_of_object: HashMap<i64, usize>,
    /// Coupled vehicle types by file.
    pub trailer_types: HashMap<std::path::PathBuf, Option<Arc<VehicleType>>>,
    /// `[sound_ai]` configurations by file.
    pub sound_cfgs: HashMap<std::path::PathBuf, Option<Arc<omsi_vehicle::SoundCfg>>>,
    pub root: std::path::PathBuf,
    /// Car-frames spent waiting for a red light (statistics).
    pub held_at_red: usize,
    /// The cars that make way for an emergency vehicle behind them, by car id (empty while
    /// no emergency vehicle is on the road).
    pub way_makers: HashMap<u64, WayMaker>,
    /// Who wants a timetable bus to stop (`Humans::stop_wishes`): the buses somebody
    /// aboard wants to get off, the stops where somebody waits. None without passengers:
    /// every bus then serves every stop.
    pub stop_wishes: Option<(hashbrown::HashSet<u64>, hashbrown::HashSet<i64>)>,
    /// Seconds the player's vehicle has been standing.
    pub player_still: f32,
    /// Time of day (seconds since midnight); light cycles and timetables run on it.
    pub day_time: f64,
    /// How fast the clock runs (the time speed): the timetable keeps to it.
    pub time_scale: f64,
    /// Day of the week (0 Monday … 6 Sunday) for the traffic density curves.
    pub weekday: i32,
    /// Street lights on → AI vehicles switch their lights on.
    pub night: bool,
    /// The light of the day, for the cars' `Envir_Brightness` (see `sync`).
    pub daylight: Option<omsi_sim::Daylight>,
    pub next_id: u64,
    /// The last car that started an overtake and when (for chase-camera debugging).
    pub last_overtaker: Option<(u64, f32)>,
    /// The first car that entered a turning lane and when (`--follow turn`).
    pub first_turner: Option<(u64, f32)>,
    /// The first car that stopped at a red light (`--follow red`), the first that gave way
    /// at a junction (`--follow yield`), the first that pulled out onto the other side of
    /// the road round an obstacle or squeezed past a bus at its stop (`--follow pass`).
    pub first_red: Option<(u64, f32)>,
    pub first_yield: Option<(u64, f32)>,
    pub first_passer: Option<(u64, f32)>,
    /// `[trafficdensity_road]` curve of the map: (hour, factor).
    pub density_curve: Vec<(f32, f32)>,
    /// The options' `[AIUnschedFactor]`: the share of the random traffic.
    pub unsched_factor: f32,
    /// The options' `[AIMaxCountScheduled]`: no more timetable vehicles than that at once.
    pub max_scheduled: u32,
    /// The options' `[AIMaxCountRandom]` (first line): no more vehicles without a timetable
    /// than that on the streets at once.
    pub max_random: u32,
    /// The options' `physics`: the engine of every AI vehicle (the bus key is the player's only).
    pub physics: omsi_cfg::PhysicsMode,
    /// `--no-timetable-buses`: the timetable runs for the player's duty, but puts no AI
    /// bus on the road (#1762).
    pub no_timetable_buses: bool,
    /// Where the player looks from (set every frame).
    pub viewer: Option<Viewer>,
    /// Buildings that hide what is behind them (the player's collision world).
    pub occluders: Option<Arc<omsi_sim::collision::CollisionWorld>>,
    /// Pedestrians on the footpaths: (lane, distance along it), for giving way at crossings
    /// and for the pedestrian lights' request buttons.
    pub walkers: Vec<(usize, f32)>,
    /// Everybody on foot on the ground: position, velocity and whether they are waiting
    /// at a stop (set every frame) - the cars stop for anybody in their way, not only on
    /// a crossing.
    pub people: Vec<(DVec3, DVec2, bool)>,
    /// No car has been placed yet: the first population may fill the view.
    pub initial: bool,
    /// Seconds of the last tick (the lamp scripts run in `sync`).
    pub last_dt: f32,
    /// Game time since the lamps' scripts last ran (see `sync`).
    pub lamp_dt: f32,
    /// `OMSI_TRACE_AI=<file.csv>`: every car's pose, steering and speed, every frame.
    pub trace: Option<std::io::BufWriter<std::fs::File>>,
    /// Cars already reported for a hard bend (`OMSI_DEBUG_TRAFFIC`).
    pub logged_hard: hashbrown::HashSet<u64>,
    /// `OMSI_DEBUG_LIGHTS=all|near|<controller>,…`: which programs log their changes, and
    /// the states they showed last.
    pub light_log: Option<String>,
    pub light_prev: Vec<Vec<i32>>,
    /// `OMSI_DEBUG_POPULATION`: log where cars appear and vanish relative to the view.
    pub debug_population: bool,
    /// Cars placed since the last look inside the view frustum (hidden behind something):
    /// (id, position). `OMSI_POPULATION_SHOTS` photographs them to check.
    pub framed_spawns: Vec<(u64, DVec3)>,
    /// The player's vehicle as of the last tick (nothing is put on the road on top of it).
    pub player: Option<PlayerBox>,
    /// The player's bus has right of way over the traffic (its script's `TrafficPriority`,
    /// OMSI: priority 1000 over the types' own): cars keep out of the way it is about
    /// to take for longer.
    pub player_priority: bool,
    /// The player's indicators (0 off, 1 left, 2 right, 3 hazard; `lan::indicator`), set
    /// before each `tick`.
    pub player_blinker: u8,
    /// Seconds since the player's bus last showed the indicator towards the traffic (the
    /// lamps go dark half of the time).
    pub player_signal_age: f32,
    /// ... and for how long it has been indicating so (s).
    pub player_signalling: f32,
    /// The player's vehicle and the LAN players' as the junctions see them (`way_user_on`),
    /// as of this tick.
    pub way_users: Vec<WayUser>,
    /// The LAN players' vehicles (their session ids and boxes as for the player), set
    /// before each `tick`: the cars stop behind them and go round them as round the
    /// player's bus.
    pub others: Vec<(u32, PlayerBox)>,
    /// The drivers at the wheel of the timetable buses near the camera, by car id (see
    /// `driver.rs`; made within `DRIVER_NEAR` m of the camera, let go beyond twice that).
    pub drivers: HashMap<u64, crate::gameplay::driver::DriverFigure>,
    /// Figures let go by their bus, hidden, for the next one (their GPU meshes stay).
    pub driver_pool: Vec<crate::gameplay::driver::DriverFigure>,
    /// Where the last `tick` spent its time (s, OMSI_PROFILE): who is on which lane and the
    /// light programs, every car's plan, the bodies and scripts on the workers.
    pub tick_split: [f64; 3],
    /// Where the last population pass spent its time and what it did (OMSI_PROFILE).
    pub populate_split: PopulateSplit,
    /// How long (s) a population pass may spend putting cars on the road, after the first
    /// one; None: until the target is met (the default, kept by the tests and the
    /// offscreen pictures). A pass cut short says so in `populate_split.cut`, and the game
    /// comes back for the rest a moment later: creating every car missing at once held a
    /// frame for 30 to 60 ms after the start, and for 250 to 500 ms on reaching a dense
    /// area (#43).
    pub populate_budget: Option<f64>,
    /// When the pass under way has to stop putting cars on the road.
    pub spawn_deadline: Option<Instant>,
    /// Seconds each of them has stood still.
    pub others_still: HashMap<u32, f32>,
    /// Per car: `AiCar::geo_block` of the frame before (who waits for whom by geometry).
    pub geo_prev: Vec<Option<u64>>,
    /// Car index by id (as of the start of the tick).
    pub index_of: HashMap<u64, usize>,
    /// `pull_out_room` by vehicle file.
    pub pull_out_rooms: HashMap<std::path::PathBuf, f32>,
    /// Timetable buses taken off the road because the tile under them was unloaded (their
    /// ids), for the timetable to put them back when the tiles come again.
    pub removed_scheduled: Vec<u64>,
    /// One-way lanes that have had their reverse twin added (`add_reverse_twins`).
    pub twinned: hashbrown::HashSet<usize>,
    /// Bodies besides the AI vehicles' that no timetable vehicle may be put into: the
    /// player's vehicle and the LAN players' (set before each `Schedule::tick`).
    pub keep_clear: Vec<omsi_sim::collision::Obb>,
    /// LAN play: this game draws the host's traffic instead of its own (`lan_world`).
    pub mirror: bool,
    /// LAN play: where the other players are (host): the traffic is kept around them too.
    pub lan_centers: Vec<DVec3>,
}

/// Where a vehicle meets a crossing lane on its way: its lane in the sequence, the distance
/// from its origin to that lane's start.
#[derive(Debug, Clone)]
pub(crate) struct Junction {
    /// (lane, distance from the car's origin to its start) of the junction's lanes on the
    /// car's way; the first is where it has to wait.
    pub(crate) lanes: Vec<(usize, f32)>,
    /// The lane after the junction and the distance to its start.
    pub(crate) exit: Option<(usize, f32)>,
    /// The car is already on one of the junction's lanes.
    pub(crate) inside: bool,
}

/// A vehicle the AI does not drive (the player's bus, a LAN player's) as the right of way
/// sees it: OMSI keeps the player's vehicle on the paths like any AI vehicle, so the
/// cars give way to it by the same rules; here it is put onto the lanes where it is.
#[derive(Debug, Clone)]
pub struct WayUser {
    /// The lane it is on and those it may take next: (lane, distance from its centre to
    /// the lane's start - negative for the lane it is on).
    pub lanes: Vec<(usize, f32)>,
    /// Speed along its way (m/s), half its length (m), how long it has stood (s).
    pub speed: f32,
    pub half_len: f32,
    pub still: f32,
    /// Its script claims priority (`TrafficPriority`).
    pub prio: bool,
}

/// What a vehicle the AI does not drive means for a car at a meeting place of its junction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Verdict {
    /// Nothing: through already, far off, standing, or the car goes first.
    Free,
    /// It is in the meeting place, or in the junction and there first: wait (a car that
    /// cannot stop any more still goes).
    Hard,
    /// It has the right of way and comes within the gap the driver would take.
    Ruled,
}
