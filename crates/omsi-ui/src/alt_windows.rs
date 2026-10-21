//! The small windows the Alt bar opens: put the bus at an entry point, the date and time, the
//! weather, the controls, the line and tour, the line and destination, the timetable
//! evaluation and the driver profile. They are made of the in-game windows' building blocks ([`crate::ingame`]) and
//! look like the "Add vehicle" window: in the middle of the picture, the title in capitals,
//! the buttons at the foot.
//!
//! What a window shows is the game's: it hands each one a [`View`] with its data, and gets
//! back the [`Event`] a click asked for. None of them pauses anything: the game goes on
//! behind them.

use crate::ingame::{
    self, button, check_row, foot, head, list_rows, stepper, text_field, ButtonKind, ListLook,
    ReportRow, BUTTON_H, CHECK_H, FIELD_H, FOOT_H, PANEL, ROW_H, TITLE_H,
};
use crate::theme::accent;
use crate::widgets::Ui;
use crate::{Color, Rect, Weight};
use glam::Vec2;

/// The note at the right of every window's title.
pub const NOTE: &str = "the game keeps running";

const LINE: Color = Color::hex(0x2b2f37);
const MUTED: Color = Color::hex(0x9aa0aa);
const FG: Color = Color::hex(0xe9eaec);
const TS: f32 = 1.16;

/// What a click in a window asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// Row `.1` of list `.0` (lists are counted from the left, from 0).
    Row(usize, usize),
    /// Stepper `.0` one step down (-1) or up (1).
    Step(usize, i32),
    /// Check box or menu entry `.0`.
    Check(usize),
    /// Quick choice `.0`.
    Quick(usize),
    /// Text field `.0` was clicked.
    Field(usize),
    /// The window's main button.
    Ok,
    /// Cancel, or Close.
    Cancel,
}

/// A list to choose one line from (the entry points, the driver profiles).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pick {
    pub title: String,
    /// Over the list.
    pub head: String,
    /// The game's names (drawn as they are).
    pub rows: Vec<String>,
    pub selected: Option<usize>,
    /// The main button's label.
    pub ok: String,
    pub hint: String,
    /// Shown while there are no rows.
    pub empty: String,
}

/// The date and time.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Time {
    pub hour: u32,
    pub minute: u32,
    pub day: u32,
    /// 1 to 12.
    pub month: u32,
    pub year: i32,
    pub weekday: String,
    /// The quick choices: their name and the time they set ("06:12").
    pub quick: Vec<(String, String)>,
    /// The clock follows the computer's date and time (the values cannot be changed then).
    pub real: bool,
}

/// The weather.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Weather {
    /// The real weather (the airport's report) is on: the list cannot be chosen from then.
    pub real: bool,
    pub real_label: String,
    pub presets: Vec<String>,
    pub selected: Option<usize>,
}

/// One entry of a [`Menu`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Entry {
    pub label: String,
    /// A switch, ticked when on; `None`: an entry that opens something.
    pub checked: Option<bool>,
    /// Why it cannot be used now.
    pub disabled: Option<String>,
}

/// A short menu of entries (the controls; the timetable and the driver).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Menu {
    pub title: String,
    pub entries: Vec<Entry>,
    pub hint: String,
}

/// The line, the tour and the stop to start from.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tour {
    pub lines: Vec<String>,
    pub line: Option<usize>,
    pub tours: Vec<String>,
    /// The time each tour leaves next.
    pub tour_times: Vec<String>,
    pub tour: Option<usize>,
    pub stops: Vec<String>,
    pub stop_times: Vec<String>,
    pub stop: Option<usize>,
    /// "Trip 2 of 5", with whether there is a trip before and after it.
    pub trip: String,
    pub trip_steps: (bool, bool),
}

/// A question with two answers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Confirm {
    pub title: String,
    pub text: String,
    pub ok: String,
    pub cancel: String,
}

/// The route number and the destination of the bus driven.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Destination {
    pub line: String,
    /// The route number's field has the keyboard.
    pub focused: bool,
    pub codes: Vec<String>,
    pub destinations: Vec<String>,
    pub selected: Option<usize>,
    /// Shown while there are no destinations.
    pub empty: String,
}

/// The timetable evaluation of the tour being driven: a trip's stops with their planned and
/// actual times, as the end-of-trip report shows them, while the trip is still on.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Evaluation {
    /// The trip shown ("Trip 3  ·  50 to Central Station  ·  08:00:00").
    pub title: String,
    /// "Trip 2 of 5", with whether there is a trip before and after it.
    pub trip: String,
    pub trip_steps: (bool, bool),
    pub rows: Vec<ReportRow>,
    /// Under the stops: (label, value).
    pub totals: Vec<(String, String)>,
    pub hint: String,
}

/// Where a stop of the trip being driven is, for [`Running`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RunStop {
    /// Behind the bus, served.
    Done,
    /// Behind the bus, not served: skipped or missed.
    Passed,
    /// The stop the bus is due at.
    #[default]
    Next,
    /// Still to come.
    Ahead,
}

impl RunStop {
    /// Whether the bus is past it.
    pub fn behind(self) -> bool {
        matches!(self, RunStop::Done | RunStop::Passed)
    }
}

