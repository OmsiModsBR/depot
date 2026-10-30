//! The navigator's objects: what it is told each frame (`NavFrame` with its stops, players
//! and focus), its own state (`Navigator`, the city map's `CityMap`, the route followed, the
//! lines panel) and the colours and distances it is drawn with. How it is drawn and how it
//! follows the bus is in `implementation::nav`.

use glam::{DVec2, DVec3};
use hashbrown::HashMap;
use omsi_render::TextureId;
use omsi_sim::traffic::Network;
use omsi_ui::{Atlas, Color, Fonts, Gpu, Rect};

use crate::traffic::Traffic;

// --- colours (sRGB) -------------------------------------------------------------------

// neutral dark, half transparent, calm
pub(in crate::hud) const NAV_REDRAW_S: f32 = 1.0 / 30.0;
pub(in crate::hud) const PANEL: Color = Color::rgba(28, 29, 32, 0.74);
// (the bars under the texts darken whatever the opacity setting leaves of the panel: at a
// third the cab showed through behind the next stop)
pub(in crate::hud) const BAR: Color = Color::rgba(32, 33, 36, 0.90);
// (the launcher's map picture draws with these too, so the two maps cannot look apart)
pub(crate) const ROAD_CASING: Color = Color::rgba(30, 30, 30, 0.9);
pub(crate) const ROAD: Color = Color::rgba(92, 92, 92, 1.0);
pub(crate) const ROAD_MAIN: Color = Color::rgba(112, 112, 112, 1.0);
pub(crate) const ROUTE: Color = Color::rgba(214, 48, 40, 1.0);
pub(in crate::hud) const DOT: Color = Color::rgba(70, 140, 255, 1.0);
/// The public transport's dots: trolleybuses, buses, trams, and the text on their line tags.
pub(in crate::hud) const TROLLEY: Color = Color::rgba(46, 184, 92, 1.0);
pub(in crate::hud) const BUS: Color = Color::rgba(226, 58, 52, 1.0);
pub(in crate::hud) const TRAM: Color = Color::rgba(240, 190, 30, 1.0);
pub(in crate::hud) const METRO: Color = Color::rgba(150, 96, 230, 1.0);
pub(in crate::hud) const TRAIN: Color = Color::rgba(236, 132, 56, 1.0);
pub(in crate::hud) const LINE_TEXT: Color = Color::rgba(15, 15, 15, 1.0);
/// The other players of a LAN session or server: an arrow the way they face, with their name.
pub(in crate::hud) const PLAYER: Color = Color::rgba(190, 96, 255, 1.0);

/// Vertical field of view and tilt of the map camera (degrees).
pub const FOV: f32 = 40.0;
pub(in crate::hud) const PITCH: f64 = 52.0;
/// Roads are built for this far around the bus (m) and again when it has gone half-way.
pub(in crate::hud) const ROAD_RADIUS: f64 = 1300.0;
/// Off the route for this long (s) before a way back is looked for, and between tries.
pub(in crate::hud) const OFF_ROUTE_AFTER: f32 = 2.0;
pub(in crate::hud) const REROUTE_EVERY: f32 = 2.5;

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
    pub kind: FocusKind,
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
pub(in crate::hud) struct Words {
    pub(in crate::hud) kmh: &'static str,
    pub(in crate::hud) days: [&'static str; 7],
    pub(in crate::hud) off_route: &'static str,
    pub(in crate::hud) rerouting: &'static str,
    pub(in crate::hud) recalculated: &'static str,
    pub(in crate::hud) jam: &'static str,
    pub(in crate::hud) slow: &'static str,
    pub(in crate::hud) no_duty: &'static str,
    pub(in crate::hud) last_stop: &'static str,
    pub(in crate::hud) on_time: &'static str,
}

/// What the navigator is centred on when it is not the bus: the walker on foot, or the free
/// camera (F4).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum FocusKind {
    Walker,
    Camera,
}

