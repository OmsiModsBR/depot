//! The route navigator: a small tilted 3D map in a corner of the screen, after the Route
//! Advisor of Euro Truck Simulator 2, made for a bus driver's duty.
//!
//! * The map turns with the bus and zooms out with speed; the camera looks over the bus
//!   from behind and above, so the road ahead fills the picture.
//! * Roads are drawn from the traffic network's lanes (dark casing, grey surface); the
//!   trip's route from the first stop to the last lies on them with arrows, coloured by
//!   how busy each stretch is (blue empty, green, yellow, red, dark red jammed), and its
//!   stops as markers (the last is a chequered flag).
//! * Leaving the route is noticed after a moment: a way back is searched on the lanes
//!   (Dijkstra, towards any lane of the route still ahead) and drawn instead - the route
//!   is recalculated as often as the driver goes wrong.
//! * Traffic: every AI vehicle near the bus is on the map, and roads whose cars crawl or
//!   stand are tinted amber or deep red (smoothed over seconds, so one car at a red light
//!   is no jam); the route itself takes the colour where it runs into one, and the header
//!   says how long it costs.
//! * The header: speed, the line, passengers aboard, day and time; the next stop with its
//!   distance and whether the bus is early or late; the speed limit sign and signals ahead
//!   on the map.
//!
//! The parts: [`minimap`] (the corner map and its header), [`city`] (the city map window),
//! [`route`] (the route on the lanes and the way back to it), [`geometry`] (roads from the
//! traffic network), [`streets`] (street names along them), [`lines`] (the bus lines panel)
//! and [`draw`] (the markers and icons both maps share).
//!
//! Everything is drawn with `omsi-ui` into a texture of its own (4x MSAA) that the game
//! shows as a premultiplied overlay. Roads are built once per area and kept on the GPU;
//! their width is in metres near the camera and in pixels far away, so zooming rebuilds
//! nothing.

use glam::{DMat3, DVec2, DVec3, Mat4, Vec2, Vec3};
use hashbrown::HashMap;
use omsi_render::TextureId;
use omsi_sim::traffic::Network;
use omsi_ui::{Atlas, Color, Fonts, Gpu, Rect};

use crate::traffic::Traffic;

pub(crate) mod city;
pub(crate) mod draw;
pub use draw::FocusKind;
pub(crate) mod geometry;
pub(crate) mod lines;
mod minimap;
mod route;
mod streets;

pub(crate) use draw::stop_requested;
pub(crate) use geometry::{city_roads, simplify, MapRoad};
use streets::Streets;

// --- colours (sRGB) -------------------------------------------------------------------

// neutral dark, half transparent, calm
const NAV_REDRAW_S: f32 = 1.0 / 30.0;
const PANEL: Color = Color::rgba(28, 29, 32, 0.74);
// (the bars under the texts darken whatever the opacity setting leaves of the panel: at a
// third the cab showed through behind the next stop)
const BAR: Color = Color::rgba(32, 33, 36, 0.90);
// (the launcher's map picture draws with these too, so the two maps cannot look apart)
pub(crate) const ROAD_CASING: Color = Color::rgba(30, 30, 30, 0.9);
pub(crate) const ROAD: Color = Color::rgba(92, 92, 92, 1.0);
pub(crate) const ROAD_MAIN: Color = Color::rgba(112, 112, 112, 1.0);
pub(crate) const ROUTE: Color = Color::rgba(214, 48, 40, 1.0);
const DOT: Color = Color::rgba(70, 140, 255, 1.0);
/// The public transport's dots: trolleybuses, buses, trams, and the text on their line tags.
const TROLLEY: Color = Color::rgba(46, 184, 92, 1.0);
const BUS: Color = Color::rgba(226, 58, 52, 1.0);
const TRAM: Color = Color::rgba(240, 190, 30, 1.0);
const METRO: Color = Color::rgba(150, 96, 230, 1.0);
const TRAIN: Color = Color::rgba(236, 132, 56, 1.0);
const LINE_TEXT: Color = Color::rgba(15, 15, 15, 1.0);
/// The other players of a LAN session or server: an arrow the way they face, with their name.
const PLAYER: Color = Color::rgba(190, 96, 255, 1.0);

