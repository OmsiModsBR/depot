//! The end-of-trip report of the new interface.
//!
//! It opens by itself once the bus has stood at the last stop of its trip with a passenger
//! door open, and pauses the simulation (not in a LAN session, which runs on for the other
//! players), as the Esc menu does. It shows the trip's stops with their planned and actual
//! times as the journey log has them. "Continue", Enter, Space or Esc closes it and the game
//! goes on as it was. On a triple screen it stands on the centre screen, as the Esc menu does;
//! the headset has no report yet (the journey log has the same in the content folder's
//! `Journeys`).

use crate::app::App;
use crate::new_ui_add::WinInput;
use glam::Vec2;
use omsi_ui::ingame::{self, ReportRow, TripReport};
use omsi_ui::widgets::Ui;
use winit::keyboard::KeyCode;

/// What the overlay draws of the report, owned so that it can cross to the renderer.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ReportFrame {
    title: String,
    note: &'static str,
    rows: Vec<ReportRow>,
    totals: Vec<(&'static str, String)>,
}

impl ReportFrame {
    /// Draw it; true when "Continue" was clicked.
    pub(crate) fn draw(&self, ui: &mut Ui, size: Vec2) -> bool {
        ingame::trip_report(
            ui,
            size,
            &TripReport {
                title: &self.title,
                note: self.note,
                rows: &self.rows,
                totals: &self.totals,
            },
        )
    }
}

#[cfg(test)]
impl ReportFrame {
    /// A trip of three stops, for the overlay's tests.
    pub(crate) fn for_test() -> Self {
        let row = |stop: &str, at: &str, state| ReportRow {
            stop: stop.into(),
            scheduled: at.into(),
            actual: at.into(),
            deviation: "0:00".into(),
            state,
        };
        ReportFrame {
            title: "50 · Central Station".into(),
            note: "paused",
            rows: vec![
                row("Depot", "08:44", ingame::StopState::OnTime),
                row("Market", "08:47", ingame::StopState::OnTime),
                row("Central Station", "08:54", ingame::StopState::OnTime),
            ],
            totals: vec![("stops served", "3".into())],
        }
    }
}

/// The report on the game's side.
#[derive(Debug)]
pub(crate) struct Report {
    frame: ReportFrame,
    /// Whether the game was paused before the report paused it.
    paused_before: bool,
    /// A press was taken: its release goes to the report too.
    pressed: bool,
}

impl Report {
    /// The window lost the mouse: the release of a press it took will not come.
    pub(crate) fn forget_press(&mut self) {
        self.pressed = false;
    }
}

/// A trip at its last stop: (line, tour, trip in the tour), its place in the duty, where
/// the stop is.
pub(crate) type Watch = (TripKey, usize, glam::DVec3);

/// Within this of the last stop, stopped with a passenger door open, the trip is over (the
/// duty's own distance for being at a stop).
const AT_TERMINUS: f64 = 25.0;
/// Further than this from it, the bus has left without opening (no report).
const LEFT_TERMINUS: f64 = 60.0;

/// What becomes of the watch on a trip's end: `Some(true)` the report opens, `Some(false)`
/// the watch ends without one, `None` it goes on. `still_done`: the duty is still on that
/// trip, at its end.
fn watch_outcome(distance: f64, doors: bool, still_done: bool) -> Option<bool> {
    if distance < AT_TERMINUS && doors {
        Some(true)
    } else if distance > LEFT_TERMINUS && !still_done {
        Some(false)
    } else {
        None
    }
}

/// A trip: (line, tour, trip in the tour).
pub(crate) type TripKey = (String, String, usize);

/// One duty update for the watch: `done` the trip at its last stop (if the duty is on one),
/// `shown` the trip whose report has opened last, `bus` where the bus is, `doors` whether it
/// stands with a passenger door open. The trip whose report opens now, with its place in the
/// duty.
fn watch_step(
    watch: &mut Option<Watch>,
    shown: Option<&TripKey>,
    done: Option<Watch>,
    bus: glam::DVec3,
    doors: bool,
) -> Option<(TripKey, usize)> {
    let still_done = done.is_some();
    if let Some(w) = done {
        if shown != Some(&w.0) {
            *watch = Some(w);
        }
    }
    let (_, _, at) = watch.as_ref()?;
    match watch_outcome((bus - *at).length(), doors, still_done)? {
        true => watch.take().map(|(key, trip, _)| (key, trip)),
        false => {
            *watch = None;
            None
        }
    }
}

impl App {
    /// After each duty update (see [`watch_step`]). The report opens once the bus stands at
    /// the trip's last stop with a door open, even when the duty has gone on to the next trip
    /// meanwhile (a late arrival).
    pub(crate) fn trip_watch(&mut self, done: Option<Watch>, bus: glam::DVec3, doors: bool) {
        let ui = &mut self.new_ui;
        if let Some((key, trip)) = watch_step(
            &mut ui.report_watch,
            ui.report_shown.as_ref(),
            done,
            bus,
            doors,
        ) {
            self.trip_finished(key, trip);
        }
    }

    /// A duty taken or given up: its trips are new ones (their reports open again, and the
    /// journey log starts a file of its own even for the same line and tour).
    pub(crate) fn duty_changed(&mut self) {
        self.journey = None;
        self.new_ui.report_shown = None;
        self.new_ui.report_watch = None;
    }

    /// Whether the report is showing (not in the headset, with no Esc menu over it).
    pub(crate) fn report_active(&self) -> bool {
        self.new_ui.report.is_some()
            && self.game_menu.is_none()
            && self.menu.is_none()
            && !self.vr_active()
    }

