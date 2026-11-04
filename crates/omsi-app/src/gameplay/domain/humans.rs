//! The types of the people on foot: the passengers and pedestrians as agents with a goal,
//! their crowds, their models and what they ride. Their procedures are in
//! `implementation/humans.rs`, and the passengers' tasks in `implementation/humans/pax.rs`.

use crate::weather::ambience;
use glam::{DVec2, DVec3, Mat4, Vec3};
use hashbrown::{HashMap, HashSet};
use omsi_render::{MaterialId, MeshId};
use omsi_sim::people::crowd::{Block, PathGraph};
use omsi_sim::people::omsi_anim::OmsiAnim;
use omsi_sim::people::{Activity, HumanType};
use omsi_vehicle::PassengerCabin;
use std::path::PathBuf;
use std::sync::Arc;

pub(crate) mod pax;
use pax::*;

/// Where the player looks from, for "nobody appears or vanishes in sight".
#[derive(Debug, Clone, Copy)]
pub struct Eye {
    pub pos: DVec3,
    pub fwd: DVec3,
    /// Cosine of half the diagonal field of view, with a margin.
    pub cos_half: f64,
}

/// What the passengers tell a bus's scripts in a frame: about its doors, entry by entry and
/// exit by exit, and its places (see [`Humans::write_door_requests`]).
#[derive(Debug, Clone, Default)]
pub struct DoorWants {
    /// `PAX_Entry<i>_Req` / `PAX_Exit<i>_Req`: somebody wants in / out there.
    pub entry_req: Vec<bool>,
    pub exit_req: Vec<bool>,
    /// `PAX_Entry<i>_Busy` / `PAX_Exit<i>_Busy`: somebody stands in that doorway.
    pub entry_busy: Vec<bool>,
    pub exit_busy: Vec<bool>,
    /// The occupancy variables the `[passpos]` places name (#721): whether somebody is on
    /// each.
    pub places: Vec<(String, bool)>,
}

/// A bus as the passengers know it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BusId {
    Player,
    Ai(u64),
}

/// An `[entry]` or `[exit]` of a cabin, in the bus frame.
#[derive(Debug, Clone)]
pub struct Door {
    /// The door's path point (the threshold) and its index.
    pub inside: Vec3,
    pub point: Option<usize>,
    /// Where somebody stands just outside, at ground level.
    pub(crate) outside: Vec3,
    /// +1 on the right side of the bus, -1 on the left.
    pub(crate) side: f32,
    /// Direction along the bus (+1 forwards) in which the queue at this door runs.
    pub(crate) queue_dir: f32,
    /// A passenger who still has to buy a ticket may board here (no `{noticketsale}`).
    pub(crate) sells: bool,
    /// `{withbutton}`: a door the passenger opens with the request button, worth walking to
    /// while it is still shut.
    pub(crate) button: bool,
    /// Where people getting off wait for the door to open: the path point next to it.
    pub wait: Vec3,
    /// The sections it belongs to that people walk between (see `Cabin::groups`).
    pub group: usize,
}

#[derive(Debug, Clone)]
pub struct Seat {
    /// The `[passpos]` point: a seated passenger's hip, a standing one's feet.
    pub pos: Vec3,
    /// The floor in front of it, where the feet go (and where a seated passenger stands
    /// before sitting down and after getting up).
    pub floor: Vec3,
    pub rot: f32,
    pub seated: bool,
    /// The `[passpos]`'s seat height (0: a standing place).
    pub height: f32,
    /// Its number for the scripts (`GetHumanCountOnSeat`): OMSI's place in the file
    /// among the `[passpos]` and `[drivpos]`, the sections behind counted on after those
    /// in front (each section asks the next one for a number past its own places).
    pub omsi_seat: usize,
    /// openOMSI's variables of the place (#721): the one that switches it on and off, and the
    /// one its occupancy is written into.
    pub switch_var: Option<String>,
    pub taken_var: Option<String>,
    /// The sections it lies in that people walk between (see `Cabin::groups`).
    pub group: usize,
}