/// Vertical field of view and tilt of the map camera (degrees).
const FOV: f32 = 40.0;
const PITCH: f64 = 52.0;
/// Roads are built for this far around the bus (m) and again when it has gone half-way.
const ROAD_RADIUS: f64 = 1300.0;
/// Off the route for this long (s) before a way back is looked for, and between tries.
const OFF_ROUTE_AFTER: f32 = 2.0;
const REROUTE_EVERY: f32 = 2.5;

/// One stop of the trip for the map.
#[derive(Debug, Clone)]
pub struct NavStop {
    /// The stop's map object (its place is looked up in the navigator's map when the
    /// timetable does not know it: its tile is not loaded yet).
    pub object_id: i64,
    pub position: DVec3,
    pub name: String,
    /// Planned arrival (s of the day).
    pub arrival: f64,
}

/// Another player of the session as the maps show them.
#[derive(Debug, Clone, PartialEq)]
pub struct NavPlayer {
    /// Where they are: their bus, or on foot where they walk (riding in someone's bus: that bus).
    pub position: DVec3,
    /// Compass heading (degrees, 0 = +y, clockwise).
    pub heading: f64,
    pub name: String,
}

/// Where the maps are centred when it is not on the bus (`Settings::nav_follow_view`): the walker
/// or the free camera, with the way they face.
#[derive(Debug, Clone, Copy)]
pub struct NavFocus {
    pub kind: draw::FocusKind,
    pub at: DVec3,
    /// Compass heading (degrees, 0 = +y, clockwise).
    pub heading: f64,
}

/// What the navigator is told each frame.
pub struct NavFrame<'a> {
    pub traffic: Option<&'a Traffic>,
    /// The other players of a LAN session or server, on both maps (#1011, #1080).
    pub players: Vec<NavPlayer>,
    pub bus: DVec3,
    /// Compass heading (degrees, 0 = +y, clockwise).
    pub heading: f64,
    /// The walker or the free camera the maps follow in place of the bus; the route still follows
    /// the bus.
    pub focus: Option<NavFocus>,
    pub speed_kmh: f32,
    /// Outside weather and cabin air temperatures (°C).
    pub outside_temp: f32,
    pub inside_temp: f32,
    /// Line, terminus, and the trip's stops from the next one on (the next is first).
    pub line: Option<String>,
    pub terminus: Option<String>,
    pub stops: Vec<NavStop>,
    /// How late the bus is (s, negative early), when on a duty.
    pub delay: Option<f64>,
    pub passengers: Option<usize>,
    /// The vehicle script's latched stop request or illuminated request lamp.
    pub stop_requested: bool,
    /// Seconds of the day and weekday (0 = Monday).
    pub time: f64,
    pub weekday: i32,
    /// The date of the session (yyyymmdd): which form of each line holds (a map with eras).
    pub date: i32,
    /// The line a timetable vehicle runs (by car id), for the trains that show none of their own.
    pub car_line: Option<&'a dyn Fn(u64) -> Option<String>>,
    pub language: &'a str,
    /// Window size in physical pixels.
    pub screen: (f32, f32),
    /// The player's interface size (`Settings::ui_scale`): the panel and the city map's
    /// texts and buttons grow with it.
    pub ui_scale: f32,
    /// The interface grows with a tall window (`Settings::ui_scale_window`); off, the panel
    /// is held to 480 px, as before.
    pub follow_window: bool,
    pub dt: f32,
    /// Where the information bar is (`ui::Ui::info_rect`): a navigator on the same side keeps
    /// below it.
    pub info_rect: Option<[f32; 4]>,
}

