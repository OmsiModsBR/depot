//! The small windows the Alt bar opens, drawn by the new interface: put the bus at an entry
//! point, the date and time, the weather, the controls, the line and tour (and ending it), the
//! line and destination, the wash and the pump, the timetable evaluation and the driver
//! profile.
//!
//! Each does what the game already does for the same thing (the Esc menu's lists and
//! settings pages, the journey log): only the window that asks for it is new. None of them
//! pauses the game, and none points at the Esc menu. While one is open the bar's other
//! buttons wait (not for the wash and refuel menu), the game goes on behind it and the
//! driving keys still reach the bus, except while the route number's field has the keyboard.
//! Esc closes it; Cancel discards what was changed in it.
//!
//! As with the "Add vehicle" window, the overlay draws the window inside the renderer: the
//! clicks and the wheel over it are handed to the overlay ([`WinInput`]) and what they asked
//! for comes back as [`Event`]s the game applies before the next frame.

use super::super::domain::menu::scale_for;
use super::super::domain::win_input::WinInput;
use super::super::domain::windows::{
    quick_times, DestForm, KeyAnswer, TimeForm, ToolFrame, ToolWindow, TourForm,
};
use super::super::interface::output::{game_lists, App, ListKind, Move, PlayerDuty};
use glam::Vec2;
use omsi_sim::SimClock;
use omsi_ui::alt_windows::{self as aw, Event, View};
use omsi_ui::widgets::Ui;
use winit::keyboard::KeyCode;

impl ToolFrame {
    pub(crate) fn draw(&self, ui: &mut Ui, size: Vec2) -> Option<Event> {
        self.view.draw(ui, size, self.key)
    }

    pub(crate) fn rect(&self, size: Vec2) -> omsi_ui::Rect {
        self.view.rect(size)
    }
}

/// Why the timetable evaluation cannot be opened.
const NO_TOUR: &str = "No tour running";

/// The stops of the duty's current trip the timetable window lists (a depot run's stations
/// the bus passes are none), by their place in the trip.
fn running_stops(d: &PlayerDuty) -> Vec<usize> {
    d.trip()
        .stops
        .iter()
        .enumerate()
        .filter(|(_, s)| s.stops)
        .map(|(k, _)| k)
        .collect()
}

/// Seconds of the day as "hh:mm".
fn hm(secs: f64) -> String {
    let m = (secs.max(0.0) / 60.0) as i64;
    format!("{:02}:{:02}", (m / 60) % 24, m % 60)
}

/// The rows of one of the game's lists that do something of `verb` (the others are its
/// "Back" and its notes), as (label, the rest of the action).
fn rows_of(app: &App, kind: &ListKind, verb: &str) -> Vec<(String, String)> {
    let prefix = format!("{verb} ");
    game_lists::items(app, kind)
        .into_iter()
        .filter_map(|(label, action)| {
            action
                .strip_prefix(&prefix)
                .map(|rest| (label, rest.to_string()))
        })
        .collect()
}

impl App {
    /// Whether one of the bar's windows is showing: it is open and neither the Esc menu nor
    /// the end-of-trip report nor a layout of its own (triple screen, headset) is in the way.
    pub(crate) fn tool_window_active(&self) -> bool {
        self.new_ui.tool.window.is_some()
            && self.new_ui.report.is_none()
            && self.game_menu.is_none()
            && self.menu.is_none()
            && !self.vr_active()
            && !self.settings.triple.enabled
    }

