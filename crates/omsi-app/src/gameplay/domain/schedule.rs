//! The types of the scheduled AI buses: the map's timetable lines, their trips and tours,
//! the buses running them and their state at the stops. Their procedures are in
//! `implementation/schedule.rs`.

use hashbrown::{HashMap, HashSet};
use omsi_sim::traffic::LaneKey;
use omsi_sim::VehicleType;
use omsi_timetable::{TimetableData, TripTimes};
use std::sync::Arc;

pub(crate) struct Departure {
    /// Seconds since midnight.
    pub(crate) time: f64,
    pub(crate) trip: usize,
    /// The trip's profile the tour runs it with (`[addtrip]`).
    pub(crate) profile: usize,
    pub(crate) line: String,
    pub(crate) ai_group: String,
    pub(crate) tour: String,
    /// The timetable line's `[priority]` (1 the most important line).
    pub(crate) priority: i32,
    /// The tour's validity mask (bits 0-6 Monday..Sunday, 7 public holiday, 8 school
    /// holidays, 9 school days): every tour's departures are kept, and which run is decided
    /// by the day (`Schedule::runs`), so a session carries on past midnight.
    pub(crate) mask: i32,
    pub(crate) spawned: bool,
}

/// One step of a trip's route: a lane in map terms (None when the tile index is not in the
/// map's list) and the leg between two stations it belongs to (0 for a track).
#[derive(Debug, Clone, Copy)]
pub struct Step {
    pub key: Option<LaneKey>,
    pub leg: usize,
    /// The path's length as the timetable file has it (m).
    pub length: f64,
}

/// A route step as the loaded network has it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Slot {
    /// The lane it runs on.
    Lane(usize),
    /// Its tile is part of the map, but has not brought its lanes yet.
    Waiting,
    /// Not in the map (a tile or path that does not exist): passed over, as a whole-map load
    /// passes it over.
    Absent,
}

/// What became of a departure that was due.
pub(crate) enum Placed {
    Spawned,
    /// The part of the route the bus is on now is not loaded: try again later.
    Wait,
    /// Nothing to do any more (the trip is over, or has no route or no vehicle).
    Drop,
    /// A car stands where the bus would appear: try again at the next call.
    Busy,
}