    /// Open the report of the trip `key` (line, tour, trip in the tour; `trip` its place in
    /// the duty), served to its end. Once a trip.
    pub(crate) fn trip_finished(&mut self, key: TripKey, trip: usize) {
        if self.new_ui.report_shown.as_ref() == Some(&key) {
            return;
        }
        self.new_ui.report_shown = Some(key);
        if self.vr_active() {
            return;
        }
        let Some((title, rows, totals)) = self.journey.as_ref().and_then(|j| j.report(trip)) else {
            return;
        };
        let lan = self.in_lan_session();
        // (the vehicle's keys and a switch held with the mouse are let go: their key-ups and
        // the button's release would come while the report has them)
        self.release_vehicle_keys();
        if let Some(p) = self.player.as_mut() {
            p.release();
        }
        self.dragging = false;
        self.new_ui.close_add();
        self.new_ui.clear_win_input();
        self.new_ui.report = Some(Report {
            frame: ReportFrame {
                title,
                note: if lan { "" } else { "paused" },
                rows,
                totals,
            },
            paused_before: self.paused,
            pressed: false,
        });
        if !lan {
            self.paused = true;
        }
    }

    /// Close the report: the game goes on as it was before it.
    pub(crate) fn close_report(&mut self) {
        if let Some(r) = self.new_ui.report.take() {
            if !self.in_lan_session() {
                self.paused = r.paused_before;
            }
        }
        self.new_ui.clear_win_input();
    }

    /// The report as it is drawn now.
    pub(crate) fn report_frame(&self) -> Option<ReportFrame> {
        self.report_active()
            .then(|| self.new_ui.report.as_ref().map(|r| r.frame.clone()))
            .flatten()
    }

    /// "Continue" clicked: close the report (before each frame).
    pub(crate) fn report_events(&mut self) {
        if self.new_ui.take_report_done() && self.new_ui.report.is_some() {
            self.close_report();
        }
    }

    /// A press or release of the left button while the report shows: the report takes them
    /// all (the game stands still behind it).
    pub(crate) fn report_click(&mut self, pressed: bool) -> bool {
        if !self.report_active() {
            return false;
        }
        let Some(r) = self.new_ui.report.as_mut() else {
            return false;
        };
        if pressed {
            r.pressed = true;
            self.new_ui.push_input(WinInput::Press);
        } else if std::mem::take(&mut r.pressed) {
            self.new_ui.push_input(WinInput::Release);
        }
        true
    }

    /// The wheel scrolls the report's stops.
    pub(crate) fn report_wheel(&mut self, amount: f32) -> bool {
        if !self.report_active() {
            return false;
        }
        self.new_ui.push_input(WinInput::Wheel(amount));
        true
    }

    /// The keys while the report shows: Enter, Space or Esc close it; the report takes every
    /// press (a key let go still reaches the game).
    pub(crate) fn report_key(&mut self, code: KeyCode, pressed: bool) -> bool {
        if !self.report_active() || !pressed {
            return false;
        }
        if matches!(
            code,
            KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space | KeyCode::Escape
        ) {
            self.close_report();
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_report_waits_for_the_doors_at_the_last_stop() {
        // standing there with a door open: the report
        assert_eq!(watch_outcome(5.0, true, false), Some(true));
        // there with the doors shut, the duty on the next trip already: still waiting
        assert_eq!(watch_outcome(5.0, false, false), None);
        // on the way to the stand, doors shut: waiting
        assert_eq!(watch_outcome(40.0, false, false), None);
        // gone without opening: no report
        assert_eq!(watch_outcome(80.0, false, false), Some(false));
        // still on that trip's end (a terminus with a layover far off): waiting
        assert_eq!(watch_outcome(80.0, false, true), None);
    }

    /// A late arrival: the duty is on the next trip before the doors open, and the report
    /// still opens, once.
    #[test]
    fn a_late_arrival_still_gets_its_report_once() {
        let key: TripKey = ("5".into(), "1".into(), 3);
        let stop = glam::DVec3::new(100.0, 0.0, 0.0);
        let near = glam::DVec3::new(95.0, 0.0, 0.0);
        let mut watch = None;
        // stopped at the last stop, doors shut: the trip is done
        assert_eq!(
            watch_step(&mut watch, None, Some((key.clone(), 2, stop)), near, false),
            None
        );
        // the duty goes on to the next trip (it is due): the doors open
        assert_eq!(watch_step(&mut watch, None, None, near, false), None);
        assert_eq!(
            watch_step(&mut watch, None, None, near, true),
            Some((key.clone(), 2))
        );
        assert!(watch.is_none());
        // a trip whose report has opened is not watched again
        assert_eq!(
            watch_step(
                &mut watch,
                Some(&key),
                Some((key.clone(), 2, stop)),
                near,
                true
            ),
            None
        );
        assert!(watch.is_none());
        // driven off without opening: no report later
        let next: TripKey = ("5".into(), "1".into(), 4);
        assert_eq!(
            watch_step(
                &mut watch,
                Some(&key),
                Some((next.clone(), 3, stop)),
                near,
                false
            ),
            None
        );
        assert_eq!(
            watch_step(
                &mut watch,
                Some(&key),
                None,
                glam::DVec3::new(200.0, 0.0, 0.0),
                false
            ),
            None
        );
        assert!(watch.is_none());
        assert_eq!(watch_step(&mut watch, Some(&key), None, near, true), None);
    }
}