    /// Open the window of the bar's button `id`. Opening a window never pauses the game.
    pub(crate) fn open_tool(&mut self, id: &str) {
        let w = match id {
            "reposition" => ToolWindow::Reposition {
                names: rows_of(self, &ListKind::Spots, "spot")
                    .into_iter()
                    .map(|r| r.0)
                    .collect(),
                selected: None,
            },
            "time" => {
                let mut f = TimeForm::new(&self.clock, self.settings.time_sync);
                f.quick = quick_times(&self.clock);
                ToolWindow::Time(f)
            }
            "weather" => {
                let now = game_lists::weather_name(self);
                let files: Vec<(String, String)> = game_lists::weather_files()
                    .into_iter()
                    .map(|f| {
                        let file = f.rsplit('/').next().unwrap_or(&f);
                        let stem = file.rsplit_once('.').map(|x| x.0).unwrap_or(file);
                        (stem.trim_start_matches('#').to_string(), f.clone())
                    })
                    .collect();
                let selected = files.iter().position(|f| f.0.eq_ignore_ascii_case(&now));
                let real_label = format!(
                    "{}  \u{b7}  {}",
                    omsi_ui::tr("Real weather (METAR)"),
                    self.metar_station()
                );
                let mut stations = vec![(
                    String::new(),
                    omsi_ui::tr("Automatic (nearest the map)").to_string(),
                )];
                stations.extend(crate::weather_setup::metar_airports(&self.args.root));
                let own = &self.settings.metar_station;
                let station = Some(
                    stations
                        .iter()
                        .position(|s| !own.is_empty() && s.0.eq_ignore_ascii_case(own))
                        .unwrap_or(0),
                );
                ToolWindow::Weather {
                    files,
                    selected,
                    real: self.settings.metar_sync,
                    real_label,
                    stations,
                    station,
                    station_before: station,
                }
            }
            "controls" => ToolWindow::Controls,
            "tour" if self.duty.is_some() => ToolWindow::Running,
            "tour" => ToolWindow::Tour(TourForm {
                lines: rows_of(self, &ListKind::Lines, "line"),
                ..Default::default()
            }),
            "destination" => ToolWindow::Destination(self.destination_form()),
            "service" => ToolWindow::Service,
            "driver" => ToolWindow::DriverMenu,
            _ => return,
        };
        self.new_ui.close_add();
        self.new_ui.close_tool();
        self.new_ui.tool.open(w);
    }

    fn destination_form(&self) -> DestForm {
        let line = self
            .player
            .as_ref()
            .and_then(|p| game_lists::line_shown(&p.vehicle))
            .unwrap_or_default();
        let destinations: Vec<(String, String, usize)> =
            rows_of(self, &ListKind::Destinations, "dest")
                .into_iter()
                .filter_map(|(label, i)| {
                    let i: usize = i.trim().parse().ok()?;
                    // (the list's label: the code, right-aligned, and the name)
                    let label = label.trim_start();
                    let (code, name) = label.split_once("  ").unwrap_or(("", label));
                    Some((code.trim().to_string(), name.trim().to_string(), i))
                })
                .collect();
        let shown = self.player.as_ref().and_then(|p| {
            let hof = p.vehicle.host.hof.as_deref()?;
            crate::schedule::shown_destination(&p.vehicle, hof, p.blind_pick.as_ref())
        });
        let selected = shown.and_then(|ti| destinations.iter().position(|d| d.2 == ti));
        DestForm {
            line_before: line.clone(),
            line,
            focused: false,
            destinations,
            selected,
            selected_before: selected,
            empty: "This bus has no depot file (.hof) with destinations".into(),
        }
    }