/// A scheduled bus whose route stops short of a tile that has not brought its lanes yet: the
/// route is carried on as the tiles come.
pub(crate) struct RunningTrip {
    pub(crate) car: u64,
    pub(crate) steps: Vec<Step>,
    /// The first step its route does not have yet.
    pub(crate) next: usize,
    /// The trip's stations with their departure times, and which of them the bus stops at
    /// already or has passed.
    pub(crate) stations: Vec<(i64, f64)>,
    pub(crate) served: Vec<bool>,
    /// Authored track entry of each type-1 station, retained while tiles stream in.
    pub(crate) station_steps: Vec<Option<usize>>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StopRoute {
    Nearest,
    Track(usize),
    Outside,
}

/// The vehicle a departure is driven with. A tour keeps its bus all day, as in OMSI: the
/// choice comes from the tour, not from the order the departures happen to spawn in, so the
/// timetable knows which vehicles its next minutes need before they are due.
pub(crate) struct Choice {
    pub(crate) ty: Arc<VehicleType>,
    pub(crate) number: Option<(String, String)>,
    pub(crate) hof: Option<Arc<omsi_vehicle::Hof>>,
    pub(crate) scheme: Option<usize>,
    /// A `.zug` train: its cars, the first one being `ty`.
    pub(crate) train: Option<Vec<(Arc<VehicleType>, bool)>>,
}

pub struct Schedule {
    pub data: TimetableData,
    /// The options' `[AIPriorityScheduled]`: the least important line (1 to 4) that runs;
    /// the player's own tour runs whatever its line's priority.
    pub(crate) max_priority: i32,
    pub(crate) departures: Vec<Departure>,
    /// Depot vehicles per AI group: (type, its fleet from the ailists, depot file).
    pub(crate) depots: HashMap<
        String,
        Vec<(
            Arc<VehicleType>,
            Vec<omsi_map::DepotEntry>,
            Option<Arc<omsi_vehicle::Hof>>,
        )>,
    >,
    pub(crate) tile_coords: Vec<(i32, i32)>,
    pub(crate) next_number: usize,
    /// Trains per AI group: list of (car type, reversed), first car leads.
    pub(crate) trains: HashMap<String, Vec<Vec<(Arc<VehicleType>, bool)>>>,
    /// Plain `[aigroup_2]` vehicle pools, loaded the first time a trip asks for one
    /// (the Tegel approaches are flown by the group's own aircraft, not by depot buses).
    pub(crate) pools: HashMap<String, Vec<Arc<VehicleType>>>,
    /// Per vehicle folder: the depot file a bus of a plain `[aigroup_2]` runs with
    /// (see [`pool_depot`]), looked up the first time such a bus is chosen.
    pub(crate) pool_hofs: HashMap<std::path::PathBuf, Option<Arc<omsi_vehicle::Hof>>>,
    /// Departures that are due but not on the road yet. Putting twenty minutes of a Berlin
    /// timetable on the map at once costs several seconds in one frame, so they are spawned
    /// a few at a time.
    pub(crate) pending: std::collections::VecDeque<usize>,
    /// Per departure: the previous departure of the same tour (its bus is the same one).
    pub(crate) tour_prev: Vec<Option<usize>>,
    /// Due departures whose bus would be on a part of its route that is not loaded: tried
    /// again when tiles bring lanes and as time moves the bus on.
    pub(crate) waiting: Vec<usize>,
    /// Buses on the road whose route is still to be carried on.
    pub(crate) running: Vec<RunningTrip>,
    /// `Traffic::lanes_generation` when the waiting departures and the running routes were
    /// last looked at, and the time of day of the last retry.
    pub(crate) seen_generation: u64,
    pub(crate) last_retry: f64,
    /// The departure each timetable bus on the road runs (by car id): a bus the traffic took
    /// off with its unloaded tile goes back to `waiting` and returns with the tiles, at the
    /// place its timetable puts it then.
    pub(crate) car_departure: HashMap<u64, usize>,
    /// Waiting departures whose vehicle is timed to reach loaded lanes at this time of day:
    /// they are tried again then, so that a plane coming in over tiles nobody loads appears
    /// where its path enters the loaded ones, not up to half a minute later in mid-air.
    pub(crate) retry_at: HashMap<usize, f64>,
    /// Vehicle sets being read ahead on the workers, with the type to upload them with, and
    /// those read and waiting for their upload.
    pub(crate) fleet_reading: HashMap<crate::world::scene::VehicleKey, Arc<VehicleType>>,
    pub(crate) fleet_ready: Arc<parking_lot::Mutex<Vec<crate::world::scene::VehicleKey>>>,
    /// Time of day of the last look at the next departures' vehicles.
    pub(crate) fleet_check: f64,
    /// Per trip and profile: when its bus is at its stations.
    pub(crate) times: Vec<Vec<TripTimes>>,
    /// Per bus stop (map object id): the trips that call there, as (trip, station index).
    pub(crate) visits: HashMap<i64, Vec<(usize, usize)>>,
    /// Per trip: today's departures that run it.
    pub(crate) trip_departures: Vec<Vec<usize>>,
    /// When the departure boards were last made (time of day).
    pub(crate) boards_made: f64,
    /// The tour the player drives (line, tour): the timetable does not run it as well.
    pub(crate) player_tour: Option<(String, String)>,
    /// With a single trip picked: the departure (s of the day) of that trip; the rest of
    /// the tour stays the AI's.
    pub(crate) player_departure: Option<f64>,
    /// A player tour was just taken over: its buses already on the road go at the next tick.
    pub(crate) purge_player_tour: bool,
    /// LAN play (host): the tours the other players drive (line, tour; lower case), left to
    /// them like our own.
    pub(crate) lan_tours: HashSet<(String, String)>,
    /// Time of today's timetable at the last tick (s).
    pub(crate) last_tod: f64,
    /// The lines the date's chrono folders take off the timetable, with the folder that does
    /// it: why a duty on such a line cannot be driven.
    pub(crate) deactivated: Vec<(String, std::path::PathBuf)>,
    /// Stations some trip stops at on its way or ends at (not only starts from).
    pub(crate) served: std::collections::HashSet<i64>,
    /// Per first station: whether another trip's bus stops there or within a bus length
    /// or two of it (maps often put one stop object per line at the same kerb), once its
    /// position is known.
    pub(crate) shared_stand: HashMap<i64, bool>,
    /// Departures queued while the map loads: their buses may appear in view.
    pub(crate) startup: std::collections::HashSet<usize>,
    /// Layover departures whose stand was taken: they come at their departure time.
    pub(crate) later_layover: std::collections::HashSet<usize>,
    /// Per departure: the next departure of the same tour (its bus takes it on).
    pub(crate) tour_next: Vec<Option<usize>>,
    /// Departures due while their tour's bus is still on its previous trip: that bus takes
    /// them on when it gets there, as in OMSI a tour keeps its bus from trip to trip.
    pub(crate) awaiting: std::collections::HashSet<usize>,
    /// The clock was set (`restart`): the next tick puts the buses out as a start does.
    pub(crate) restarted: bool,
    /// The map's holidays, for the day's tours.
    pub(crate) calendar: omsi_map::Calendar,
    /// The date (yyyymmdd) the departures are for, and the mask bits it selects (day,
    /// school); `set_day` moves them on at midnight.
    pub(crate) day: i32,
    pub(crate) day_bits: (i32, i32),
    /// The weekday bit of the next day (night tours run on into it).
    pub(crate) next_day_bit: i32,
    /// Where today's midnight lies on the traffic's clock (`Traffic::day_time` counts on past
    /// 24:00): a departure leaves at `day_base + time` (`dep_time`), and the date moves on
    /// when the clock passes the next midnight.
    pub(crate) day_base: f64,
    /// Departures put out as the previous service day's (a night tour's trips at 24:00 or
    /// later, once the clock has passed midnight): where that day's midnight lay on the
    /// traffic's clock. `dep_time` uses it instead of `day_base` while the departure stays
    /// spawned.
    pub(crate) carried_base: HashMap<usize, f64>,
    /// The current date (its time of day is not used).
    pub(crate) date_clock: omsi_sim::SimClock,
    /// The map's `car_use/*.ocu`: which vehicles serve which line's tours.
    pub(crate) car_use: Vec<omsi_timetable::CarUse>,
    /// Per tour (`tour_key_of`): the depot vehicle (index in its group) and fleet number
    /// it runs with today - from `car_use`, else drawn the first time the tour is due. A
    /// number is given to one tour only (`used_numbers`), as in OMSI:
    /// hashing each tour to a number put the same fleet number on two buses at once.
    pub(crate) tour_vehicle: HashMap<u64, (usize, usize)>,
    pub(crate) used_numbers: HashSet<(String, String)>,
}

/// A destination picked by hand for a hand-cranked roller blind: its row of the depot file
/// and the route number for the number rollers (`SetLineTo`).
#[derive(Debug, Clone, PartialEq)]
pub struct BlindPick {
    pub row: usize,
    pub line: String,
}

/// The key presses a driver makes on the IBIS for a trip.
#[derive(Debug, Clone, PartialEq)]
pub struct IbisCodes {
    /// Complete depot line code: numeric line × 100 plus the letter suffix
    /// (the IBIS reads the last two digits as a suffix; 0 = none).
    pub line: Option<u32>,
    /// Route entry (the last two digits of the depot file's route code).
    pub route: Option<u32>,
    /// Destination entry, when there is no route to type.
    pub terminus: Option<u32>,
}

/// One stop of a planned trip with its scheduled times (seconds since midnight).
#[derive(Debug, Clone)]
pub struct PlannedStop {
    pub object_id: i64,
    pub name: String,
    pub arr: f64,
    pub dep: f64,
    pub position: Option<glam::DVec3>,
    /// Which way the trip runs through the stop ([`StopDir`]): a circular route, or one
    /// that turns back, calls at the same place twice and the two stops of it stand a few
    /// metres apart. Only the direction says which of them a bus has reached (#254).
    pub dir: StopDir,
    /// The bus stops here (a depot run passes its stations).
    pub stops: bool,
}

/// Which way a trip runs through one of its stops: the direction it arrives on and the one
/// it leaves on, as unit vectors of the ground plane (x east, y north). None where a
/// neighbour's place is unknown or too near to tell a direction - any heading will do then.
#[derive(Debug, Clone, Copy, Default)]
pub struct StopDir {
    pub inbound: Option<glam::DVec2>,
    pub outbound: Option<glam::DVec2>,
}

#[derive(Debug, Clone)]
pub struct PlannedTrip {
    pub name: String,
    pub line: String,
    pub terminus: String,
    pub departure: f64,
    /// Arrival at the last station.
    pub end: f64,
    pub stops: Vec<PlannedStop>,
}

/// The player's tour: its trips with planned stop times, and the progress along them.
pub struct PlayerDuty {
    pub line: String,
    pub tour: String,
    pub trips: Vec<PlannedTrip>,
    pub trip_index: usize,
    /// Where `trips` begins in the tour: a picked trip is a duty of its own, and a saved
    /// situation counts the trip under way from the tour's first.
    pub first_trip: usize,
    /// Next stop to serve on the current trip.
    pub next_stop: usize,
    /// True while the bus stands at the next stop.
    pub at_stop: bool,
    /// How late the bus arrived at the stop it stands at (s after its arrival time).
    pub arrived_late: Option<f64>,
    /// The bus has reached the last stop of the current trip.
    pub done: bool,
    /// The last stop where this trip actually stopped with a passenger door open.
    pub served_terminus: Option<glam::DVec3>,
    /// How late (s, negative = early) the bus left the last stop it served on this trip;
    /// None while it has not left one.
    pub left_late: Option<f64>,
    /// A page moved the duty back to an earlier stop: `catch_up` must not jump forward
    /// again to a later stop the bus still stands at, until the bus reaches a stop again.
    pub held_back: bool,
    /// A page or the menu moved the duty to another stop since the bus last left one: the
    /// stop before `next_stop` is not the one the bus came from, so the delay on the way is
    /// not worked out between the two.
    pub jumped: bool,
    /// The first update looks where the bus stands.
    pub placed: bool,
    /// The current trip changed since the last `take_trip_change`.
    pub trip_changed: bool,
    /// Stops the bus passed without stopping since the last `take_skipped` (see `catch_up`).
    pub skipped: Option<(usize, usize, usize)>,
    /// The player picked the current trip: the duty does not move on past it before it is
    /// driven (or given up), however late the bus is for it.
    pub picked: bool,
    /// Time of day of the first update (placing waits a little for the places of stops
    /// beyond the loaded tiles, see `learn_places`).
    pub first_update: Option<f64>,
    /// The way the bus faces (degrees clockwise from north), from the last update: it says
    /// which of two stops a few metres apart the bus is at (see `StopDir`).
    pub heading: f64,
    /// Where the bus was at the last update: how far along the way to the next stop it is,
    /// for the delay on the way (`delay`).
    pub position: Option<glam::DVec3>,
    /// The stops the driver skipped, and the ones marked to skip when they come.
    pub skips: Skips,
}

/// The stops the driver skips on purpose (the Alt bar's timetable window, the game menu's
/// "Skip the next stop", Ctrl+Shift+H): a deliberate improvement, as the original has no way
/// to say a stop was left out for a reason. A skipped stop counts with no mark in the
/// evaluation: neither missed nor late.
#[derive(Clone, Debug, Default)]
pub struct Skips {
    /// Given up: (the trip's place in the duty, the stop's place in the trip).
    pub(crate) done: Vec<(usize, usize)>,
    /// To be given up when the duty is due at them.
    pub(crate) marked: Vec<(usize, usize)>,
}

/// Where a timetable bus on the road is in its trip, for the departure boards.
pub(crate) struct OnRoad {
    /// Timetable departure time of its next stop (None: it has served its last).
    pub(crate) next: Option<f64>,
    /// Standing at that stop.
    pub(crate) dwelling: bool,
    /// How late it left its last stop (s).
    pub(crate) late: f64,
}