/// One stop of [`Running`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RunRow {
    pub stop: String,
    /// Under the name: its time and how it went ("08:56 · served, +1 min").
    pub detail: String,
    pub state: RunStop,
    /// The label of the button at its right ("Skip this stop"); none for a stop behind.
    pub skip: Option<String>,
}

/// The timetable being driven: the trip's stops, each still to come with a button to skip
/// it (a deliberate improvement: the original has no such list).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Running {
    /// At the right of the title ("Line 136 · trip 08:52").
    pub sub: String,
    pub rows: Vec<RunRow>,
    /// Under the stops: what skipping does.
    pub hint: String,
}

/// One of the windows, with what it shows.
#[derive(Clone, Debug, PartialEq)]
pub enum View {
    Pick(Pick),
    Time(Time),
    Weather(Weather),
    Menu(Menu),
    Tour(Tour),
    Confirm(Confirm),
    Destination(Destination),
    Evaluation(Evaluation),
    Running(Running),
}

/// A window of `w` x `h` points in the middle of a picture of `size` points.
fn centred(size: Vec2, w: f32, h: f32) -> Rect {
    let w = w.min(size.x - 32.0).max(0.0);
    let h = h.min(size.y - 32.0).max(0.0);
    Rect::new(
        ((size.x - w) * 0.5).floor(),
        ((size.y - h) * 0.5).floor(),
        w,
        h,
    )
}

impl View {
    /// Where the window is in a picture of `size` points.
    pub fn rect(&self, size: Vec2) -> Rect {
        let (w, h) = match self {
            View::Pick(_) => (540.0, 560.0),
            View::Time(_) => (620.0, 340.0),
            View::Weather(_) => (560.0, 580.0),
            View::Menu(m) => (
                460.0,
                TITLE_H + 1.0 + 16.0 + m.entries.len() as f32 * CHECK_H + FOOT_H,
            ),
            View::Tour(_) => (1000.0, 600.0),
            View::Confirm(_) => (500.0, 190.0),
            View::Destination(_) => (560.0, 600.0),
            View::Evaluation(_) => (900.0, 620.0),
            View::Running(_) => (700.0, 470.0),
        };
        centred(size, w, h)
    }

    /// Draw the window in a picture of `size` points and return what a click in it asked for.
    /// `key` tells one opening of a window from the next (its lists start at the top, or at
    /// the line chosen, each time it opens).
    pub fn draw(&self, ui: &mut Ui, size: Vec2, key: u32) -> Option<Event> {
        let r = self.rect(size);
        match self {
            View::Pick(v) => pick(ui, r, v, key),
            View::Time(v) => time(ui, r, v),
            View::Weather(v) => weather(ui, r, v, key),
            View::Menu(v) => menu(ui, r, v),
            View::Tour(v) => tour(ui, r, v, key),
            View::Confirm(v) => confirm(ui, r, v),
            View::Destination(v) => destination(ui, r, v, key),
            View::Evaluation(v) => evaluation(ui, r, v, key),
            View::Running(v) => running(ui, r, v),
        }
    }
}

/// The main button and Cancel at the foot of `r`; the event they asked for.
fn ok_cancel(
    ui: &mut Ui,
    name: &str,
    r: Rect,
    ok: &str,
    enabled: bool,
    hint: &str,
) -> Option<Event> {
    match foot(ui, name, r, &[(ok, enabled), ("Cancel", true)], hint) {
        Some(0) => Some(Event::Ok),
        Some(_) => Some(Event::Cancel),
        None => None,
    }
}

/// The bottom of the space for the lists of a window `r` (over its foot).
fn lists_bottom(r: Rect) -> f32 {
    r.bottom() - FOOT_H - 6.0
}

fn pick(ui: &mut Ui, r: Rect, v: &Pick, key: u32) -> Option<Event> {
    let body = ingame::window(ui, r, &v.title, NOTE);
    let mut event = None;
    head(ui, &v.head, body.x + 16.0, body.y + 17.0);
    let area = Rect::new(
        body.x + 4.0,
        body.y + 34.0,
        body.w - 8.0,
        lists_bottom(r) - body.y - 34.0,
    );
    let look = ListLook {
        as_is: true,
        empty: &v.empty,
        ..Default::default()
    };
    if let Some(i) = list_rows(
        ui,
        &format!("tool:{key}:pick"),
        area,
        &v.rows,
        v.selected,
        &look,
    ) {
        event = Some(Event::Row(0, i));
    }
    let ok = ok_cancel(ui, "tool", r, &v.ok, v.selected.is_some(), &v.hint);
    ok.or(event)
}