    /// The window as it is drawn now.
    pub(crate) fn tool_frame(&self) -> Option<ToolFrame> {
        if !self.tool_window_active() {
            return None;
        }
        let w = self.new_ui.tool.window.as_ref()?;
        let view = match w {
            ToolWindow::Reposition { names, selected } => View::Pick(aw::Pick {
                title: "Reposition at an entry point".into(),
                head: "Entry point".into(),
                rows: names.clone(),
                selected: *selected,
                ok: "Reposition".into(),
                hint: "The bus is put at the entry point".into(),
                empty: "This map has no entry points".into(),
            }),
            ToolWindow::Time(f) => View::Time(f.view()),
            ToolWindow::Weather {
                files,
                selected,
                real,
                real_label,
                stations,
                station,
                ..
            } => View::Weather(aw::Weather {
                real: *real,
                real_label: real_label.clone(),
                presets: files.iter().map(|f| f.0.clone()).collect(),
                selected: *selected,
                stations: stations.iter().map(|s| s.1.clone()).collect(),
                station: *station,
            }),
            ToolWindow::Controls => View::Menu(self.controls_menu()),
            ToolWindow::Tour(f) => View::Tour(f.view()),
            ToolWindow::Running => View::Running(self.running()),
            ToolWindow::EndTour => View::Confirm(aw::Confirm {
                title: "Cancel the timetable?".into(),
                // (one text with the line in its place, so every language can put it where
                // its own word order wants it)
                text: omsi_ui::tr(
                    "The bus stops following line {}. This does not change the saved timetable.",
                )
                .replacen(
                    "{}",
                    self.duty
                        .as_ref()
                        .map(|d| d.line.trim())
                        .unwrap_or_default(),
                    1,
                ),
                ok: "Yes, cancel it".into(),
                cancel: "Keep it".into(),
            }),
            ToolWindow::Destination(d) => View::Destination(aw::Destination {
                line: d.line.clone(),
                focused: d.focused,
                codes: d.destinations.iter().map(|x| x.0.clone()).collect(),
                destinations: d.destinations.iter().map(|x| x.1.clone()).collect(),
                selected: d.selected,
                empty: d.empty.clone(),
            }),
            ToolWindow::Service => View::Menu(self.service_menu()),
            ToolWindow::Evaluation { trip } => View::Evaluation(self.evaluation(*trip)),
            ToolWindow::DriverMenu => View::Menu(aw::Menu {
                title: "Timetable and driver".into(),
                entries: vec![
                    aw::Entry {
                        label: "Timetable evaluation".into(),
                        checked: None,
                        disabled: self.duty.is_none().then(|| NO_TOUR.to_string()),
                    },
                    aw::Entry {
                        label: "Driver profile".into(),
                        checked: None,
                        disabled: crate::input::on_server(&self.args)
                            .then(|| "Not on a server".to_string()),
                    },
                    aw::Entry {
                        label: "Change the current driver profile".into(),
                        checked: None,
                        disabled: crate::input::on_server(&self.args)
                            .then(|| "Not on a server".to_string()),
                    },
                ],
                hint: "Each trip's report opens at its last stop".into(),
            }),
            ToolWindow::Profile => View::Profile(self.profile_view()),
            ToolWindow::Driver { names, selected } => View::Pick(aw::Pick {
                title: "Driver profile".into(),
                head: "Driver".into(),
                rows: names.clone(),
                selected: *selected,
                ok: "Choose".into(),
                hint: "The run goes on in the chosen driver's file".into(),
                empty: "No driver profiles found".into(),
            }),
        };
        Some(ToolFrame {
            view,
            key: self.new_ui.tool.opened,
        })
    }

    /// The driver's personnel file as it is saved (what the launcher's Profile page adds up
    /// from the same file), for the driver profile window.
    fn profile_view(&self) -> aw::Profile {
        let c = &self.career;
        let mut rows: Vec<(String, String)> = Vec::new();
        let name = c
            .driver
            .as_ref()
            .map(|d| d.name.clone())
            .or_else(|| {
                c.path
                    .as_ref()
                    .and_then(|p| p.file_stem())
                    .map(|s| s.to_string_lossy().to_string())
            })
            .unwrap_or_default();
        let mut row = |k: &str, v: String| rows.push((omsi_ui::tr(k).to_string(), v));
        row("Driver", name);
        if let Some(d) = c.driver.as_ref() {
            let served = d.bus_stops[0];
            let (early, late) = (d.bus_stops[1], d.bus_stops[2]);
            row("Distance", format!("{:.1} km", d.hektom / 10.0));
            row("Stops served", served.to_string());
            row(
                "On time / early / late",
                format!("{} / {early} / {late}", (served - early - late).max(0)),
            );
            row("Tickets sold", format!("{:.0}", d.tickets[0]));
            row("Takings", format!("{:.2}", d.tickets[1]));
            row("Crashes", d.crashes[0].to_string());
            row("Pedestrians hurt", d.crashes[1].to_string());
            // (the rating's counters as the file keeps them: the penalty, the people content,
            // the tickets asked for and their points, the people who stepped in)
            let r = &d.rating;
            let per = |num: f64, den: f64| if den > 0.0 { 100.0 * num / den } else { 100.0 };
            row(
                "Driving",
                format!("{:.0} %", 100.0 * (1.0 - r[0].clamp(0.0, 1.0))),
            );
            row("Comfort", format!("{:.0} %", per(r[1], r[4])));
            row("Ticket sales", format!("{:.0} %", per(r[3], 2.0 * r[2])));
        } else {
            row("Driving", format!("{:.0} %", c.driving_rating()));
        }
        aw::Profile {
            title: "Driver profile".into(),
            rows,
            hint: "as saved after the drive".into(),
        }
    }