/// What a click on the lines panel of the city map does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::hud) enum LinesHit {
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
pub(in crate::hud) struct LinesView {
    pub(in crate::hud) open: bool,
    /// Lines ticked, in the order they were ticked.
    pub(in crate::hud) selected: Vec<usize>,
    /// The line the data card is about (opened by a double click).
    pub(in crate::hud) focus: Option<usize>,
    /// The last line clicked in the list and when (the navigator's clock), to tell a double click.
    pub(in crate::hud) last_click: Option<(usize, f32)>,
    /// The route chosen for a line (index into its routes).
    pub(in crate::hud) route: HashMap<usize, usize>,
    pub(in crate::hud) filter: u8,
    pub(in crate::hud) modal: u8,
    /// First row of the list on show.
    pub(in crate::hud) scroll: f32,
    /// Bumped whenever the choice changes: the ways are drawn again.
    pub(in crate::hud) version: u64,
    /// The ways' mesh (buffer 10): the version it was built for, its size and anchor.
    pub(in crate::hud) mesh: Option<(u64, u32, DVec2)>,
    /// The panel and its list, and what a click lands on (map picture pixels).
    pub(in crate::hud) panel: Option<Rect>,
    pub(in crate::hud) list: Option<Rect>,
    pub(in crate::hud) hits: Vec<(Rect, LinesHit)>,
    /// The view is moved to the line just ticked at the next frame.
    pub(in crate::hud) fit: bool,
    /// The timetable on show (of the route chosen: a route is a direction), its day (None: today's) and the
    /// departure picked (s of the day); `jump` takes the list to the next departure.
    pub(in crate::hud) sheet: bool,
    pub(in crate::hud) day: Option<usize>,
    pub(in crate::hud) dep: Option<crate::line_times::DepKey>,
    pub(in crate::hud) jump: bool,
}

/// The route being followed: the lanes, how far along the bus is, and whether it was
/// replaced by a way back after leaving it.
#[derive(Default)]
pub(in crate::hud) struct Route {
    pub(in crate::hud) key: String,
    pub(in crate::hud) lanes: Vec<usize>,
    pub(in crate::hud) complete: bool,
    pub(in crate::hud) generation: u64,
    /// Index into `lanes` of the lane the bus is on, and how far along it.
    pub(in crate::hud) progress: usize,
    pub(in crate::hud) s: f32,
    pub(in crate::hud) on_route: bool,
    pub(in crate::hud) off_for: f32,
    pub(in crate::hud) retry_in: f32,
    /// Seconds the "recalculated" note still shows.
    pub(in crate::hud) note: f32,
    /// Bumped whenever `lanes` or `progress` change (the cached route mesh).
    pub(in crate::hud) version: u64,
    /// `lanes` is a way to the next stop found by the navigator itself (the trip's own
    /// route is not there yet: its tiles still load); the trip's replaces it.
    pub(in crate::hud) provisional: bool,
    /// The bus has been on the route: before that, the way to it is simply the way to the
    /// next stop (found at once, and no "recalculating").
    pub(in crate::hud) joined: bool,
    /// `lanes` begins with a way to the route (the bus is looked for on it too).
    pub(in crate::hud) approach: bool,
}

/// Roads around a centre, built into vertex buffer 0.
pub(in crate::hud) struct Roads {
    pub(in crate::hud) light: bool,
    pub(in crate::hud) anchor: DVec2,
    pub(in crate::hud) lanes_seen: usize,
    pub(in crate::hud) verts: usize,
    pub(in crate::hud) built_at: f32,
}