fn time(ui: &mut Ui, r: Rect, v: &Time) -> Option<Event> {
    let body = ingame::window(ui, r, "Date and time", NOTE);
    let mut event = None;
    let x0 = body.x + 16.0;
    // the two numbers of the time need less room than the three parts of the date
    let col = ((body.w - 48.0) * 0.42).floor();
    let dcol = body.w - 48.0 - col;
    let x1 = x0 + col + 16.0;
    let on = !v.real;
    // the time of day at the left, the date at the right
    head(ui, "Time of day", x0, body.y + 20.0);
    head(ui, "Date", x1, body.y + 20.0);
    let y = body.y + 36.0;
    let hw = ((col - 20.0) * 0.5).floor();
    let steps = [
        (
            0,
            Rect::new(x0, y, hw, ingame::STEPPER_H),
            format!("{:02}", v.hour),
        ),
        (
            1,
            Rect::new(x0 + col - hw, y, hw, ingame::STEPPER_H),
            format!("{:02}", v.minute),
        ),
    ];
    for (k, rr, value) in steps {
        let by = stepper(ui, &format!("tool:time:{k}"), rr, &value, on);
        if by != 0 {
            event = Some(Event::Step(k, by));
        }
    }
    ingame::text(
        ui,
        ":",
        x0 + hw + 7.0,
        y + ingame::STEPPER_H * 0.5,
        16.0,
        Weight::Bold,
        FG,
    );
    let dw = ((dcol - 12.0) * 0.27).floor();
    let yw = ((dcol - 12.0) * 0.38).floor();
    let mw = dcol - 12.0 - dw - yw;
    let month = crate::widgets::MONTHS[(v.month.clamp(1, 12) - 1) as usize];
    let date = [
        (
            2,
            Rect::new(x1, y, dw, ingame::STEPPER_H),
            v.day.to_string(),
        ),
        (
            3,
            Rect::new(x1 + dw + 6.0, y, mw, ingame::STEPPER_H),
            month.to_string(),
        ),
        (
            4,
            Rect::new(x1 + dw + mw + 12.0, y, yw, ingame::STEPPER_H),
            v.year.to_string(),
        ),
    ];
    for (k, rr, value) in date {
        let by = stepper(ui, &format!("tool:time:{k}"), rr, &value, on);
        if by != 0 {
            event = Some(Event::Step(k, by));
        }
    }
    let below = y + ingame::STEPPER_H + 16.0;
    head(ui, &v.weekday, x1, below);
    // the quick choices, across
    let qy = below + 26.0;
    head(ui, "Quick choice", x0, qy);
    let n = v.quick.len().max(1) as f32;
    let qw = ((body.w - 32.0 - (n - 1.0) * 8.0) / n).floor();
    for (i, (name, at)) in v.quick.iter().enumerate() {
        let rr = Rect::new(x0 + i as f32 * (qw + 8.0), qy + 16.0, qw, BUTTON_H);
        let label = format!("{}  {at}", crate::tr(name));
        if button(
            ui,
            &format!("tool:quick:{i}"),
            rr,
            &label,
            ButtonKind::Normal,
            on,
        ) {
            event = Some(Event::Quick(i));
        }
    }
    let cy = qy + 16.0 + BUTTON_H + 12.0;
    ui.p().rect(Rect::new(r.x + 1.0, cy, r.w - 2.0, 1.0), LINE);
    let row = Rect::new(body.x, cy + 1.0, body.w, CHECK_H);
    if check_row(
        ui,
        "tool:time:real",
        row,
        "Use the computer's date and time",
        v.real,
        None,
    ) {
        event = Some(Event::Check(0));
    }
    let ok = ok_cancel(
        ui,
        "tool",
        r,
        "OK",
        true,
        "Cancel leaves the clock as it is",
    );
    ok.or(event)
}

fn weather(ui: &mut Ui, r: Rect, v: &Weather, key: u32) -> Option<Event> {
    let body = ingame::window(ui, r, "Weather", NOTE);
    let mut event = None;
    let row = Rect::new(body.x, body.y + 4.0, body.w, CHECK_H);
    if check_row(ui, "tool:weather:real", row, &v.real_label, v.real, None) {
        event = Some(Event::Check(0));
    }
    let y = row.bottom() + 4.0;
    ui.p().rect(Rect::new(r.x + 1.0, y, r.w - 2.0, 1.0), LINE);
    head(ui, "Ready-made weather", body.x + 16.0, y + 18.0);
    let area = Rect::new(
        body.x + 4.0,
        y + 35.0,
        body.w - 8.0,
        lists_bottom(r) - y - 35.0,
    );
    let look = ListLook {
        enabled: !v.real,
        as_is: true,
        empty: "No weather files found",
        ..Default::default()
    };
    let name = format!("tool:{key}:weather");
    if let Some(i) = list_rows(ui, &name, area, &v.presets, v.selected, &look) {
        event = Some(Event::Row(0, i));
    }
    let hint = if v.real {
        "The weather follows the airport's report"
    } else {
        "Cancel leaves the weather as it is"
    };
    let ok = ok_cancel(ui, "tool", r, "OK", true, hint);
    ok.or(event)
}

fn menu(ui: &mut Ui, r: Rect, v: &Menu) -> Option<Event> {
    let body = ingame::window(ui, r, &v.title, NOTE);
    let mut event = None;
    for (i, e) in v.entries.iter().enumerate() {
        let row = Rect::new(body.x, body.y + 8.0 + i as f32 * CHECK_H, body.w, CHECK_H);
        if i > 0 {
            let line = Rect::new(row.x + 16.0, row.y, row.w - 32.0, 1.0);
            ui.p().rect(line, Color::hex(0x20242b));
        }
        let id = format!("tool:menu:{i}");
        let clicked = match e.checked {
            Some(on) => check_row(ui, &id, row, &e.label, on, e.disabled.as_deref()),
            None => opens_row(ui, &id, row, &e.label, e.disabled.as_deref()),
        };
        if clicked {
            event = Some(Event::Check(i));
        }
    }
    // (a menu's entries apply at once: its only button closes it)
    if foot(ui, "tool", r, &[("Close", true)], &v.hint).is_some() {
        event = Some(Event::Cancel);
    }
    event
}