    /// The wash and the pump as they are now: each ticked while it runs, dim away from a
    /// petrol station (one running can always be stopped).
    fn service_menu(&self) -> aw::Menu {
        let at = self.player.is_some() && self.at_service_station();
        let entry = |id: &str, label: &str| {
            let on = self.pumping.is_some_and(|p| p.0 == id);
            let off = if self.player.is_none() {
                Some("No vehicle to service")
            } else if !at && !on {
                Some(super::super::domain::alt::NO_STATION)
            } else {
                None
            };
            aw::Entry {
                label: label.to_string(),
                checked: Some(on),
                disabled: off.map(String::from),
            }
        };
        aw::Menu {
            title: "Wash and refuel".into(),
            entries: vec![entry("wash", "Wash"), entry("refuel", "Refuel")],
            hint: "Each runs until it is done or the bus drives off".into(),
        }
    }

    /// The timetable running: the current trip's stops with how each went, the one due now,
    /// and a button to skip each still to come.
    fn running(&self) -> aw::Running {
        let Some(d) = self.duty.as_ref() else {
            return aw::Running::default();
        };
        let trip = d.trip();
        let done = d.trip_done();
        let rows = running_stops(d)
            .into_iter()
            .map(|k| {
                let s = &trip.stops[k];
                let t = hm(if k == 0 { s.dep } else { s.arr });
                let marked = d.skip_marked(k);
                let served = || {
                    self.journey
                        .as_ref()
                        .is_some_and(|j| j.served((d.trip_index, k)))
                };
                let state = if done || k < d.next_stop {
                    if served() {
                        aw::RunStop::Done
                    } else {
                        aw::RunStop::Passed
                    }
                } else if k == d.next_stop {
                    aw::RunStop::Next
                } else {
                    aw::RunStop::Ahead
                };
                let detail = match state {
                    aw::RunStop::Done | aw::RunStop::Passed => {
                        let how = self
                            .journey
                            .as_ref()
                            .and_then(|j| j.how((d.trip_index, k)))
                            .unwrap_or_else(|| "not served".into());
                        format!("{t}  \u{b7}  {}", omsi_ui::tr(&how))
                    }
                    aw::RunStop::Next => format!("{t}  \u{b7}  {}", omsi_ui::tr("next stop")),
                    aw::RunStop::Ahead if marked => {
                        format!("{t}  \u{b7}  {}", omsi_ui::tr("to be skipped"))
                    }
                    aw::RunStop::Ahead => t,
                };
                let skip = match state {
                    aw::RunStop::Done | aw::RunStop::Passed => None,
                    aw::RunStop::Ahead if marked => Some("Keep this stop".to_string()),
                    _ => Some("Skip this stop".to_string()),
                };
                aw::RunRow {
                    stop: s.name.trim().to_string(),
                    detail,
                    state,
                    skip,
                }
            })
            .collect();
        aw::Running {
            sub: format!(
                "{} {}  \u{b7}  {} {}",
                omsi_ui::tr("Line"),
                d.line.trim(),
                omsi_ui::tr("trip"),
                hm(trip.departure)
            ),
            rows,
            hint: "Skipping a stop tells the timetable you did not stop there for a reason on the trip. In the evaluation it counts with no mark: neither missed nor late.".into(),
        }
    }

    /// The timetable evaluation of trip `trip` of the duty: its stops with their planned and
    /// actual times as the journey log has them so far (the end-of-trip report's rows).
    fn evaluation(&self, trip: usize) -> aw::Evaluation {
        let Some(d) = self.duty.as_ref() else {
            return aw::Evaluation::default();
        };
        let last = d.trip_index.min(d.trips.len().saturating_sub(1));
        let trip = trip.min(last);
        let (title, rows, totals) = self
            .journey
            .as_ref()
            .and_then(|j| j.report(trip))
            .unwrap_or_default();
        // (how far off the bus was at the last stop it has been at)
        let now = rows
            .iter()
            .rev()
            .find(|r| r.state != omsi_ui::ingame::StopState::None && !r.deviation.is_empty())
            .map(|r| r.deviation.clone())
            .unwrap_or_else(|| "\u{2013}".to_string());
        let mut totals: Vec<(String, String)> = totals
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        totals.insert(0, ("Deviation at the last stop reached".into(), now));
        aw::Evaluation {
            title,
            // (counted as the trips' titles are: in the tour)
            trip: format!(
                "{} {} / {}",
                omsi_ui::tr("Trip"),
                d.first_trip + trip + 1,
                d.first_trip + d.trips.len()
            ),
            trip_steps: (trip > 0, trip < last),
            rows,
            totals,
            hint: "Updated at every stop  ·  the journey log is in Journeys".into(),
        }
    }