/// What passengers need to know about one vehicle type's cabin.
pub struct Cabin {
    pub(crate) data: PassengerCabin,
    pub graph: PathGraph,
    pub links: Vec<(i32, i32, bool)>,
    /// Each link's footstep sounds: its section's `[stepsoundpack]` named by the link's
    /// `[next_stepsound]` (index into `step_packs`), none where the paths.cfg gives none -
    /// OMSI hears no steps there - and on the joint between two sections.
    pub link_pack: Vec<Option<usize>>,
    pub step_packs: Vec<Arc<[String]>>,
    pub entries: Vec<Door>,
    pub exits: Vec<Door>,
    /// Where a passenger stands at the cash desk, its path point, and the heading (bus
    /// frame) they face: between the desk top, where the money goes, and the driver.
    pub(crate) desk: Option<(Vec3, Option<usize>, f64)>,
    pub seats: Vec<Seat>,
    /// The sections (one for a rigid bus), front first; everything above is in the
    /// unfolded frame of the front section.
    pub parts: Vec<CabinPart>,
    /// How many groups of sections people walk between (one but where a trailer hangs on
    /// that nobody walks into from the bus, #718), and each path point's group: a passenger
    /// gets in, rides and gets out within one.
    pub groups: usize,
    pub(crate) point_group: Vec<usize>,
    /// Each link's room height (`[next_roomheight]`; 2 m before any).
    pub(crate) link_room: Vec<f32>,
    /// The routing tables of the path network.
    pub(crate) routes: Vec<Vec<RouteLink>>,
    /// The validators and the cash desk: (path point, device). OMSI keeps one of each,
    /// the last of the file; every `[stamper]` of every
    /// section is kept here, and a passenger stamps at the one nearest the door they came
    /// in by (`Cabin::nearest_stamper`, #722) - with one, that one.
    pub stampers: Vec<(Option<usize>, Vec3)>,
    pub(crate) sale: Option<(Option<usize>, Vec3)>,
    /// Where the money goes and where the change is taken from, with the
    /// money point's spread.
    pub(crate) money_point: Option<Vec3>,
    pub(crate) money_var: Option<(Vec3, [f32; 2], Option<String>)>,
    pub(crate) change_point: Option<Vec3>,
}

/// A section of an articulated bus in its cabin's unfolded frame.
#[derive(Debug, Clone, Copy)]
pub struct CabinPart {
    /// Where the section's own origin lies.
    pub(crate) offset: Vec3,
    /// The unfolded y of the joint in front of it (the front section: none, +inf).
    pub(crate) joint_y: f32,
}

/// One vehicle of a coupled train as a cabin is put together from it: its definition, its
/// origin in the front vehicle's unfolded frame, and the unfolded y of its front joint.
pub(crate) type TrainPart<'a> = (&'a omsi_vehicle::Vehicle, Vec3, f32);

/// A bus as the passengers see it this frame.
#[derive(Clone)]
pub struct BusNow {
    pub id: BusId,
    pub next_stop: Option<RequestStop>,
    pub cabin: Arc<Cabin>,
    pub pos: DVec3,
    pub rot: Mat4,
    pub heading: f64,
    /// m/s, forwards.
    pub speed: f64,
    pub entry_open: Vec<bool>,
    pub exit_open: Vec<bool>,
    /// The doors a walker may use (another player's bus: its doors as they are, while
    /// `entry_open` stays shut for the passengers here); None: as `entry_open`/`exit_open`.
    pub walk_open: Option<(Vec<bool>, Vec<bool>)>,
    pub interior: f32,
    /// The saloon's air and the light outside, for what boarding passengers say.
    pub air: CabinAir,
    /// Half extents across / along and the centre of its bounding box (bus frame).
    pub half: DVec2,
    pub centre: DVec2,
    /// Acceleration of the floor (bus frame: x to the right, y forwards; m/s²).
    pub accel: DVec2,
    /// The sections behind the front one (the cabin's parts after the first).
    pub trailers: Vec<PartFrame>,
    /// The terminus it shows, by name (OMSI's bus terminus). None: "$allexit$" - the
    /// scripts' `target_index_int` names a hof terminus added with `[addterminus_allexit]`
    /// ("Nicht einsteigen", a works trip) - or none; no timetable target has it.
    pub terminus: Option<String>,
    /// Whom it takes on at the stops.
    pub takes: Takes,
    /// The places its scripts have switched off (see `places_off`; empty: none).
    pub places_off: Vec<bool>,
    /// A timetable bus boarding at a stop the passengers' nearby stops do not have (its
    /// tile is not among theirs): the stop its timetable serves, so that its riders still
    /// get off there.
    pub served: Option<i64>,
}

