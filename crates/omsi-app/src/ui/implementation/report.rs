//! The end-of-trip report of the new interface.
//!
//! It opens by itself once the bus has stood at the last stop of its trip with a passenger
//! door open, and pauses the simulation (not in a LAN session, which runs on for the other
//! players), as the Esc menu does. It shows the trip's stops with their planned and actual
//! times as the journey log has them. "Continue", Enter, Space or Esc closes it and the game
//! goes on as it was. On a triple screen it stands on the centre screen, as the Esc menu does;
//! the headset has no report yet (the journey log has the same in the content folder's
//! `Journeys`).

use super::super::domain::report::{watch_step, Report, ReportFrame, TripKey, Watch};
use super::super::interface::output::{App, WinInput};
use glam::Vec2;
use omsi_ui::ingame::{self, TripReport};
use omsi_ui::widgets::Ui;
use winit::keyboard::KeyCode;

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