    /// The controls' entries as they are now: on, off, or dim with the reason.
    fn controls_menu(&self) -> aw::Menu {
        let found = self.controllers.as_ref().is_some_and(|c| c.any());
        let controller = found && self.controllers.as_ref().is_some_and(|c| c.enabled);
        let mouse = self.mouse_drive;
        let entry = |label: &str, on: bool, off: Option<&str>| aw::Entry {
            label: label.to_string(),
            checked: Some(on),
            disabled: off.map(String::from),
        };
        aw::Menu {
            title: "Controls".into(),
            entries: vec![
                entry(
                    "Game controller",
                    controller,
                    (!found).then_some("No game controller found"),
                ),
                entry("Mouse steering", mouse, None),
                entry("Keyboard only", !controller && !mouse, None),
                entry(
                    "Head tracking",
                    self.settings.head_tracking,
                    self.vr_active().then_some("The headset tracks the head"),
                ),
            ],
            hint: "Each choice applies at once".into(),
        }
    }

    /// Whether the cursor is over the window.
    fn over_tool_window(&self) -> bool {
        let (Some(s), Some(frame)) = (self.surface.as_ref(), self.tool_frame()) else {
            return false;
        };
        let (w, h) = (s.config.width, s.config.height);
        let scale = scale_for(w, h);
        let rect = frame.rect(Vec2::new(w as f32, h as f32) / scale);
        rect.contains(Vec2::new(self.cursor.0, self.cursor.1) / scale)
    }

    /// A press or release of the left button. True when the window takes it: a press over the
    /// window, and the release of a press it took. The game does not see it then.
    pub(crate) fn tool_click(&mut self, pressed: bool) -> bool {
        if !self.tool_window_active() {
            self.new_ui.tool.pressed = false;
            return false;
        }
        let over = self.over_tool_window();
        let tool = &mut self.new_ui.tool;
        if pressed {
            tool.pressed = over;
            if !over {
                // (a click in the world: the route number's field lets go of the keyboard)
                if let Some(ToolWindow::Destination(d)) = tool.window.as_mut() {
                    d.focused = false;
                }
                return false;
            }
            self.new_ui.push_input(WinInput::Press);
            true
        } else if std::mem::take(&mut tool.pressed) {
            self.new_ui.push_input(WinInput::Release);
            true
        } else {
            false
        }
    }

    /// The wheel over the window scrolls its list or steps its value.
    pub(crate) fn tool_wheel(&mut self, amount: f32) -> bool {
        if !self.tool_window_active() || !self.over_tool_window() {
            return false;
        }
        self.new_ui.push_input(WinInput::Wheel(amount));
        true
    }

    /// A key, before the game sees it. True when the window takes it.
    pub(crate) fn tool_key(&mut self, code: KeyCode, pressed: bool) -> bool {
        if pressed && !self.tool_window_active() {
            return false;
        }
        match self.new_ui.tool.key(code, pressed) {
            KeyAnswer::Game => false,
            KeyAnswer::Taken => true,
            KeyAnswer::Close => {
                self.new_ui.close_tool();
                true
            }
            KeyAnswer::Ok => {
                self.tool_event(Event::Ok);
                true
            }
        }
    }

    /// Text typed while the window's field has the keyboard. True when it took it.
    pub(crate) fn tool_text(&mut self, text: &str, code: Option<KeyCode>) -> bool {
        if !self.tool_window_active() || !text.chars().any(|c| !c.is_control()) {
            return false;
        }
        let tool = &mut self.new_ui.tool;
        let Some(ToolWindow::Destination(d)) = tool.window.as_mut().filter(|w| w.typing()) else {
            return false;
        };
        d.type_text(text);
        if let Some(code) = code {
            tool.taken.insert(code);
        }
        true
    }

    /// Apply what the clicks in the window asked for (before each frame).
    pub(crate) fn tool_events(&mut self) {
        // (the timetable evaluation, the running timetable and its "Cancel the timetable?"
        // are the running tour's: they close with the tour, however it ended)
        if self.duty.is_none()
            && matches!(
                self.new_ui.tool.window,
                Some(ToolWindow::Evaluation { .. } | ToolWindow::Running | ToolWindow::EndTour)
            )
        {
            self.new_ui.close_tool();
        }
        for e in self.new_ui.take_tool_events() {
            if self.new_ui.tool.window.is_none() {
                return;
            }
            self.tool_event(e);
        }
    }