/// What passengers feel stepping into a bus: the vehicle's `Cabinair_Temp`,
/// `Cabinair_relHum` and `AI_Interiorlight`, and the weather's temperature.
#[derive(Debug, Clone, Copy, Default)]
pub struct CabinAir {
    /// °C, when the bus keeps its cabin air (every bus does: its script or the engine).
    pub temp: Option<f32>,
    /// Relative humidity, a fraction.
    pub rel_hum: f32,
    /// The temperature outside (°C).
    pub outside: f32,
    /// `AI_Interiorlight`: whether the saloon is lit (0 .. 1). The bus scripts write it
    /// for the player's bus (the engine only for the buses the AI drives); a script that
    /// does not leaves it at 0, a dark saloon.
    pub saloon_lit: f32,
}

/// Where a rear section of a bus is this frame, with its place in the cabin.
#[derive(Debug, Clone, Copy)]
pub struct PartFrame {
    pub pos: DVec3,
    pub rot: Mat4,
    pub heading: f64,
    pub offset: Vec3,
    pub joint_y: f32,
    /// Half extents across / along and the centre of its bounding box (own frame).
    pub half: DVec2,
    pub centre: DVec2,
}

/// One piece of a walk along a pavement path: lane `lane` from distance `a` to `b`.
#[derive(Debug, Clone, Copy)]
pub struct Leg {
    pub lane: usize,
    pub a: f32,
    pub b: f32,
}

/// A walk along the pavement network.
#[derive(Debug, Clone)]
pub struct PedWalk {
    pub(crate) legs: Vec<Leg>,
    pub(crate) leg: usize,
    /// Metres walked into the current leg.
    pub(crate) s: f32,
    /// A stroll: goes on at random when the legs run out.
    pub(crate) roam: bool,
    /// Keep-right offset (m).
    pub(crate) side: f32,
    /// Seconds spent waiting at the kerb before the current leg.
    pub(crate) held: f32,
}

/// The pavement paths as a walking network: path ends closer than a metre are one
/// junction, whatever their heading (the road network joins lane ends only when they
/// continue in the same direction, which leaves every pavement corner open).
pub struct PedNet {
    /// Per pavement lane: its start and end junction.
    pub(crate) ends: HashMap<usize, (usize, usize)>,
    /// Per junction: (lane, walked forwards) leaving it.
    pub out: Vec<Vec<(usize, bool)>>,
    /// Where each pavement lane crosses a carriageway (lazily), and the carriageway lanes
    /// it crosses.
    pub(crate) crossings: HashMap<usize, Vec<DVec2>>,
    pub(crate) crossed: HashMap<usize, Vec<usize>>,
    /// Pavement lanes by 50 m cell.
    pub(crate) grid: HashMap<(i32, i32), Vec<usize>>,
    /// The junctions and a 1.5 m grid of them, for joining the paths of tiles loaded later.
    pub(crate) nodes: Vec<DVec3>,
    pub(crate) cells: HashMap<(i64, i64), Vec<usize>>,
    /// How many lanes of the traffic network are in (the network only grows: tiles bring
    /// their lanes and the indices stay).
    pub(crate) built: usize,
}