/// A menu entry that opens something: its label, and an arrow at the right (or the reason it
/// cannot be used). Returns whether it was clicked.
fn opens_row(ui: &mut Ui, id: &str, r: Rect, label: &str, disabled: Option<&str>) -> bool {
    let enabled = disabled.is_none();
    let (hover, _, click) = ui.interact(crate::widgets::id_of(id), r);
    if hover && enabled {
        ui.p().rect(r, Color::WHITE.alpha(0.04));
    }
    let px = 14.0 * TS;
    let reason_w = disabled
        .map(|d| (ui.width(d, 12.0 * TS, Weight::Regular) + 16.0).min(r.w * 0.55))
        .unwrap_or(30.0);
    let fg = if enabled { FG } else { FG.alpha(0.4) };
    let lr = Rect::new(r.x + 16.0, r.y, (r.w - 32.0 - reason_w).max(0.0), r.h);
    ui.text_in(
        label,
        lr,
        px,
        Weight::Regular,
        fg,
        crate::paint::Align::Left,
    );
    match disabled {
        Some(why) => {
            let rr = Rect::new(r.right() - 16.0 - reason_w, r.y, reason_w, r.h);
            ui.text_in(
                why,
                rr,
                12.0 * TS,
                Weight::Regular,
                MUTED,
                crate::paint::Align::Right,
            );
        }
        None => {
            let c = if hover { accent() } else { MUTED };
            ui.icon(
                "chevron_right",
                Vec2::new(r.right() - 24.0, r.center().y),
                20.0,
                c,
            );
        }
    }
    click && enabled
}

fn tour(ui: &mut Ui, r: Rect, v: &Tour, key: u32) -> Option<Event> {
    let body = ingame::window(ui, r, "Line and tour", NOTE);
    let mut event = None;
    let w0 = (body.w * 0.22).floor();
    let w1 = (body.w * 0.34).floor();
    let x1 = body.x + w0 + 1.0;
    let x2 = x1 + w1 + 1.0;
    let w2 = body.right() - x2;
    let top = body.y + 34.0;
    let h = lists_bottom(r) - top;
    for x in [x1 - 1.0, x2 - 1.0] {
        ui.p().rect(
            Rect::new(x, body.y + 8.0, 1.0, r.bottom() - FOOT_H - body.y - 8.0),
            LINE,
        );
    }
    head(ui, "Line", body.x + 16.0, body.y + 17.0);
    head(ui, "Tour", x1 + 16.0, body.y + 17.0);
    head(ui, "Start from", x2 + 16.0, body.y + 17.0);
    let lines = ListLook {
        as_is: true,
        empty: "No timetable on this map",
        ..Default::default()
    };
    let area = Rect::new(body.x + 4.0, top, w0 - 8.0, h);
    if let Some(i) = list_rows(
        ui,
        &format!("tool:{key}:lines"),
        area,
        &v.lines,
        v.line,
        &lines,
    ) {
        event = Some(Event::Row(0, i));
    }
    let tours = ListLook {
        leads: &v.tour_times,
        lead_w: 62.0,
        as_is: true,
        empty: if v.line.is_some() {
            "No tour of this line runs now"
        } else {
            "Choose a line"
        },
        ..Default::default()
    };
    let area = Rect::new(x1 + 4.0, top, w1 - 8.0, h);
    // (another line's tours start at their top)
    let name = format!("tool:{key}:tours:{}", v.line.unwrap_or(usize::MAX));
    if let Some(i) = list_rows(ui, &name, area, &v.tours, v.tour, &tours) {
        event = Some(Event::Row(1, i));
    }
    if v.tour.is_some() && !v.trip.is_empty() {
        let sw = 190.0f32.min(w2 - 120.0);
        let sr = Rect::new(body.right() - 12.0 - sw, body.y + 4.0, sw, 28.0);
        let enabled = v.trip_steps.0 || v.trip_steps.1;
        let by = stepper(ui, "tool:tour:trip", sr, &v.trip, enabled);
        if (by < 0 && v.trip_steps.0) || (by > 0 && v.trip_steps.1) {
            event = Some(Event::Step(0, by));
        }
    }
    let stops = ListLook {
        leads: &v.stop_times,
        lead_w: 62.0,
        as_is: true,
        empty: "Choose a tour",
        ..Default::default()
    };
    let area = Rect::new(x2 + 4.0, top, w2 - 8.0, h);
    let name = format!(
        "tool:{key}:stops:{}:{}",
        v.tour.unwrap_or(usize::MAX),
        v.trip
    );
    if let Some(i) = list_rows(ui, &name, area, &v.stops, v.stop, &stops) {
        event = Some(Event::Row(2, i));
    }
    let ready = v.tour.is_some() && v.stop.is_some();
    let hint = "The tour starts from the chosen stop; the bus stays where it is";
    let ok = ok_cancel(ui, "tool", r, "Start the tour", ready, hint);
    ok.or(event)
}