pub struct Navigator {
    /// The current small-map texture in Scene::overlays, for the cockpit display.
    pub panel_overlay: Option<usize>,
    pub cockpit_display: bool,
    pub(in crate::hud) drawn_at: f32,
    pub enabled: bool,
    /// The next stops with their times under the map (Shift+N cycles map, map and
    /// schedule, off).
    pub schedule: bool,
    /// Smoothed speed (m/s) for the time to the next stop.
    pub(in crate::hud) speed_avg: f32,
    pub opacity: f32,
    pub corner: String,
    /// Where the navigator was dragged to (#940): its top-left corner as a share of the
    /// room the window leaves it across and down (`navigator_corner = at x,y`); None: the
    /// corner.
    pub at: Option<[f32; 2]>,
    /// A press on the navigator: where it was (cursor and the panel's top-left) and whether
    /// it has moved far enough to be a drag rather than a click.
    pub(in crate::hud) panel_drag: Option<([f32; 2], [f32; 2], bool)>,
    /// The room the window leaves the navigator (width, height) when it was last placed.
    pub panel_room: [f32; 2],
    /// The city map (a click on the navigator or Shift+M) and where the navigator is on the
    /// screen.
    pub city: CityMap,
    pub panel_rect: [f32; 4],
    pub origin_x: f32,
    pub(in crate::hud) gpu: Option<Gpu>,
    /// Under "DirectX9 (D3D9Ex)", the mirror the maps' Gpu hands its draws to when it is
    /// made (the maps' textures are drawn on that device too).
    pub(crate) ui_mirror: Option<omsi_ui::UiMirror>,
    pub(in crate::hud) fonts: Fonts,
    pub(in crate::hud) atlas: Atlas,
    pub(in crate::hud) target: Option<(TextureId, u32, u32)>,
    /// The map's own lanes when there is no traffic system.
    pub(in crate::hud) own_net: Option<std::sync::Arc<Network>>,
    /// The whole map's road network and object places (read in the background at the
    /// start, see `World::navigation_map`): routes, roads and stops beyond the loaded
    /// tiles. Its version changes when it arrives.
    pub(in crate::hud) global: Option<std::sync::Arc<Network>>,
    pub(in crate::hud) stop_pos: std::sync::Arc<HashMap<i64, DVec3>>,
    /// Street names of the map's lanes (from its street name signs).
    pub(in crate::hud) streets: Option<std::sync::Arc<Streets>>,
    /// The map's own surface (`Maps/<Map>/openomsi-realmap`), once read, its names, and a
    /// number that changes when it arrives (the meshes are built again).
    pub(in crate::hud) surface: Option<std::sync::Arc<crate::map_surface::MapSurface>>,
    pub(in crate::hud) surface_labels: Vec<crate::map_surface::Label>,
    pub(in crate::hud) surface_version: u64,
    /// The surface is drawn (the `nav_surface` setting).
    pub show_surface: bool,
    /// The terrain view of the city map (contour lines, when the map's package carries them): a button on the map.
    pub show_contours: bool,
    /// How the city map is turned: 0 as the map is drawn, 1 real north up, 2 the vehicle's heading up (a click on the compass changes it).
    pub orient: u8,
    /// The minimap looks straight down (the `nav_view` setting `top`) instead of tilted ahead of the vehicle.
    pub view_top: bool,
    /// The map's marker pictures as a texture of the navigator's own GPU state.
    pub(in crate::hud) icon_tex: Option<usize>,
    /// The municipality and the street the bus is on (from the package), refreshed now and then.
    pub(in crate::hud) loc_city: Option<String>,
    pub(in crate::hud) loc_street: Option<String>,
    /// The limit of the street the bus is on (km/h) and that of heavy vehicles, 0 when not said.
    pub(in crate::hud) loc_limit: (u32, u32),
    /// The height where the bus is (the terrain view only).
    pub(in crate::hud) loc_alt: Option<f32>,
    /// The light look of the maps (the `nav_light` setting).
    pub light: bool,
    /// The small map's surface mesh (buffer 6): the surface version and anchor it was built for, and its size.
    pub(in crate::hud) surface_mesh: Option<(u64, DVec2, u32)>,
    #[allow(clippy::type_complexity)]
    pub(in crate::hud) building: Option<
        std::sync::mpsc::Receiver<(
            Network,
            HashMap<i64, DVec3>,
            Streets,
            Option<crate::map_surface::MapSurface>,
            Vec<std::path::PathBuf>,
        )>,
    >,
    pub global_version: u64,
    pub(in crate::hud) roads: Option<Roads>,
    pub(in crate::hud) route: Route,
    /// The route mesh (buffer 1): the route and traffic versions and the anchor it was
    /// built for, and its size.
    pub(in crate::hud) route_mesh: (u64, u64, DVec2, u32, usize),
    /// Per lane of the traffic: how congested it is (0 free … 1 standing), smoothed.
    pub(in crate::hud) congestion: HashMap<usize, f32>,
    /// The same for the route's lanes ahead (in the route's network), and a version that
    /// changes when any of their levels does (the route is drawn again).
    pub(in crate::hud) route_jam: HashMap<usize, f32>,
    pub(in crate::hud) jam_version: u64,
    pub(in crate::hud) congestion_t: f32,
    /// Map camera: distance, heading, both smoothed.
    pub(in crate::hud) zoom: f64,
    pub(in crate::hud) cam_heading: f64,
    pub(in crate::hud) time: f32,
    /// Distance along the route to the next stop (m), refreshed now and then.
    pub(in crate::hud) next_dist: Option<f64>,
    pub(in crate::hud) dist_t: f32,
    /// The next turn on the route: direction (-1 left, 1 right, 2 back), how sharp
    /// (degrees), how far (m) and the street it turns into.
    /// OMSI 2's route arrows are wanted (see `turn_hint`).
    pub arrows: bool,
    /// The other (AI) vehicles are drawn on the maps (the `nav_ai` setting).
    pub show_ai: bool,
    /// How far the panel has faded in (Shift+N fades it in and out rather than cutting).
    pub(in crate::hud) shown: f32,
    /// The trip's next stops (place, name, the bus's heading there) and where the bus is,
    /// for the route arrows.
    pub(in crate::hud) stop_spots: Vec<(DVec3, String, f64, i64)>,
    pub(in crate::hud) bus_at: DVec3,
    /// The bus and the way it faces while the maps follow the walker or the free camera.
    pub(in crate::hud) bus_marker: Option<(DVec3, f64)>,
    pub(in crate::hud) next_turn: Option<(i32, f32, f64, Option<String>)>,
    /// The street the bus is on.
    pub(in crate::hud) street_here: Option<String>,
    /// Seconds of delay the jams ahead on the route cost.
    pub(in crate::hud) jam_cost: f32,
    pub(in crate::hud) first: bool,
    /// The game's time of day (s) and weekday (0 = Monday), as of the last frame: the lines panel's timetable.
    pub(in crate::hud) day_clock: (f64, i32),
    pub(in crate::hud) date: i32,
    /// The eras in force (Chrono folders), for the timetable.
    pub(in crate::hud) chrono: Vec<std::path::PathBuf>,
}

