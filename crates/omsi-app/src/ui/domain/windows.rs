//! The state of the Alt bar's windows: which one is open, the forms of the date and time,
//! the line and tour and the line and destination as they are edited, and what a key does in
//! them.

use super::super::interface::output::ROUTE_NUMBER_MAX;
use omsi_sim::SimClock;
use omsi_ui::alt_windows::{self as aw, View};
use winit::keyboard::KeyCode;

/// What the overlay draws of the window open, owned so that it can cross to the renderer.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolFrame {
    pub(in crate::ui) view: View,
    /// Which opening of a window this is (see `aw::View::draw`).
    pub(in crate::ui) key: u32,
}

/// The date and time as the window has them, not yet applied.
#[derive(Clone, Debug, PartialEq)]
pub struct TimeForm {
    pub hour: u32,
    pub minute: u32,
    pub day: u32,
    pub month: u32,
    pub year: i32,
    /// "Use the computer's date and time" ticked.
    pub real: bool,
    /// The quick choices: their names and the minute of the day they set.
    pub quick: Vec<(&'static str, u32)>,
}

/// The years the date can be set to.
pub const YEARS: (i32, i32) = (1900, 2100);

const WEEKDAYS: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

impl TimeForm {
    pub fn new(clock: &SimClock, real: bool) -> TimeForm {
        let (day, month) = clock.day_month();
        let t = clock.time.max(0.0) as u32;
        TimeForm {
            hour: (t / 3600) % 24,
            minute: (t / 60) % 60,
            day: day as u32,
            month: month as u32,
            year: clock.year,
            real,
            quick: Vec::new(),
        }
    }

    fn days_in_month(&self) -> u32 {
        omsi_ui::widgets::days_in_month(self.year, self.month)
    }

    /// Value `which` (hour, minute, day, month, year) one step down or up: the hour, the
    /// minute, the day and the month go round, the day stays in its month.
    pub fn step(&mut self, which: usize, by: i32) {
        let wrap = |v: u32, lo: u32, hi: u32| -> u32 {
            let n = (hi - lo + 1) as i32;
            ((v as i32 - lo as i32 + by).rem_euclid(n)) as u32 + lo
        };
        match which {
            0 => self.hour = wrap(self.hour, 0, 23),
            1 => self.minute = wrap(self.minute, 0, 59),
            2 => self.day = wrap(self.day, 1, self.days_in_month()),
            3 => self.month = wrap(self.month, 1, 12),
            4 => self.year = (self.year + by).clamp(YEARS.0, YEARS.1),
            _ => {}
        }
        self.day = self.day.clamp(1, self.days_in_month());
    }

    pub fn choose_quick(&mut self, i: usize) {
        if let Some(&(_, m)) = self.quick.get(i) {
            self.hour = m / 60 % 24;
            self.minute = m % 60;
        }
    }

    /// `base` with the form's date and time.
    pub fn clock(&self, base: &SimClock) -> SimClock {
        let mut c = base.clone();
        c.set_date(self.year, self.month as i32, self.day as i32);
        c.time = (self.hour * 3600 + self.minute * 60) as f64;
        c
    }

    pub fn weekday(&self) -> &'static str {
        let c = self.clock(&SimClock::default());
        WEEKDAYS[c.weekday().clamp(0, 6) as usize]
    }

    pub fn view(&self) -> aw::Time {
        aw::Time {
            hour: self.hour,
            minute: self.minute,
            day: self.day,
            month: self.month,
            year: self.year,
            weekday: self.weekday().to_string(),
            quick: self
                .quick
                .iter()
                .map(|(n, m)| (n.to_string(), format!("{:02}:{:02}", m / 60, m % 60)))
                .collect(),
            real: self.real,
        }
    }
}

/// The quick choices of the date and time for the day `date`: dawn and dusk when the sun
/// rises and sets there (the map's place, as the sky has it), else at six and seven.
pub(in crate::ui) fn quick_times(date: &SimClock) -> Vec<(&'static str, u32)> {
    let place = omsi_sim::daylight::place();
    let mut c = date.clone();
    let (mut rise, mut set) = (None, None);
    let mut before: Option<f64> = None;
    for m in (0..1440).step_by(2) {
        c.time = m as f64 * 60.0;
        let (alt, _) = omsi_sim::daylight::sun_position(&c, &place);
        if let Some(b) = before {
            if b < 0.0 && alt >= 0.0 && rise.is_none() {
                rise = Some(m);
            }
            if b >= 0.0 && alt < 0.0 {
                set = Some(m);
            }
        }
        before = Some(alt);
    }
    vec![
        ("Dawn", rise.unwrap_or(6 * 60)),
        ("Day", 12 * 60),
        ("Dusk", set.unwrap_or(19 * 60)),
        ("Night", 23 * 60),
    ]
}