fn confirm(ui: &mut Ui, r: Rect, v: &Confirm) -> Option<Event> {
    let body = ingame::window(ui, r, &v.title, NOTE);
    ui.paragraph(
        &v.text,
        Vec2::new(body.x + 16.0, body.y + 20.0),
        body.w - 32.0,
        14.0 * TS,
        Weight::Regular,
        FG,
    );
    match foot(ui, "tool", r, &[(&v.ok, true), (&v.cancel, true)], "") {
        Some(0) => Some(Event::Ok),
        Some(_) => Some(Event::Cancel),
        None => None,
    }
}

fn destination(ui: &mut Ui, r: Rect, v: &Destination, key: u32) -> Option<Event> {
    let body = ingame::window(ui, r, "Line and destination", NOTE);
    let mut event = None;
    head(ui, "Route number", body.x + 16.0, body.y + 20.0);
    let f = Rect::new(body.x + 16.0, body.y + 36.0, body.w - 32.0, FIELD_H);
    if text_field(
        ui,
        "tool:dest:line",
        f,
        &v.line,
        v.focused,
        "Type a route number",
    ) {
        event = Some(Event::Field(0));
    }
    let y = f.bottom() + 16.0;
    ui.p().rect(Rect::new(r.x + 1.0, y, r.w - 2.0, 1.0), LINE);
    head(ui, "Destination", body.x + 16.0, y + 18.0);
    let area = Rect::new(
        body.x + 4.0,
        y + 35.0,
        body.w - 8.0,
        lists_bottom(r) - y - 35.0,
    );
    let look = ListLook {
        leads: &v.codes,
        lead_w: 56.0,
        as_is: true,
        empty: &v.empty,
        ..Default::default()
    };
    let name = format!("tool:{key}:dest");
    if let Some(i) = list_rows(ui, &name, area, &v.destinations, v.selected, &look) {
        event = Some(Event::Row(0, i));
    }
    let ok = ok_cancel(ui, "tool", r, "OK", true, "Applies to the bus you drive");
    ok.or(event)
}

fn evaluation(ui: &mut Ui, r: Rect, v: &Evaluation, key: u32) -> Option<Event> {
    let body = ingame::window(ui, r, "Timetable evaluation", NOTE);
    let mut event = None;
    let sw = 190.0f32.min(body.w * 0.4);
    let title_w = body.w - 48.0 - sw;
    ui.text_in(
        &v.title,
        Rect::new(body.x + 16.0, body.y + 4.0, title_w.max(0.0), 36.0),
        14.0 * TS,
        Weight::Medium,
        FG,
        crate::paint::Align::Left,
    );
    if !v.trip.is_empty() {
        let sr = Rect::new(body.right() - 16.0 - sw, body.y + 8.0, sw, 28.0);
        let enabled = v.trip_steps.0 || v.trip_steps.1;
        let by = stepper(ui, "tool:eval:trip", sr, &v.trip, enabled);
        if (by < 0 && v.trip_steps.0) || (by > 0 && v.trip_steps.1) {
            event = Some(Event::Step(0, by));
        }
    }
    let totals_h = v.totals.len() as f32 * ROW_H;
    let head_y = body.y + 44.0;
    let area = Rect::new(
        body.x,
        head_y,
        body.w,
        (r.bottom() - FOOT_H - totals_h - 8.0 - head_y).max(ROW_H + 34.0),
    );
    // (each trip shown, and each opening, starts at its first stop)
    let scroll = format!("tool:{key}:eval:{}", v.title);
    let rows = ingame::stops_table(ui, &scroll, area, &v.rows);
    if v.rows.is_empty() {
        ui.text_in(
            "No stops on this trip",
            rows,
            14.0 * TS,
            Weight::Regular,
            MUTED,
            crate::paint::Align::Center,
        );
    }
    let totals = Rect::new(r.x, rows.bottom() + 8.0, r.w, totals_h);
    ui.p()
        .rect(Rect::new(r.x + 1.0, totals.y - 1.0, r.w - 2.0, 1.0), LINE);
    let kv: Vec<(&str, String, Option<Color>)> = v
        .totals
        .iter()
        .map(|(k, val)| (k.as_str(), val.clone(), None))
        .collect();
    ingame::key_values(ui, totals, &kv);
    // (it only shows: its one button closes it)
    if foot(ui, "tool", r, &[("Close", true)], &v.hint).is_some() {
        event = Some(Event::Cancel);
    }
    event
}

/// The height of a stop's row in [`Running`].
const RUN_ROW_H: f32 = 52.0;

/// The red of a step that ends something (the timetable).
const DANGER: Color = Color::hex(0xe8523f);