/// The texts, per language.
struct Words {
    kmh: &'static str,
    days: [&'static str; 7],
    off_route: &'static str,
    rerouting: &'static str,
    recalculated: &'static str,
    jam: &'static str,
    slow: &'static str,
    no_duty: &'static str,
    last_stop: &'static str,
    on_time: &'static str,
}

fn words(lang: &str) -> Words {
    match lang.to_ascii_uppercase().as_str() {
        "DEU" | "DE" | "GER" => Words {
            kmh: "km/h",
            days: ["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"],
            off_route: "Abseits der Route",
            rerouting: "Route wird neu berechnet",
            recalculated: "Route neu berechnet",
            jam: "Stau",
            slow: "Zähfließend",
            no_duty: "Freie Fahrt",
            last_stop: "Endhaltestelle",
            on_time: "pünktlich",
        },
        "FRA" | "FR" => Words {
            kmh: "km/h",
            days: ["Lun", "Mar", "Mer", "Jeu", "Ven", "Sam", "Dim"],
            off_route: "Hors itinéraire",
            rerouting: "Recalcul de l'itinéraire",
            recalculated: "Itinéraire recalculé",
            jam: "Bouchon",
            slow: "Ralentissement",
            no_duty: "Conduite libre",
            last_stop: "Terminus",
            on_time: "à l'heure",
        },
        "RUS" | "RU" => Words {
            kmh: "км/ч",
            days: ["Пн", "Вт", "Ср", "Чт", "Пт", "Сб", "Вс"],
            off_route: "Вне маршрута",
            rerouting: "Перестроение маршрута",
            recalculated: "Маршрут перестроен",
            jam: "Пробка",
            slow: "Затруднено",
            no_duty: "Свободная езда",
            last_stop: "Конечная",
            on_time: "по графику",
        },
        "PTB" | "PT" | "POR" | "BRA" => Words {
            kmh: "km/h",
            days: ["Seg", "Ter", "Qua", "Qui", "Sex", "Sáb", "Dom"],
            off_route: "Fora da rota",
            rerouting: "Recalculando a rota",
            recalculated: "Rota recalculada",
            jam: "Congestionamento",
            slow: "Trânsito lento",
            no_duty: "Direção livre",
            last_stop: "Ponto final",
            on_time: "no horário",
        },
        _ => Words {
            kmh: "km/h",
            days: ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"],
            off_route: "Off route",
            rerouting: "Recalculating route",
            recalculated: "Route recalculated",
            jam: "Traffic jam",
            slow: "Slow traffic",
            no_duty: "Free drive",
            last_stop: "Final stop",
            on_time: "on time",
        },
    }
}

/// What a click on the lines panel of the city map does.
#[derive(Clone, Copy, Debug, PartialEq)]
enum LinesHit {
    /// Tick or untick a line (its index in the package's lines).
    Toggle(usize),
    /// Show a ticked line's data (or tick it).
    Focus(usize),
    /// The filter tab: 0 all, 1 BRT, 2 express, 3 feeders.
    Filter(u8),
    /// The kind of vehicle on show: 0 bus, 1 tram, 2 metro, 3 rail.
    Modal(u8),
    /// The route of the line in focus (0 = route 1).
    RoutePick(usize),
    /// The timetable in place of the list, and back.
    Sheet,
    /// A day group (a weekday: 0, 5, 6) and a departure (its time and trip).
    Day(usize),
    Dep(crate::line_times::DepKey),
    /// Back from a departure's stops to the departures.
    Back,
    /// Tick every line of the tab on show, or clear them when all are ticked.
    All,
    /// A stop of the departure on show (its map object): the view goes to it.
    Stop(i64),
}

/// The lines panel of the city map: which lines are ticked and what is on show.
#[derive(Default)]
struct LinesView {
    open: bool,
    /// Lines ticked, in the order they were ticked.
    selected: Vec<usize>,
    /// The line the data card is about (opened by a double click).
    focus: Option<usize>,
    /// The last line clicked in the list and when (the navigator's clock), to tell a double click.
    last_click: Option<(usize, f32)>,
    /// The route chosen for a line (index into its routes).
    route: HashMap<usize, usize>,
    filter: u8,
    modal: u8,
    /// First row of the list on show.
    scroll: f32,
    /// Bumped whenever the choice changes: the ways are drawn again.
    version: u64,
    /// The ways' mesh (buffer 10): the version it was built for, its size and anchor.
    mesh: Option<(u64, u32, DVec2)>,
    /// The panel and its list, and what a click lands on (map picture pixels).
    panel: Option<Rect>,
    list: Option<Rect>,
    hits: Vec<(Rect, LinesHit)>,
    /// The view is moved to the line just ticked at the next frame.
    fit: bool,
    /// The timetable on show (of the route chosen: a route is a direction), its day (None: today's) and the
    /// departure picked (s of the day); `jump` takes the list to the next departure.
    sheet: bool,
    day: Option<usize>,
    dep: Option<crate::line_times::DepKey>,
    jump: bool,
}

/// The route being followed: the lanes, how far along the bus is, and whether it was
/// replaced by a way back after leaving it.
#[derive(Default)]
struct Route {
    key: String,
    lanes: Vec<usize>,
    complete: bool,
    generation: u64,
    /// Index into `lanes` of the lane the bus is on, and how far along it.
    progress: usize,
    s: f32,
    on_route: bool,
    off_for: f32,
    retry_in: f32,
    /// Seconds the "recalculated" note still shows.
    note: f32,
    /// Bumped whenever `lanes` or `progress` change (the cached route mesh).
    version: u64,
    /// `lanes` is a way to the next stop found by the navigator itself (the trip's own
    /// route is not there yet: its tiles still load); the trip's replaces it.
    provisional: bool,
    /// The bus has been on the route: before that, the way to it is simply the way to the
    /// next stop (found at once, and no "recalculating").
    joined: bool,
    /// `lanes` begins with a way to the route (the bus is looked for on it too).
    approach: bool,
}

/// Roads around a centre, built into vertex buffer 0.
struct Roads {
    light: bool,
    anchor: DVec2,
    lanes_seen: usize,
    verts: usize,
    built_at: f32,
}

pub struct Navigator {
    /// The current small-map texture in Scene::overlays, for the cockpit display.
    pub panel_overlay: Option<usize>,
    pub cockpit_display: bool,
    drawn_at: f32,
    pub enabled: bool,
    /// The next stops with their times under the map (Shift+N cycles map, map and
    /// schedule, off).
    pub schedule: bool,
    /// Smoothed speed (m/s) for the time to the next stop.
    speed_avg: f32,
    pub opacity: f32,
    pub corner: String,
    /// Where the navigator was dragged to (#940): its top-left corner as a share of the
    /// room the window leaves it across and down (`navigator_corner = at x,y`); None: the
    /// corner.
    pub at: Option<[f32; 2]>,
    /// A press on the navigator: where it was (cursor and the panel's top-left) and whether
    /// it has moved far enough to be a drag rather than a click.
    panel_drag: Option<([f32; 2], [f32; 2], bool)>,
    /// The room the window leaves the navigator (width, height) when it was last placed.
    pub panel_room: [f32; 2],
    /// The city map (a click on the navigator or Shift+M) and where the navigator is on the
    /// screen.
    pub city: CityMap,
    pub panel_rect: [f32; 4],
    pub origin_x: f32,
    gpu: Option<Gpu>,
    /// Under "DirectX9 (D3D9Ex)", the mirror the maps' Gpu hands its draws to when it is
    /// made (the maps' textures are drawn on that device too).
    pub(crate) ui_mirror: Option<omsi_ui::UiMirror>,
    fonts: Fonts,
    atlas: Atlas,
    target: Option<(TextureId, u32, u32)>,
    /// The map's own lanes when there is no traffic system.
    own_net: Option<std::sync::Arc<Network>>,
    /// The whole map's road network and object places (read in the background at the
    /// start, see `World::navigation_map`): routes, roads and stops beyond the loaded
    /// tiles. Its version changes when it arrives.
    global: Option<std::sync::Arc<Network>>,
    stop_pos: std::sync::Arc<HashMap<i64, DVec3>>,
    /// Street names of the map's lanes (from its street name signs).
    streets: Option<std::sync::Arc<Streets>>,
    /// The map's own surface (`Maps/<Map>/openomsi-realmap`), once read, its names, and a
    /// number that changes when it arrives (the meshes are built again).
    surface: Option<std::sync::Arc<crate::map_surface::MapSurface>>,
    surface_labels: Vec<crate::map_surface::Label>,
    surface_version: u64,
    /// The surface is drawn (the `nav_surface` setting).
    pub show_surface: bool,
    /// The terrain view of the city map (contour lines, when the map's package carries them): a button on the map.
    pub show_contours: bool,
    /// How the city map is turned: 0 as the map is drawn, 1 real north up, 2 the vehicle's heading up (a click on the compass changes it).
    pub orient: u8,
    /// The minimap looks straight down (the `nav_view` setting `top`) instead of tilted ahead of the vehicle.
    pub view_top: bool,
    /// The map's marker pictures as a texture of the navigator's own GPU state.
    icon_tex: Option<usize>,
    /// The municipality and the street the bus is on (from the package), refreshed now and then.
    loc_city: Option<String>,
    loc_street: Option<String>,
    /// The limit of the street the bus is on (km/h) and that of heavy vehicles, 0 when not said.
    loc_limit: (u32, u32),
    /// The height where the bus is (the terrain view only).
    loc_alt: Option<f32>,
    /// The light look of the maps (the `nav_light` setting).
    pub light: bool,
    /// The small map's surface mesh (buffer 6): the surface version and anchor it was built for, and its size.
    surface_mesh: Option<(u64, DVec2, u32)>,
    #[allow(clippy::type_complexity)]
    building: Option<
        std::sync::mpsc::Receiver<(
            Network,
            HashMap<i64, DVec3>,
            Streets,
            Option<crate::map_surface::MapSurface>,
            Vec<std::path::PathBuf>,
        )>,
    >,
    pub global_version: u64,
    roads: Option<Roads>,
    route: Route,
    /// The route mesh (buffer 1): the route and traffic versions and the anchor it was
    /// built for, and its size.
    route_mesh: (u64, u64, DVec2, u32, usize),
    /// Per lane of the traffic: how congested it is (0 free … 1 standing), smoothed.
    congestion: HashMap<usize, f32>,
    /// The same for the route's lanes ahead (in the route's network), and a version that
    /// changes when any of their levels does (the route is drawn again).
    route_jam: HashMap<usize, f32>,
    jam_version: u64,
    congestion_t: f32,
    /// Map camera: distance, heading, both smoothed.
    zoom: f64,
    cam_heading: f64,
    time: f32,
    /// Distance along the route to the next stop (m), refreshed now and then.
    next_dist: Option<f64>,
    dist_t: f32,
    /// The next turn on the route: direction (-1 left, 1 right, 2 back), how sharp
    /// (degrees), how far (m) and the street it turns into.
    /// OMSI 2's route arrows are wanted (see `turn_hint`).
    pub arrows: bool,
    /// The other (AI) vehicles are drawn on the maps (the `nav_ai` setting).
    pub show_ai: bool,
    /// How far the panel has faded in (Shift+N fades it in and out rather than cutting).
    shown: f32,
    /// The trip's next stops (place, name, the bus's heading there) and where the bus is,
    /// for the route arrows.
    stop_spots: Vec<(DVec3, String, f64, i64)>,
    bus_at: DVec3,
    /// The bus and the way it faces while the maps follow the walker or the free camera.
    bus_marker: Option<(DVec3, f64)>,
    next_turn: Option<(i32, f32, f64, Option<String>)>,
    /// The street the bus is on.
    street_here: Option<String>,
    /// Seconds of delay the jams ahead on the route cost.
    jam_cost: f32,
    first: bool,
    /// The game's time of day (s) and weekday (0 = Monday), as of the last frame: the lines panel's timetable.
    day_clock: (f64, i32),
    date: i32,
    /// The eras in force (Chrono folders), for the timetable.
    chrono: Vec<std::path::PathBuf>,
}

/// The duty as the navigator shows it: line, terminus, the stops from the next one on,
/// and the current trip (a key that changes with it, and its name).
#[allow(clippy::type_complexity)]
pub fn duty_parts(
    duty: Option<&crate::schedule::PlayerDuty>,
) -> (
    Option<String>,
    Option<String>,
    Vec<NavStop>,
    Option<(String, String)>,
) {
    let Some(d) = duty else {
        return (None, None, Vec::new(), None);
    };
    let Some(trip) = d.trips.get(d.trip_index) else {
        return (None, None, Vec::new(), None);
    };
    let line = if trip.line.trim().is_empty() {
        d.line.trim()
    } else {
        trip.line.trim()
    };
    let stops = trip
        .stops
        .iter()
        .skip(d.next_stop)
        .filter(|s| s.stops)
        .map(|s| NavStop {
            object_id: s.object_id,
            position: s.position.unwrap_or(DVec3::ZERO),
            name: s.name.clone(),
            arrival: s.arr,
        })
        .collect();
    (
        Some(line.to_string()),
        Some(trip.terminus.clone()),
        stops,
        Some((format!("{}/{}", d.trip_index, trip.name), trip.name.clone())),
    )
}

/// Screen position of a world point (relative to the anchor) in target pixels.
pub fn project(vp: Mat4, viewport: [f32; 4], p: Vec3) -> Option<Vec2> {
    let c = vp * p.extend(1.0);
    if c.w <= 0.1 {
        return None;
    }
    let n = c.truncate() / c.w;
    Some(Vec2::new(
        viewport[0] + (n.x * 0.5 + 0.5) * viewport[2],
        viewport[1] + (0.5 - n.y * 0.5) * viewport[3],
    ))
}

fn angle_diff(a: f64, b: f64) -> f64 {
    let mut d = (b - a) % 360.0;
    if d > 180.0 {
        d -= 360.0;
    } else if d < -180.0 {
        d += 360.0;
    }
    d
}

fn ease(dt: f32, tau: f32) -> f64 {
    (1.0 - (-dt / tau.max(1e-3)).exp()) as f64
}

/// A `navigator_corner` of the form `at x,y` (where the navigator was dragged to, #940).
pub fn placed_at(corner: &str) -> Option<[f32; 2]> {
    let (x, y) = corner.trim().strip_prefix("at")?.trim().split_once(',')?;
    let (x, y) = (x.trim().parse::<f32>().ok()?, y.trim().parse::<f32>().ok()?);
    (x.is_finite() && y.is_finite()).then(|| [x.clamp(0.0, 1.0), y.clamp(0.0, 1.0)])
}

/// The GPS is a separate render texture composed after the scene's post AA.
fn map_samples(format: wgpu::TextureFormat) -> u32 {
    if format
        .guaranteed_format_features(wgpu::Features::empty())
        .flags
        .sample_count_supported(4)
    {
        4
    } else {
        1
    }
}

impl<'a> NavFrame<'a> {
    fn clone_ref(&self) -> NavFrame<'a> {
        NavFrame {
            traffic: self.traffic,
            players: self.players.clone(),
            bus: self.bus,
            heading: self.heading,
            focus: self.focus,
            speed_kmh: self.speed_kmh,
            outside_temp: self.outside_temp,
            inside_temp: self.inside_temp,
            line: self.line.clone(),
            terminus: self.terminus.clone(),
            stops: self.stops.clone(),
            delay: self.delay,
            passengers: self.passengers,
            time: self.time,
            weekday: self.weekday,
            date: self.date,
            car_line: self.car_line,
            language: self.language,
            screen: self.screen,
            ui_scale: self.ui_scale,
            follow_window: self.follow_window,
            dt: self.dt,
            stop_requested: self.stop_requested,
            info_rect: self.info_rect,
        }
    }
}