/// The line, tour, trip and stop as the window has them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TourForm {
    /// (label, the line's name)
    pub lines: Vec<(String, String)>,
    pub line: Option<usize>,
    /// (label, the time it leaves next, the tour's number)
    pub tours: Vec<(String, String, String)>,
    pub tour: Option<usize>,
    /// The trip chosen (in order of the time they leave) and how many there are.
    pub trip: usize,
    pub trips: usize,
    /// (stop, time)
    pub stops: Vec<(String, String)>,
    pub stop: Option<usize>,
}

impl TourForm {
    pub fn view(&self) -> aw::Tour {
        aw::Tour {
            lines: self.lines.iter().map(|l| l.0.clone()).collect(),
            line: self.line,
            tours: self.tours.iter().map(|t| t.0.clone()).collect(),
            tour_times: self.tours.iter().map(|t| t.1.clone()).collect(),
            tour: self.tour,
            stops: self.stops.iter().map(|s| s.0.clone()).collect(),
            stop_times: self.stops.iter().map(|s| s.1.clone()).collect(),
            stop: self.stop,
            trip: if self.trips > 0 {
                format!("{} {} / {}", omsi_ui::tr("Trip"), self.trip + 1, self.trips)
            } else {
                String::new()
            },
            trip_steps: (self.trip > 0, self.trip + 1 < self.trips),
        }
    }

    /// The line and tour chosen: (line, tour number).
    pub fn chosen(&self) -> Option<(String, String)> {
        let line = self.lines.get(self.line?)?.1.clone();
        let tour = self.tours.get(self.tour?)?.2.clone();
        Some((line, tour))
    }
}

/// The route number and the destination as the window has them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DestForm {
    pub line: String,
    /// The route number shown when the window opened (changed only when typed over).
    pub line_before: String,
    pub focused: bool,
    /// (code, name, row of the depot file)
    pub destinations: Vec<(String, String, usize)>,
    pub selected: Option<usize>,
    pub selected_before: Option<usize>,
    pub empty: String,
}

/// The longest route number the field takes.
pub const ROUTE_MAX: usize = ROUTE_NUMBER_MAX;

impl DestForm {
    /// Text typed into the field.
    pub fn type_text(&mut self, text: &str) {
        for c in text.chars().filter(|c| !c.is_control()) {
            if self.line.chars().count() >= ROUTE_MAX {
                break;
            }
            self.line.push(c);
        }
    }

    /// What OK changes: the route number when it was typed over, the destination when another
    /// was chosen (its row of the depot file).
    pub fn changes(&self) -> (Option<String>, Option<usize>) {
        let line = self.line.trim();
        let line = (line != self.line_before.trim() && !line.is_empty()).then(|| line.to_string());
        let dest = self
            .selected
            .filter(|_| self.selected != self.selected_before)
            .and_then(|i| self.destinations.get(i))
            .map(|d| d.2);
        (line, dest)
    }
}

/// One of the bar's windows, with what it holds until OK.
#[derive(Clone, Debug, PartialEq)]
pub enum ToolWindow {
    /// The map's entry points.
    Reposition {
        names: Vec<String>,
        selected: Option<usize>,
    },
    Time(TimeForm),
    Weather {
        /// (name, file) of the weather files.
        files: Vec<(String, String)>,
        selected: Option<usize>,
        real: bool,
        real_label: String,
        /// (ICAO code, name) of the airports the real weather can follow, the first being
        /// "the nearest the map" (no code).
        stations: Vec<(String, String)>,
        station: Option<usize>,
        /// The station when the window opened.
        station_before: Option<usize>,
    },
    /// Game controller, mouse, keyboard only, head tracking: each applies at once.
    Controls,
    Tour(TourForm),
    /// The tour running: its trip's stops, each to come with a button to skip it.
    Running,
    /// The tour running: end it?
    EndTour,
    Destination(DestForm),
    /// The wash and the pump: two switches, each on while it runs.
    Service,
    /// The timetable evaluation and the driver profile.
    DriverMenu,
    /// The driver's personnel file, as it is saved.
    Profile,
    /// The timetable evaluation of the tour being driven, at trip `trip` of the duty.
    Evaluation {
        trip: usize,
    },
    Driver {
        names: Vec<String>,
        selected: Option<usize>,
    },
}