#[derive(Debug, Clone)]
pub enum State {
    /// A pedestrian strolling the pavements (OMSI's task 8, `WalkStreet`).
    Strolling(PedWalk),
    /// Moved by somebody else: an avatar, or one of a LAN host's people.
    Idle,
    /// Task 8 without a path: somebody who got off where no pavement is
    /// stands where they are until the player is gone.
    Standing,
    /// A passenger (see `humans_pax`).
    Pax(Box<Pax>),
}

/// Where a person is: on the ground, or inside a bus at a point of its frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Place {
    Ground,
    Bus(BusId, Vec3),
}

pub struct Person {
    pub id: u32,
    pub ty: Arc<HumanType>,
    /// Clothing variant (`HumanType::variant_texture`).
    pub variant: usize,
    pub meshes: Vec<(MeshId, usize)>,
    pub position: DVec3,
    pub heading: f64,
    /// Heading in the bus frame while inside one.
    pub lheading: f64,
    pub place: Place,
    /// Velocity in the plane the person walks in (ground or bus floor).
    pub vel: DVec2,
    pub pace: f64,
    pub activity: Activity,
    /// The animation: OMSI's walk phase and joint angles.
    pub anim: OmsiAnim,
    pub state: State,
    /// Seconds in the current state.
    pub t_state: f32,
    /// Skinned positions and normals, per mesh.
    pub skins: Vec<(Vec<Vec3>, Vec<Vec3>)>,
    /// The bones the skins were made with, and whether this frame's pose changed them
    /// (somebody standing still keeps the mesh of the frame before: skinning and uploading
    /// thirty waiting people every frame took 2 ms of the frame at a bus station).
    pub skin_bones: Option<[glam::Affine3A; omsi_sim::people::SLOTS]>,
    pub pose_changed: bool,
    /// Interior light of the bus the person is in (0 outside).
    pub interior: f32,
    /// The interior light as drawn: it follows `interior` over a moment (stepping through
    /// the door, people lit up and went dark again from one frame to the next).
    pub lit: f32,
    /// The tilt of the floor the person stands on (a bus pitching under the brakes and
    /// leaning in a bend), without its heading: riders are drawn with it. Upright on the
    /// ground; drawn upright in a tilted bus, their feet sank through the floor on one side.
    pub tilt: Mat4,
    /// Age in years: the `.hum`'s `[age]`, else 40 as in OMSI. The
    /// ticket pack's tickets have age ranges (the reduced fare is for 6..13).
    pub age: f32,
    /// Seconds without getting nearer the goal while wanting to move; seconds left
    /// passing through others.
    pub stuck: f32,
    pub ghost: f32,
    /// Seconds a standing vehicle has stood in the way (see the crowd step).
    pub car_wait: f32,
    /// Seconds left going round something in the way off the pavement's line (a lamp post
    /// on the path): the corridor does not pull them back into it meanwhile.
    pub detour: f32,
    /// Which way round (+1 anticlockwise, -1 clockwise) while `detour` lasts: round a corner
    /// the sides' own choices flipped each other and people shuffled at a post.
    pub detour_side: f64,
    /// Why the person is standing, for `OMSI_DEBUG_PAX`.
    pub why: &'static str,
    /// Whether this person has ever been posed (an unposed model is the file's T-pose).
    pub skinned: bool,
    /// Frames since the last pose and where the person stood then (the
    /// feet of a mesh posed a frame ago stay on the floor when it is drawn there).
    pub since_posed: u32,
    pub posed_at: (DVec3, f64),
    /// Ankles of the last pose (model frame), for `OMSI_TRACE_PAX`.
    pub ankles: [Vec3; 2],
    /// A scripted test person (`OMSI_PAX_GALLERY`).
    pub puppet: Option<Puppet>,
    /// LAN play: one of the host's people, drawn where the host says (`mirror_set`).
    pub remote: bool,
}