/// The street names of the map: per lane of its network the street it belongs to, and
/// where to write each name on the city map. OMSI maps have no street names of their own,
/// but their street name signs carry them: a sign names the road its plate runs along,
/// and the name is carried on along that road until it turns or meets another name.
pub struct Streets {
    pub(in crate::hud) names: Vec<String>,
    /// Per lane: index into `names`, or `u32::MAX`.
    pub(in crate::hud) of_lane: Vec<u32>,
    /// A place to write a name: the point, the road's direction (radians, world, from +x
    /// anticlockwise) and the name.
    pub(in crate::hud) labels: Vec<(DVec2, f32, u32)>,
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
    pub(in crate::hud) follow: bool,
    pub drag: Option<(f32, f32)>,
    /// Where the pointer is over the map (window pixels), and the street found under it (for a point of about
    /// two metres: not looked up again while it stays on it).
    pub(in crate::hud) hover: Option<(f32, f32)>,
    /// The turn of the view this frame: the map direction that points up on the screen lies this many radians clockwise from the map's up.
    pub phi: f64,
    /// Where the compass is this frame (it is drawn after the buttons are listed, and takes its place in the list on the next one).
    pub(in crate::hud) compass: Option<Rect>,
    pub(in crate::hud) hover_cache: Option<(
        (i64, i64, i64, bool),
        (Option<crate::map_surface::Street>, Option<f32>),
    )>,
    pub(in crate::hud) target: Option<(TextureId, u32, u32)>,
    /// The roads mesh (buffer 3): the map version it was built for, its size and anchor.
    pub(in crate::hud) roads: Option<(u64, u32, DVec2)>,
    /// The route mesh (buffer 4): the route, traffic, arrow spacing and roads versions it
    /// was built for, and its size.
    pub(in crate::hud) route: ((u64, u64, u32, u64), u32),
    /// The map's extent (world), for the zoom limits.
    pub(in crate::hud) extent: (DVec2, DVec2),
    /// "Centre on the bus" and zoom buttons (window pixels).
    pub buttons: Vec<(Rect, u8)>,
    /// The surface mesh (buffer 7): the surface version it was built for, its size and anchor.
    pub(in crate::hud) surface: Option<(u64, u32, DVec2)>,
    /// The lines panel (the package's bus lines).
    pub(in crate::hud) lines: LinesView,
}