    fn tool_event(&mut self, e: Event) {
        let Some(w) = self.new_ui.tool.window.as_mut() else {
            return;
        };
        if e == Event::Cancel {
            self.new_ui.close_tool();
            return;
        }
        match w {
            ToolWindow::Reposition { selected, .. } => match e {
                Event::Row(_, i) => *selected = Some(i),
                Event::Ok => {
                    let Some(i) = *selected else { return };
                    self.new_ui.close_tool();
                    let spot = rows_of(self, &ListKind::Spots, "spot")
                        .get(i)
                        .map(|r| r.1.clone());
                    if let Some(spot) = spot {
                        game_lists::run(self, &ListKind::Spots, &format!("spot {spot}"));
                    }
                }
                _ => {}
            },
            ToolWindow::Time(f) => match e {
                Event::Step(k, by) => {
                    f.step(k, by);
                    // (dawn and dusk are the chosen day's)
                    if k >= 2 {
                        f.quick = quick_times(&f.clock(&self.clock));
                    }
                }
                Event::Quick(i) => f.choose_quick(i),
                Event::Check(_) => f.real = !f.real,
                Event::Ok => {
                    let f = f.clone();
                    self.new_ui.close_tool();
                    self.apply_time(&f);
                }
                _ => {}
            },
            ToolWindow::Weather {
                files,
                selected,
                real,
                stations,
                station,
                station_before,
                ..
            } => match e {
                Event::Row(0, i) => *selected = Some(i),
                Event::Row(_, i) => *station = Some(i),
                Event::Check(_) => *real = !*real,
                Event::Ok => {
                    let (real, file) = (
                        *real,
                        selected.and_then(|i| files.get(i)).map(|f| f.1.clone()),
                    );
                    // (the airport, when the player chose another: "" is the automatic one)
                    let airport = (*station != *station_before)
                        .then(|| station.and_then(|i| stations.get(i)))
                        .flatten()
                        .map(|s| s.0.clone());
                    self.new_ui.close_tool();
                    if let Some(code) = airport {
                        game_lists::dropdown_apply(self, &format!("metar_src {code}"));
                    }
                    self.apply_weather(real, file);
                }
                _ => {}
            },
            ToolWindow::Controls => {
                if let Event::Check(i) = e {
                    self.toggle_control(i);
                }
            }
            ToolWindow::Tour(_) => self.tour_event(e),
            ToolWindow::Running => match e {
                Event::Row(_, i) => {
                    if let Some(k) = self
                        .duty
                        .as_ref()
                        .and_then(|d| running_stops(d).get(i).copied())
                    {
                        self.skip_stop_at(k);
                    }
                }
                Event::Check(_) => self.new_ui.tool.open(ToolWindow::EndTour),
                _ => {}
            },
            ToolWindow::EndTour => {
                if e == Event::Ok {
                    self.new_ui.close_tool();
                    game_lists::end_duty(self);
                }
            }
            ToolWindow::Destination(d) => {
                d.focused = e == Event::Field(0);
                match e {
                    Event::Row(_, i) => d.selected = Some(i),
                    Event::Ok => {
                        let (line, dest) = d.changes();
                        self.new_ui.close_tool();
                        if let Some(ti) = dest {
                            game_lists::run(self, &ListKind::Destinations, &format!("dest {ti}"));
                        }
                        if let Some(line) = line {
                            game_lists::set_route_by_hand(self, &line);
                        }
                    }
                    _ => {}
                }
            }
            ToolWindow::Service => {
                if let Event::Check(i) = e {
                    let id = if i == 0 { "wash" } else { "refuel" };
                    self.toggle_service(id);
                }
            }
            ToolWindow::Evaluation { trip } => {
                if let Event::Step(_, by) = e {
                    let last = self.duty.as_ref().map_or(0, |d| d.trip_index);
                    *trip = (*trip as i64 + by as i64).clamp(0, last as i64) as usize;
                }
            }
            // (it only shows: Close is handled with every window's Cancel)
            ToolWindow::Profile => {}
            ToolWindow::DriverMenu => {
                if e == Event::Check(0) {
                    if let Some(d) = self.duty.as_ref() {
                        let trip = d.trip_index;
                        self.new_ui.tool.open(ToolWindow::Evaluation { trip });
                    }
                } else if e == Event::Check(1) {
                    self.new_ui.tool.open(ToolWindow::Profile);
                } else if e == Event::Check(2) {
                    let names = game_lists::driver_names(self);
                    let now = self
                        .career
                        .path
                        .as_ref()
                        .and_then(|p| p.file_stem())
                        .map(|s| s.to_string_lossy().to_string());
                    let selected =
                        now.and_then(|n| names.iter().position(|x| x.eq_ignore_ascii_case(&n)));
                    self.new_ui
                        .tool
                        .open(ToolWindow::Driver { names, selected });
                }
            }
            ToolWindow::Driver { names, selected } => match e {
                Event::Row(_, i) => *selected = Some(i),
                Event::Ok => {
                    let name = selected.and_then(|i| names.get(i)).cloned();
                    self.new_ui.close_tool();
                    if let Some(name) = name {
                        game_lists::switch_driver(self, &name);
                    }
                }
                _ => {}
            },
        }
    }

