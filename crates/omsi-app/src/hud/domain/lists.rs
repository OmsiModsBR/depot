//! The menu's lists and settings windows as data: which list is open, a move on a line,
//! the pages and drop-downs, and the steps the options run through.

/// Which list the chooser shows.
#[derive(Debug, Clone, PartialEq)]
pub enum ListKind {
    Admin,
    /// The options of the game, on this page of the settings window.
    Options(usize),
    /// What can be done with the vehicle, on this page.
    Vehicle(usize),
    /// The clock, the weather and the traffic, on this page.
    World(usize),
    /// Entry points for keyboard and game controller configuration.
    Controls,
    /// Vehicle bindings and game/camera bindings on separate pages.
    Keyboard(usize),
    ControllerDevices(usize),
    Controller(String, usize),
    ControllerAxis(String, usize),
    ControllerButtons(String),
    ControllerButtonSettings(String, usize),
    ControllerButton(String, usize),
    ControllerCapture(String),
    /// OMSI's KY_ vehicle events that can be added to the keyboard.
    Events,
    Lines,
    /// A line's tours; the stop chosen in the timetable beside them to start from: (the
    /// tour's number, the stop as `Schedule::tour_stops` lists them), none: the default.
    Tours(String, Option<(String, usize, usize)>),
    Drivers,
    Numbers,
    /// The termini of the bus's depot file, for its destination display.
    Destinations,
    /// The route numbers (lines) for the destination display: the depot file's and the
    /// map timetable's.
    RouteNumbers,
    /// The depot files (.hof) of the bus driven.
    Hofs,
    /// The map's entry points (the launcher's "Start at"), to put the bus at.
    Spots,
    /// Placing a vehicle: its manufacturer, then its type (the manufacturer's key), its
    /// livery, then its depot file (bus file; bus file and livery).
    PlaceMaker,
    PlaceType(String),
    PlaceLivery(String),
    PlaceHof(String, String),
}

/// The time speeds, traffic amounts and passenger shares the options step through.
pub const SPEEDS: [f64; 5] = [1.0, 2.0, 4.0, 8.0, 15.0];

pub const TRAFFIC: [usize; 7] = [0, 10, 20, 30, 50, 80, 120];

pub(in crate::hud) const PAX: [f32; 6] = [0.25, 0.5, 0.75, 1.0, 1.5, 2.0];

pub(in crate::hud) const VOLUME: [f32; 6] = [0.0, 0.2, 0.4, 0.6, 0.8, 1.0];

/// The pedal strengths the options step through (see `settings::pedal_curve`).
pub(in crate::hud) const PEDAL: [f32; 7] = [0.5, 0.7, 0.85, 1.0, 1.25, 1.5, 2.0];

/// A list line that heads the lines under it: not chosen, not run.
pub const HEADING: &str = "#";

/// The end of the action of a line whose value Left and Right step down and up (and the
/// arrows drawn round its value): the action is run with `-` or `+` in its place, and with
/// it as it is on Enter (see `App::chooser_adjust`).
pub(crate) const ADJUST: &str = " ±";

// ---------------------------------------------------------------------------------------
// the settings windows (options, vehicle, world): pages of rows

/// How a row's value is changed.
#[derive(Clone, Copy, Debug)]
pub enum Move {
    /// Enter or a click on the line: the next value (a switch flips, a row of values wraps).
    Next,
    /// Left: the previous value, or off.
    Dec,
    /// Right: the next value, or on.
    Inc,
    /// A click on a slider: this far (0 to 1) along it.
    To(f32),
}

/// One page of a settings window: its title and its rows (label, action).
pub(in crate::hud) type Page = (&'static str, Vec<(String, String)>);

/// The cloud types of OMSI's weather (`Weather/clouds.cfg`): the name in a weather file and
/// the name shown.
pub(in crate::hud) const CLOUD_TYPES: [(&str, &str); 5] = [
    ("-1", "None"),
    ("Cumulus 1", "Few clouds"),
    ("Cumulus 2", "Scattered"),
    ("Cumulus 3", "Broken"),
    ("Overcast 1", "Overcast"),
];

/// The kinds of precipitation of a weather file (`[precip]`'s first number).
pub(in crate::hud) const PRECIP_KINDS: [&str; 3] = ["None", "Rain", "Snow"];

/// The name the weather has once it was set by hand.
pub(crate) const CUSTOM_WEATHER: &str = "Custom weather";

/// A drop-down over a row of a settings window (the weather preset, the clouds), as a
/// select in a page: the entries drop down under the row's value.
pub(crate) struct Dropdown {
    /// The row of the window it belongs to.
    pub row: usize,
    /// (label, action) of its entries.
    pub items: Vec<(String, String)>,
    /// The entry the keyboard is on, the first one shown, and the one in force now.
    pub sel: usize,
    pub top: usize,
    pub current: Option<usize>,
}

pub(in crate::hud) type TitlesCache =
    Option<(ListKind, bool, std::time::Instant, (Vec<String>, usize))>;