/// The stops of the trip being driven, from two behind the next on, as many as fit. Skipping
/// one is `Event::Row(0, stop)`, "Cancel the timetable" `Event::Check(0)`, Close `Cancel`.
fn running(ui: &mut Ui, r: Rect, v: &Running) -> Option<Event> {
    let body = ingame::window(ui, r, "Timetable running", &v.sub);
    let mut event = None;
    let hint_h = 40.0;
    let list = Rect::new(
        body.x + 16.0,
        body.y + 10.0,
        body.w - 32.0,
        (r.bottom() - FOOT_H - hint_h - body.y - 10.0).max(0.0),
    );
    let fit = (list.h / RUN_ROW_H).floor().max(1.0) as usize;
    let next = v
        .rows
        .iter()
        .position(|x| !x.state.behind())
        .unwrap_or(v.rows.len());
    let from = next.saturating_sub(2).min(v.rows.len().saturating_sub(fit));
    for (k, row) in v.rows.iter().enumerate().skip(from).take(fit) {
        let rr = Rect::new(
            list.x,
            list.y + (k - from) as f32 * RUN_ROW_H,
            list.w,
            RUN_ROW_H - 4.0,
        );
        if row.state == RunStop::Next {
            // (mixed over the panel, as the Alt bar's tiles: blended they came out lighter)
            ui.p().rounded(rr, 4.0, PANEL.mix(accent(), 0.18));
            ui.p()
                .rounded_border(rr, 4.0, 1.0, PANEL.mix(accent(), 0.7));
        }
        let icon = match row.state {
            RunStop::Done => "o-check",
            RunStop::Passed => "o-skip",
            RunStop::Next | RunStop::Ahead => "o-pin",
        };
        // (a stop left unserved keeps no mark: its sign is muted)
        let tone = if row.state == RunStop::Passed {
            MUTED
        } else {
            FG
        };
        ui.icon(icon, Vec2::new(rr.x + 18.0, rr.center().y), 18.0, tone);
        let bw = row
            .skip
            .as_deref()
            .map(|l| ui.width(l, 13.0 * TS, Weight::Medium) + 30.0)
            .unwrap_or(0.0);
        let tw = (rr.w - 48.0 - bw - 12.0).max(0.0);
        ui.text_in(
            &row.stop,
            Rect::new(rr.x + 40.0, rr.y + 4.0, tw, 22.0),
            14.0 * TS,
            Weight::Medium,
            FG,
            crate::paint::Align::Left,
        );
        ui.text_in(
            &row.detail,
            Rect::new(rr.x + 40.0, rr.y + 24.0, tw, 18.0),
            12.0 * TS,
            Weight::Regular,
            MUTED,
            crate::paint::Align::Left,
        );
        if let Some(label) = row.skip.as_deref() {
            let b = Rect::new(rr.right() - 10.0 - bw, rr.center().y - 14.0, bw, 28.0);
            if button(
                ui,
                &format!("tool:run:skip:{k}"),
                b,
                label,
                ButtonKind::Normal,
                true,
            ) {
                event = Some(Event::Row(0, k));
            }
        }
    }
    if v.rows.is_empty() {
        ingame::note(ui, "This trip has no stops", list);
    }
    ui.paragraph(
        &v.hint,
        Vec2::new(body.x + 16.0, r.bottom() - FOOT_H - hint_h + 4.0),
        body.w - 32.0,
        12.0 * TS,
        Weight::Regular,
        MUTED,
    );
    // the foot: ending the timetable at the left, in red; Close at the right
    ui.p().rect(
        Rect::new(r.x + 1.0, r.bottom() - FOOT_H, r.w - 2.0, 1.0),
        LINE,
    );
    let y = r.bottom() - FOOT_H + 12.0;
    let end = "Cancel the timetable";
    let ew = ingame::button_width(ui, end);
    let er = Rect::new(r.x + 16.0, y, ew, BUTTON_H);
    let (hover, _, click) = ui.interact(crate::widgets::id_of("tool:run:end"), er);
    ui.p().rounded(
        er,
        4.0,
        if hover {
            DANGER.alpha(0.14)
        } else {
            DANGER.alpha(0.06)
        },
    );
    ui.p().rounded_border(er, 4.0, 1.0, DANGER);
    ui.text_in(
        end,
        er,
        14.0 * TS,
        Weight::Medium,
        DANGER,
        crate::paint::Align::Center,
    );
    if click {
        event = Some(Event::Check(0));
    }
    let cw = ingame::button_width(ui, "Close");
    if button(
        ui,
        "tool:run:close",
        Rect::new(r.right() - 16.0 - cw, y, cw, BUTTON_H),
        "Close",
        ButtonKind::Normal,
        true,
    ) {
        event = Some(Event::Cancel);
    }
    event
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::id_of;

    const SIZE: Vec2 = Vec2::new(1672.0, 941.0);

    /// One frame of `view`, with a click at `at` first when there is one; the event.
    fn run(ui: &mut Ui, view: &View, click: Option<Vec2>) -> Option<Event> {
        let frame = |ui: &mut Ui| {
            ui.begin(SIZE, 1.0, 0.016);
            let e = view.draw(ui, SIZE, 1);
            ui.finish();
            e
        };
        if let Some(p) = click {
            ui.input.mouse = p;
            ui.input.pressed = true;
            ui.input.down = true;
            frame(ui);
            ui.input.released = true;
            ui.input.down = false;
        }
        frame(ui)
    }

    fn at(ui: &Ui, id: &str) -> Vec2 {
        ui.drawn[&id_of(id)].center()
    }

    #[test]
    fn a_pick_window_needs_a_choice_before_its_main_button_works() {
        let mut v = Pick {
            title: "Reposition".into(),
            head: "Entry point".into(),
            rows: (0..30).map(|i| format!("Entry {i}")).collect(),
            ok: "Reposition".into(),
            ..Default::default()
        };
        let mut ui = Ui::new();
        run(&mut ui, &View::Pick(v.clone()), None);
        let ok = at(&ui, "tool:btn:0");
        assert_eq!(run(&mut ui, &View::Pick(v.clone()), Some(ok)), None);
        let row = at(&ui, "tool:1:pick:row:3");
        assert_eq!(
            run(&mut ui, &View::Pick(v.clone()), Some(row)),
            Some(Event::Row(0, 3))
        );
        v.selected = Some(3);
        run(&mut ui, &View::Pick(v.clone()), None);
        assert_eq!(
            run(&mut ui, &View::Pick(v.clone()), Some(ok)),
            Some(Event::Ok)
        );
        let cancel = at(&ui, "tool:btn:1");
        assert_eq!(
            run(&mut ui, &View::Pick(v), Some(cancel)),
            Some(Event::Cancel)
        );
    }

    #[test]
    fn the_time_window_steps_its_values_unless_the_real_time_is_on() {
        let mut v = Time {
            hour: 8,
            minute: 30,
            day: 20,
            month: 10,
            year: 2026,
            weekday: "Tuesday".into(),
            quick: vec![
                ("Dawn".into(), "06:12".into()),
                ("Night".into(), "23:00".into()),
            ],
            real: false,
        };
        let mut ui = Ui::new();
        run(&mut ui, &View::Time(v.clone()), None);
        // the right half of the hour's stepper steps it up
        let hour = ui.drawn[&(id_of("tool:time:0") ^ 2)].center();
        assert_eq!(
            run(&mut ui, &View::Time(v.clone()), Some(hour)),
            Some(Event::Step(0, 1))
        );
        let year_down = ui.drawn[&(id_of("tool:time:4") ^ 1)].center();
        assert_eq!(
            run(&mut ui, &View::Time(v.clone()), Some(year_down)),
            Some(Event::Step(4, -1))
        );
        let night = at(&ui, "tool:quick:1");
        assert_eq!(
            run(&mut ui, &View::Time(v.clone()), Some(night)),
            Some(Event::Quick(1))
        );
        let real = at(&ui, "tool:time:real");
        assert_eq!(
            run(&mut ui, &View::Time(v.clone()), Some(real)),
            Some(Event::Check(0))
        );
        v.real = true;
        assert_eq!(run(&mut ui, &View::Time(v.clone()), Some(hour)), None);
        assert_eq!(run(&mut ui, &View::Time(v), Some(night)), None);
    }

    #[test]
    fn a_dim_menu_entry_cannot_be_chosen_and_close_cancels() {
        let v = Menu {
            title: "Controls".into(),
            entries: vec![
                Entry {
                    label: "Game controller".into(),
                    checked: Some(false),
                    disabled: Some("No game controller found".into()),
                },
                Entry {
                    label: "Mouse".into(),
                    checked: Some(true),
                    disabled: None,
                },
                Entry {
                    label: "Driver profile".into(),
                    checked: None,
                    disabled: None,
                },
            ],
            hint: String::new(),
        };
        let view = View::Menu(v);
        let mut ui = Ui::new();
        run(&mut ui, &view, None);
        let r = view.rect(SIZE);
        for i in 0..3 {
            assert!(r.contains(at(&ui, &format!("tool:menu:{i}"))));
        }
        let pad = at(&ui, "tool:menu:0");
        assert_eq!(run(&mut ui, &view, Some(pad)), None);
        let mouse = at(&ui, "tool:menu:1");
        assert_eq!(run(&mut ui, &view, Some(mouse)), Some(Event::Check(1)));
        let driver = at(&ui, "tool:menu:2");
        assert_eq!(run(&mut ui, &view, Some(driver)), Some(Event::Check(2)));
        let close = at(&ui, "tool:btn:0");
        assert_eq!(run(&mut ui, &view, Some(close)), Some(Event::Cancel));
    }

    #[test]
    fn the_tour_window_starts_only_with_a_tour_and_a_stop() {
        let mut v = Tour {
            lines: vec!["Line 1".into(), "Line 2".into()],
            line: Some(0),
            tours: vec!["Tour 1".into(), "Tour 2".into()],
            tour_times: vec!["08:00".into(), "08:20".into()],
            ..Default::default()
        };
        let mut ui = Ui::new();
        run(&mut ui, &View::Tour(v.clone()), None);
        let start = at(&ui, "tool:btn:0");
        assert_eq!(run(&mut ui, &View::Tour(v.clone()), Some(start)), None);
        let line = at(&ui, "tool:1:lines:row:1");
        assert_eq!(
            run(&mut ui, &View::Tour(v.clone()), Some(line)),
            Some(Event::Row(0, 1))
        );
        let tour = at(&ui, "tool:1:tours:0:row:1");
        assert_eq!(
            run(&mut ui, &View::Tour(v.clone()), Some(tour)),
            Some(Event::Row(1, 1))
        );
        v.tour = Some(1);
        v.stops = vec!["Depot".into(), "Market".into()];
        v.stop_times = vec!["08:20".into(), "08:31".into()];
        v.stop = Some(0);
        v.trip = "Trip 1 of 3".into();
        v.trip_steps = (false, true);
        run(&mut ui, &View::Tour(v.clone()), None);
        let later = ui.drawn[&(id_of("tool:tour:trip") ^ 2)].center();
        assert_eq!(
            run(&mut ui, &View::Tour(v.clone()), Some(later)),
            Some(Event::Step(0, 1))
        );
        // (there is no trip before the first)
        let earlier = ui.drawn[&(id_of("tool:tour:trip") ^ 1)].center();
        assert_eq!(run(&mut ui, &View::Tour(v.clone()), Some(earlier)), None);
        assert_eq!(run(&mut ui, &View::Tour(v), Some(start)), Some(Event::Ok));
    }

    #[test]
    fn the_destination_window_reports_its_field_and_its_rows() {
        let v = Destination {
            line: "50".into(),
            codes: vec!["1".into(), "2".into()],
            destinations: vec!["Central Station".into(), "Depot".into()],
            selected: Some(0),
            ..Default::default()
        };
        let view = View::Destination(v);
        let mut ui = Ui::new();
        run(&mut ui, &view, None);
        let field = at(&ui, "tool:dest:line");
        assert_eq!(run(&mut ui, &view, Some(field)), Some(Event::Field(0)));
        let depot = at(&ui, "tool:1:dest:row:1");
        assert_eq!(run(&mut ui, &view, Some(depot)), Some(Event::Row(0, 1)));
    }

    #[test]
    fn the_evaluation_steps_through_the_trips_driven_and_close_closes_it() {
        let row = |stop: &str| ReportRow {
            stop: stop.into(),
            scheduled: "08:00:00".into(),
            actual: String::new(),
            deviation: String::new(),
            state: crate::ingame::StopState::None,
        };
        let v = Evaluation {
            title: "Trip 2".into(),
            trip: "Trip 2 / 4".into(),
            trip_steps: (true, false),
            rows: (0..30).map(|i| row(&format!("Stop {i}"))).collect(),
            totals: vec![("Stops served".into(), "0 of 30".into())],
            hint: String::new(),
        };
        let view = View::Evaluation(v);
        let mut ui = Ui::new();
        run(&mut ui, &view, None);
        let earlier = ui.drawn[&(id_of("tool:eval:trip") ^ 1)].center();
        assert_eq!(run(&mut ui, &view, Some(earlier)), Some(Event::Step(0, -1)));
        // (the trip on now is the last there is to see)
        let later = ui.drawn[&(id_of("tool:eval:trip") ^ 2)].center();
        assert_eq!(run(&mut ui, &view, Some(later)), None);
        let close = at(&ui, "tool:btn:0");
        assert_eq!(run(&mut ui, &view, Some(close)), Some(Event::Cancel));
    }

    #[test]
    fn every_window_fits_a_small_picture() {
        let views = [
            View::Pick(Pick::default()),
            View::Time(Time::default()),
            View::Weather(Weather::default()),
            View::Menu(Menu {
                entries: vec![Entry::default(); 4],
                ..Default::default()
            }),
            View::Tour(Tour::default()),
            View::Confirm(Confirm::default()),
            View::Destination(Destination::default()),
            View::Evaluation(Evaluation::default()),
        ];
        // (the smallest picture: 960 x 540 pixels at the least scale)
        let small = Vec2::new(1280.0, 720.0);
        for v in &views {
            let r = v.rect(small);
            assert!(
                r.x >= 0.0 && r.y >= 0.0 && r.right() <= small.x && r.bottom() <= small.y,
                "{v:?}"
            );
            let mut ui = Ui::new();
            ui.begin(small, 1.0, 0.016);
            assert_eq!(v.draw(&mut ui, small, 0), None);
            ui.finish();
        }
    }

    #[test]
    fn the_running_timetable_skips_a_stop_ends_or_closes() {
        let row = |stop: &str, state, skip: bool| RunRow {
            stop: stop.into(),
            detail: "08:52".into(),
            state,
            skip: skip.then(|| "Skip this stop".into()),
        };
        let mut rows = vec![
            row("A", RunStop::Done, false),
            row("B", RunStop::Done, false),
        ];
        rows.push(row("C", RunStop::Next, true));
        rows.extend((0..30).map(|i| row(&format!("S{i}"), RunStop::Ahead, true)));
        let v = View::Running(Running {
            sub: "Line 5 · trip 08:52".into(),
            rows,
            hint: "Skipping a stop".into(),
        });
        let mut ui = Ui::new();
        run(&mut ui, &v, None);
        // the list starts two stops before the next; a stop behind has no button
        assert!(!ui.drawn.contains_key(&id_of("tool:run:skip:0")));
        assert!(!ui.drawn.contains_key(&id_of("tool:run:skip:1")));
        let skip = at(&ui, "tool:run:skip:3");
        assert_eq!(run(&mut ui, &v, Some(skip)), Some(Event::Row(0, 3)));
        // (the window shows as many as fit, not all thirty)
        assert!(!ui.drawn.contains_key(&id_of("tool:run:skip:32")));
        let end = at(&ui, "tool:run:end");
        assert_eq!(run(&mut ui, &v, Some(end)), Some(Event::Check(0)));
        let close = at(&ui, "tool:run:close");
        assert_eq!(run(&mut ui, &v, Some(close)), Some(Event::Cancel));
    }
}