    /// A click in the line and tour window.
    fn tour_event(&mut self, e: Event) {
        let Some(ToolWindow::Tour(f)) = self.new_ui.tool.window.as_mut() else {
            return;
        };
        let mut f = std::mem::take(f);
        match e {
            Event::Row(0, i) if i < f.lines.len() => {
                f.line = Some(i);
                f.tour = None;
                f.stops.clear();
                f.stop = None;
                f.trips = 0;
                f.tours = self.tours_of(&f.lines[i].1);
            }
            Event::Row(1, i) if i < f.tours.len() => {
                f.tour = Some(i);
                if let Some((line, tour)) = f.chosen() {
                    let now = self.clock.time;
                    let sch = self.schedule.as_ref();
                    f.trips = sch.map_or(0, |s| s.tour_trip_count(&line, &tour));
                    f.trip = sch.map_or(0, |s| s.tour_trip_now(&line, &tour, now));
                    f.trip = f.trip.min(f.trips.saturating_sub(1));
                    self.fill_stops(&mut f);
                }
            }
            Event::Row(2, i) if i < f.stops.len() => f.stop = Some(i),
            Event::Step(_, by) if f.trips > 0 => {
                let to = (f.trip as i64 + by as i64).clamp(0, f.trips as i64 - 1) as usize;
                if to != f.trip {
                    f.trip = to;
                    self.fill_stops(&mut f);
                }
            }
            Event::Ok => {
                if let (Some((line, tour)), Some(stop)) = (f.chosen(), f.stop) {
                    self.new_ui.close_tool();
                    game_lists::start_duty_at(self, &line, &tour, f.trip, stop);
                    return;
                }
            }
            _ => {}
        }
        if let Some(ToolWindow::Tour(slot)) = self.new_ui.tool.window.as_mut() {
            *slot = f;
        }
    }

    /// The tours of line `line` running now, as the game menu's list of tours has them, with
    /// the time each leaves next and where it goes (as the line's preview shows them, matched
    /// by the tour's number).
    fn tours_of(&self, line: &str) -> Vec<(String, String, String)> {
        let sch = self.schedule.as_ref();
        let found = sch.and_then(|s| s.data.lines.iter().find(|l| l.name == line));
        rows_of(self, &ListKind::Tours(line.to_string(), None), "tour")
            .into_iter()
            .filter_map(|(label, rest)| {
                let (_, number) = rest.split_once('\u{1}')?;
                let tour = found.and_then(|l| l.tours.iter().find(|t| t.number == number));
                let (what, when) = match (sch, tour) {
                    (Some(s), Some(t)) => game_lists::tour_preview(s, line, t, self.clock.time),
                    _ => (label, String::new()),
                };
                Some((what, when, number.to_string()))
            })
            .collect()
    }

    /// The stops of the trip chosen in the line and tour window; the first is chosen.
    fn fill_stops(&self, f: &mut TourForm) {
        let Some((line, tour)) = f.chosen() else {
            return;
        };
        f.stops = self
            .schedule
            .as_ref()
            .map(|s| s.tour_trip_stops(&line, &tour, f.trip))
            .unwrap_or_default()
            .into_iter()
            .map(|(_, _, name, at)| (name.trim().to_string(), hm(at)))
            .collect();
        f.stop = (!f.stops.is_empty()).then_some(0);
    }