impl ToolWindow {
    /// The bar's button that opens this window.
    pub fn button(&self) -> &'static str {
        match self {
            ToolWindow::Reposition { .. } => "reposition",
            ToolWindow::Time(_) => "time",
            ToolWindow::Weather { .. } => "weather",
            ToolWindow::Controls => "controls",
            ToolWindow::Tour(_) | ToolWindow::Running | ToolWindow::EndTour => "tour",
            ToolWindow::Destination(_) => "destination",
            ToolWindow::Service => "service",
            ToolWindow::DriverMenu
            | ToolWindow::Profile
            | ToolWindow::Evaluation { .. }
            | ToolWindow::Driver { .. } => "driver",
        }
    }

    /// A text field of the window has the keyboard.
    pub fn typing(&self) -> bool {
        matches!(self, ToolWindow::Destination(d) if d.focused)
    }
}

/// The bar's windows on the game's side.
#[derive(Debug, Default)]
pub struct Tools {
    pub window: Option<ToolWindow>,
    /// How many windows were opened (each opening's lists start afresh).
    pub opened: u32,
    /// A press over the window was taken: its release goes to the window too.
    pub(in crate::ui) pressed: bool,
    /// Keys whose press the window took (its text field had the keyboard): their releases
    /// are the window's too.
    pub(in crate::ui) taken: std::collections::HashSet<KeyCode>,
}

impl Tools {
    /// Open `w` in place of the window open, if one was.
    pub fn open(&mut self, w: ToolWindow) {
        self.window = Some(w);
        self.opened = self.opened.wrapping_add(1);
        self.pressed = false;
    }

    pub fn close(&mut self) {
        self.window = None;
        self.pressed = false;
    }

    /// The window lost the mouse and the keys: no press or key of it is pending any more.
    pub fn forget_input(&mut self) {
        self.pressed = false;
        self.taken.clear();
        if let Some(ToolWindow::Destination(d)) = self.window.as_mut() {
            d.focused = false;
        }
    }

    /// A key while the window shows: true when it takes it. Esc closes the window (the Esc
    /// menu does not open then); while a text field has the keyboard every key but the
    /// modifiers is the field's (Enter is OK). Any other key is the game's. (What a key
    /// types comes as text first, `App::tool_text`; a press that brought none, as some
    /// systems send, types what its key stands for, as the game menu's route number does.)
    pub fn key(&mut self, code: KeyCode, pressed: bool) -> KeyAnswer {
        if !pressed {
            return if self.taken.remove(&code) {
                KeyAnswer::Taken
            } else {
                KeyAnswer::Game
            };
        }
        let typing = self.window.as_ref().is_some_and(ToolWindow::typing);
        let answer = match code {
            KeyCode::Escape => KeyAnswer::Close,
            _ if !typing => return KeyAnswer::Game,
            KeyCode::ShiftLeft
            | KeyCode::ShiftRight
            | KeyCode::ControlLeft
            | KeyCode::ControlRight
            | KeyCode::AltLeft
            | KeyCode::AltRight => return KeyAnswer::Game,
            KeyCode::Enter | KeyCode::NumpadEnter => KeyAnswer::Ok,
            KeyCode::Backspace | KeyCode::Delete => {
                if let Some(ToolWindow::Destination(d)) = self.window.as_mut() {
                    d.line.pop();
                }
                KeyAnswer::Taken
            }
            _ => {
                if let (Some(c), Some(ToolWindow::Destination(d))) =
                    (crate::input::route_char(code), self.window.as_mut())
                {
                    d.type_text(c.encode_utf8(&mut [0; 4]));
                }
                KeyAnswer::Taken
            }
        };
        self.taken.insert(code);
        answer
    }
}

/// What a key was to the window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum KeyAnswer {
    /// Not the window's: the game has it.
    Game,
    Taken,
    Close,
    Ok,
}