/// Rotation of a 2D direction by a compass heading (for tests).
#[allow(dead_code)]
pub fn heading_vec(h: f64) -> DVec2 {
    let m = DMat3::from_rotation_z(-h.to_radians());
    m.transform_vector2(DVec2::Y)
}

/// The city map: the whole map from above in a large window over the game (after ETS2's
/// map screen, but not full-screen) - every road, the trip's route, its stops with their
/// names, the bus and the traffic. Dragged to move, the wheel zooms at the cursor, Escape
/// or a click outside closes it.
#[derive(Default)]
pub struct CityMap {
    pub open: bool,
    /// Where the window is on the screen (physical pixels).
    pub rect: [f32; 4],
    /// Centre of the view (world) and metres per pixel.
    pub center: DVec2,
    pub mpp: f64,
    /// The view stays on the bus until the map is dragged.
    follow: bool,
    pub drag: Option<(f32, f32)>,
    /// Where the pointer is over the map (window pixels), and the street found under it (for a point of about
    /// two metres: not looked up again while it stays on it).
    hover: Option<(f32, f32)>,
    /// The turn of the view this frame: the map direction that points up on the screen lies this many radians clockwise from the map's up.
    pub phi: f64,
    /// Where the compass is this frame (it is drawn after the buttons are listed, and takes its place in the list on the next one).
    compass: Option<Rect>,
    hover_cache: Option<(
        (i64, i64, i64, bool),
        (Option<crate::map_surface::Street>, Option<f32>),
    )>,
    target: Option<(TextureId, u32, u32)>,
    /// The roads mesh (buffer 3): the map version it was built for, its size and anchor.
    roads: Option<(u64, u32, DVec2)>,
    /// The route mesh (buffer 4): the route, traffic, arrow spacing and roads versions it
    /// was built for, and its size.
    route: ((u64, u64, u32, u64), u32),
    /// The map's extent (world), for the zoom limits.
    extent: (DVec2, DVec2),
    /// "Centre on the bus" and zoom buttons (window pixels).
    pub buttons: Vec<(Rect, u8)>,
    /// The surface mesh (buffer 7): the surface version it was built for, its size and anchor.
    surface: Option<(u64, u32, DVec2)>,
    /// The lines panel (the package's bus lines).
    lines: LinesView,
}