    /// A switch of the wash and refuel menu: the wash or the pump starts (in place of the
    /// other, if that was running), or stops if it was running. It stops by itself once done,
    /// and when the bus drives off (`App::tick_service`).
    fn toggle_service(&mut self, id: &str) {
        if self.pumping.is_some_and(|p| p.0 == id) {
            self.pumping = None;
            let msg = if id == "refuel" {
                "Refuelling stopped"
            } else {
                "Washing stopped"
            };
            self.service_msg = Some((msg.into(), 3.0));
        } else if self.at_service_station() {
            self.pumping = None;
            self.run_service(id);
        }
    }

    /// Entry `i` of the controls: the game controller, the mouse steering, the keyboard only
    /// (both off) or the head tracking, switched as their keys and settings do.
    fn toggle_control(&mut self, i: usize) {
        match i {
            0 => {
                self.game_action("toggel_ctrler");
            }
            1 => {
                self.game_action("toggel_mouse_ctrl");
            }
            2 => {
                if let Some(c) = self.controllers.as_mut() {
                    c.enabled = false;
                }
                if self.mouse_drive {
                    self.set_mouse_drive(false);
                }
                self.service_msg = Some(("Keyboard only".into(), 3.0));
            }
            3 if !self.vr_active() => {
                game_lists::option_do(self, "headtrack", "", Move::Next);
                let msg = if self.settings.head_tracking {
                    "Head tracking on"
                } else {
                    "Head tracking off"
                };
                self.service_msg = Some((msg.into(), 3.0));
            }
            _ => {}
        }
    }

    /// OK in the date and time window: the real-time sync as ticked, else the clock set to the
    /// date and time chosen.
    fn apply_time(&mut self, f: &TimeForm) {
        if self.is_lan_client() {
            self.service_msg = Some(("In a LAN session the host sets the clock".into(), 3.0));
            return;
        }
        if f.real != self.settings.time_sync {
            let to = if f.real { Move::Inc } else { Move::Dec };
            game_lists::option_do(self, "time_sync", "", to);
        }
        if !f.real {
            self.set_date_time(f.clock(&self.clock));
        }
    }

    /// Set the clock to `to` (date and time), the traffic's clock with it, as moving it does
    /// (`App::shift_clock`).
    fn set_date_time(&mut self, to: SimClock) {
        let gap = crate::real_time::gap(&self.clock, &to);
        if gap.abs() < 1.0 || self.real_time_locked() {
            return;
        }
        self.clock.year = to.year;
        self.clock.day_of_year = to.day_of_year;
        self.clock.time = to.time;
        if let Some(t) = self.traffic.as_mut() {
            t.day_time += gap;
        }
        if let Some(p) = self.player.as_mut() {
            p.vehicle.host.clock = self.clock.clone();
        }
        let (d, m) = self.clock.day_month();
        self.service_msg = Some((
            omsi_ui::tr("Clock: %{date} %{time}")
                .replace("%{date}", &format!("{:04}-{m:02}-{d:02}", self.clock.year))
                .replace("%{time}", &hm(self.clock.time)),
            3.0,
        ));
        self.clock_jump += gap;
        if self.clock_hold == 0.0 {
            self.timetable_after_clock_jump();
        }
    }

    /// OK in the weather window: the real weather as ticked, else the weather file chosen.
    fn apply_weather(&mut self, real: bool, file: Option<String>) {
        if self.is_lan_client() {
            self.service_msg = Some(("In a LAN session the host sets the weather".into(), 3.0));
            return;
        }
        if real != self.settings.metar_sync {
            let to = if real { Move::Inc } else { Move::Dec };
            game_lists::option_do(self, "metar_sync", "", to);
        }
        if real {
            return;
        }
        let now = game_lists::weather_name(self);
        if let Some(file) = file {
            let stem = file.rsplit('/').next().unwrap_or(&file);
            let stem = stem.rsplit_once('.').map(|x| x.0).unwrap_or(stem);
            if !stem.trim_start_matches('#').eq_ignore_ascii_case(&now) {
                game_lists::dropdown_apply(self, &format!("wx {file}"));
            }
        }
    }
}