/// What a person wants this frame.
pub(crate) struct Want {
    pub(crate) vel: DVec2,
    /// Heading to turn to when standing (world on the ground, bus frame inside).
    pub(crate) face: Option<f64>,
    pub(crate) give: f64,
    pub(crate) corridor: Option<(DVec2, DVec2, f64)>,
    /// What they do when not walking.
    pub(crate) idle: Activity,
}

pub struct Humans {
    pub(crate) types: Vec<Arc<HumanType>>,
    pub people: Vec<Person>,
    pub(crate) rng: u64,
    pub(crate) next_id: u32,
    /// Seconds since the start.
    pub(crate) time: f64,
    pub(crate) wall_cells: HashMap<(i32, i32, i32), Vec<(Block, f64, f64)>>,
    pub(crate) wall_key: (usize, usize, usize, f64),
    /// Passenger cabins by vehicle files (the front vehicle and its coupled parts).
    pub(crate) cabins: HashMap<Vec<PathBuf>, Option<Arc<Cabin>>>,
    pub(crate) player_cabin: Option<Arc<Cabin>>,
    pub player_next_stop: Option<RequestStop>,
    /// Which places of each bus are taken.
    pub(crate) seats: HashMap<BusId, Vec<bool>>,
    /// The bus stops as OMSI keeps them for the people (see `humans_pax`).
    pub stops: HashMap<i64, PaxStop>,
    /// Kilometres each bus has driven (the odometer the riders read).
    pub(crate) odometer: HashMap<BusId, f64>,
    /// The `PAX_Entry<n>_Req` / `PAX_Exit<n>_Req` of each bus this frame.
    pub(crate) pax_req: HashMap<BusId, (Vec<bool>, Vec<bool>)>,
    /// Its `PAX_Entry<n>_Busy` / `PAX_Exit<n>_Busy`: somebody in that doorway.
    pub(crate) pax_busy: HashMap<BusId, (Vec<bool>, Vec<bool>)>,
    /// The occupancy variables its places name, and whether somebody is on each (#721).
    pub(crate) pax_places: HashMap<BusId, Vec<(String, bool)>>,
    /// Who is at the player's cash desk, how often the driver has been asked
    /// again and the most of that in this sale.
    pub(crate) desk_busy: Option<u32>,
    pub(crate) pardons: u8,
    pub(crate) pardon_max: u8,
    /// The first populate put people at the stops; later stops fill on foot when in sight.
    pub(crate) started: bool,
    pub(crate) ped: Option<PedNet>,
    pub(crate) hidden: Vec<usize>,
    /// GPU side of the human types, shared by everyone of a type: textures by file and the
    /// materials of every (type, mesh) - each person used to upload its own copies - and
    /// the meshes and instances of the people who have gone, taken over by the next person
    /// of the same type (the skinned vertices are rewritten anyway). Without that every
    /// passenger who ever appeared kept a mesh, its textures and materials on the GPU.
    pub(crate) gpu_textures: HashMap<PathBuf, Option<omsi_render::TextureId>>,
    /// Per (type, clothing variant, mesh): its materials, and the meshes and instances of
    /// people who have gone, kept for the next person dressed alike.
    pub(crate) gpu_materials: HashMap<(usize, usize, usize), Vec<MaterialId>>,
    pub(crate) spare: HashMap<(usize, usize, usize), Vec<(MeshId, usize)>>,
    /// Stop the player's bus is serving (standing at it).
    pub(crate) served_stop: Option<i64>,
    /// Timetable buses at a stop: id → (stop, time the visit began).
    pub(crate) ai_visits: HashMap<u64, (i64, f64)>,
    /// When each bus last had a door open (the passengers' clock).
    pub(crate) last_door_open: HashMap<BusId, f64>,
    /// Timetable buses to keep at their stop for a few seconds more (for the traffic): the
    /// bus, the stop it must be serving for it (none: any), the seconds.
    pub(crate) holds: Vec<(u64, Option<i64>, f32)>,
    /// Door requests for the timetable buses' scripts.
    pub(crate) ai_requests: Vec<(u64, DoorWants)>,
    pub tickets: Option<Arc<omsi_content::tickets::TicketPack>>,
    /// Current ticket request at the player's cash desk: (ticket name, value).
    pub request: Option<(String, f32)>,
    /// Payment on the desk: (paid, ticket value), and the change still owed after the ticket.
    pub paid: Option<(f32, f32)>,
    pub change_due: Option<f32>,
    pub money: Option<crate::money::Money>,
    /// The people inside the player's bus's box last frame (`run_over`): knocked down once
    /// when they come into it, not again every frame they are in it.
    pub(crate) under_bus: hashbrown::HashSet<u32>,
    /// The tear-off ticket blocks of the player's bus (`money::TicketBlocks`).
    pub ticket_blocks: Option<crate::money::TicketBlocks>,
    /// A rider pressed the stop button for the next stop (the app fires the vehicle trigger `int_haltewunsch`).
    pub stop_request: bool,
    /// Buses the player asked to empty (`force_disembark`).
    pub empty_bus: hashbrown::HashSet<BusId>,
    /// Tickets sold at the cash desk this session and what they were worth.
    pub tickets_sold: u32,
    pub ticket_cash: f32,
    /// Passengers that reached the cash desk, and those the driver served there.
    pub boarded: u32,
    pub served: u32,
    /// OMSI's rating counters: people who stepped into the player's bus
    /// and of those who had nothing to complain about (comfort = content / stepped in);
    /// tickets asked for and the points for selling them, two for the right change, one
    /// for the wrong (ticket selling = points / 2 × asked).
    pub stepped_in: u32,
    pub content: u32,
    pub ticket_requests: u32,
    pub ticket_points: u32,
    /// `PAX_Entry<i>_Req`: somebody at the kerb wants in through entry `i`.
    pub entry_req: Vec<bool>,
    /// `PAX_Exit<i>_Req`: somebody inside wants out through exit `i`.
    pub exit_req: Vec<bool>,
    /// `PAX_Entry<i>_Busy` / `PAX_Exit<i>_Busy`: somebody stands in that doorway.
    pub entry_busy: Vec<bool>,
    pub exit_busy: Vec<bool>,
    pub(crate) sync_frame: u32,
    /// Feet put down since the app last collected them (see [`Humans::take_footfalls`]).
    pub(crate) footfalls: Vec<ambience::Footfall>,
    /// `[trafficdensity_passenger]` factor for the current hour (set by the app).
    pub density: f32,
    /// The same curve without the passengers factor: how many people walk the pavements
    /// (the factor is for those waiting at stops only).
    pub people_curve: f32,
    /// The clock's time of day in seconds (set by the app): day tickets sell by it.
    pub time_of_day: f64,
    /// The daylight outside, 0 dark .. 1 (the sky's light, without the street lamps'
    /// light maps; set by the app): boarding passengers may find the bus too dark.
    pub daylight: f32,
    /// How late the player's bus is on its duty (s; set by the app): over five minutes,
    /// boarding passengers may say so.
    pub delay: f64,
    /// The game's folder (the ticket pack's voices are found from it).
    pub(crate) root: std::path::PathBuf,
    /// What passengers said since the app last collected it (see `take_voice_lines`).
    pub(crate) voice_lines: Vec<VoiceLine>,
    /// When each voice file was last said (seconds of `time`): OMSI keeps such a list
    /// and says a greeting or a complaint only when that
    /// very file has not been heard for 10 s - without it every boarding passenger said
    /// "Hallo" one after the other.
    pub(crate) voice_said: HashMap<std::path::PathBuf, f64>,
    /// What passengers may say (the `pax_voices` setting): 0 everything, 1 only the
    /// ticket they ask for, 2 nothing.
    pub voices: u8,
    /// When anybody last greeted or complained (seconds of `time`).
    pub(crate) last_chat: f64,
    /// Avatars (the player on foot, other players' walkers): key → person id, and what
    /// the game wants of each this frame.
    pub(crate) avatars: HashMap<u32, u32>,
    pub(crate) avatar_cmds: HashMap<u32, AvatarCmd>,
    /// Avatars not drawn (the first-person view), by person id.
    pub(crate) avatar_hidden: HashMap<u32, bool>,
    /// The buses of the last tick (for the avatars' seats and doors).
    pub(crate) last_buses: Vec<BusNow>,
    /// Only avatars: nobody else is put on the map (the passengers are off).
    pub avatar_only: bool,
    /// The player has got up and left the wheel: a standing bus with a door open is left
    /// by its riders as at a terminus (see `ALL_OUT_STOP`).
    pub driver_away: bool,
    /// Per bus stop, OMSI's station targets (`Schedule::stop_targets`): the
    /// stops the trips go on to, each with the termini of those trips. A person waiting there
    /// wants one of them and boards only a bus showing one of its termini; at a stop no trip
    /// goes on from, anybody takes the first bus.
    pub stop_targets: Option<HashMap<i64, Vec<(String, HashSet<String>)>>>,
    /// Per bus stop, the destinations of the trips due there soon (`Schedule::
    /// due_destinations`, made anew every game minute): the people turning up draw theirs
    /// from these alone. None: from all of the stop's (no timetable).
    pub due_dests: Option<HashMap<i64, HashSet<String>>>,
    /// Game time `due_dests` was made at.
    pub due_at: f64,
    /// The timetable's name of each stop object (`Schedule::stop_names`), the names the
    /// targets above are made of.
    pub stop_names: Option<HashMap<i64, String>>,
    /// The player's duty (`set_duty`): its trip, the stop of it the duty is due at, and
    /// whether the trip has reached its last stop. None: free drive, and nobody waiting
    /// boards the player's bus.
    pub(crate) duty: Option<(Arc<DutyTrip>, usize, bool)>,
    /// Buses whose validator somebody used since the app last looked (`take_stamped`).
    pub(crate) stamped: Vec<BusId>,
    /// Pedestrians to keep strolling near the player (scaled by `density`).
    pub pedestrians: usize,
    /// OMSI's people (`[AIMaxCountRandom]`'s second line, the `ai_max_humans` setting):
    /// it makes that many at the start and draws everybody waiting at a stop,
    /// walking the pavements or riding from them - never more; at most half of them walk
    /// the pavements. Here people are made as they are wanted, so they are
    /// counted against it instead.
    pub max_people: usize,
    pub(crate) stroll_timer: f32,
    /// Passengers pay the exact fare: no change is ever due.
    pub exact_fare: bool,
    /// How passengers board (`boarding` in the settings): `auto` - pay and take the
    /// ticket by themselves after a moment; `pay` - wait at the desk for the driver to
    /// sell it (and show a pass after `PAY_PATIENCE`); `walk` - no cash desk at all.
    pub boarding: String,
    /// The driver pressed the ticket key (`ticket_give`): sell the requested ticket.
    pub give_ticket: bool,
    /// The driver pressed `change_give`: all the change owed goes on the tray at once.
    pub give_change_all: bool,
    /// The key that sells a ticket, as the HUD names it.
    pub ticket_key: String,
    /// Where the player looks from (set by the app every frame).
    pub eye: Option<Eye>,
    /// Around where people are kept (the player's bus, else the camera).
    pub(crate) center: DVec3,
    /// A line for the HUD about something that just happened.
    pub(crate) message: Option<String>,
    /// Frames ticked, total and longest tick (ms).
    pub(crate) tick_stats: (u32, f64, f64),
    /// Where the time of this tick went (stage, ms since the one before), for the slow
    /// ticks OMSI_PROFILE reports.
    pub(crate) tick_stages: Vec<(&'static str, f64)>,
    /// Speed, heading and floor acceleration of each bus last frame (for the riders' balance).
    pub(crate) bus_motion: HashMap<BusId, (f64, f64, DVec2)>,
    /// `types` has been cut down to the map's `humans.txt`.
    pub(crate) map_humans_done: bool,
    /// Simulation time of the last `sync`.
    pub(crate) last_sync: f64,
    /// Frames synced, people posed and skinned, the time that took and the part of it spent
    /// uploading (ms), in total.
    pub(crate) pose_stats: (u32, usize, f64, f64),
    /// `OMSI_TRACE_PAX=<csv>`: every person near the eye, every frame (see `sync`).
    pub(crate) trace: Option<std::io::BufWriter<std::fs::File>>,
    /// `World::tiles_generation` the stops were last checked against.
    pub(crate) tiles_seen: u64,
    /// LAN play: this game draws the host's people instead of its own (`lan_world`).
    pub(crate) mirror: bool,
    /// LAN play: where the other players are (host): people are kept around them too.
    pub lan_centers: Vec<DVec3>,
    /// LAN play: the other players' buses this frame (`set_remote_buses`), for their riders
    /// to sit in. Nobody of ours boards them: their doors count as shut.
    pub(crate) remote_now: Vec<BusNow>,
    /// The vehicles the player placed and is not driving now (`placed_bus_id`): their
    /// riders stay in them when the player drives another.
    pub(crate) placed_now: Vec<BusNow>,
    /// LAN play (client): waiting people our bus could take, to ask the host for, and when
    /// each was last asked for.
    pub(crate) claims_out: Vec<u32>,
    pub(crate) claimed: HashMap<u32, f64>,
    /// The host's people waiting at a stop (client): (stop, waiting place).
    pub(crate) mirror_wait: HashMap<u32, (i64, usize)>,
    /// How the player's bus is driven, for its riders' complaints.
    pub(crate) comfort: RideComfort,
    /// LAN play (host): the waiting people handed over to another player's bus, by stop and
    /// that bus. They count among the people of the stop while the bus stands there, as
    /// the people who board a bus of ours keep their stop until it has left: without them
    /// the stop filled up again at once - one more person a frame once its 10..15 s were
    /// up - and the client's bus took them all, one stream of passengers that never ended
    /// (#842, #840, #830).
    pub(crate) handed: Vec<(i64, u64)>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PuppetMode {
    /// The player on foot (or another player's walker): moved by the game, see `avatar`.
    Avatar,
}

/// A person the game moves itself (the player on foot).
#[derive(Debug, Clone, Copy)]
pub struct Puppet {
    pub mode: PuppetMode,
}

/// What the game wants of an avatar this frame.
#[derive(Debug, Clone, Copy)]
pub struct AvatarCmd {
    /// The feet (on foot), in the world.
    pub pos: DVec3,
    /// Facing (degrees, OMSI's).
    pub heading: f64,
    /// Velocity over the ground (m/s).
    pub vel: DVec2,
    /// How high the feet are over the ground (a jump).
    pub lift: f64,
    /// Sitting on this seat of this bus.
    pub seat: Option<(BusId, usize)>,
    /// Standing on a vehicle's floor at this height rather than on the ground (walking
    /// inside a bus: the feet stay on its floor, not reaching down to the road).
    pub floor: Option<f64>,
    /// Standing or walking inside this bus at this point of its cabin (bus frame): placed
    /// in the bus's frame as it is this frame, as its passengers are (a world point taken
    /// a frame earlier left the figure trembling behind the moving bus).
    pub aboard: Option<(BusId, Vec3)>,
}

/// A seat an avatar may take: which, in which bus.
#[derive(Debug, Clone, Copy)]
pub struct SeatSpot {
    pub bus: BusId,
    pub seat: usize,
}

/// A heading in 0..360 degrees.
/// A line a passenger says: the sample and where they stand.
pub struct VoiceLine {
    pub position: DVec3,
    pub path: std::path::PathBuf,
}
