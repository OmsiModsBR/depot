//! The end-of-trip report of the new interface.
//!
//! It opens by itself once the bus has stood at the last stop of its trip with a passenger
//! door open, and pauses the simulation (not in a LAN session, which runs on for the other
//! players), as the Esc menu does. It shows the trip's stops with their planned and actual
//! times as the journey log has them. "Continue", Enter, Space or Esc closes it and the game
//! goes on as it was. The triple screen and the headset have no report (their layouts are
//! their own); the journey log has the same in the content folder's `Journeys`.

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

impl App {
    /// Whether the report is showing (on the plain window, with no Esc menu over it).
    pub(crate) fn report_active(&self) -> bool {
        self.new_ui.report.is_some()
            && self.game_menu.is_none()
            && self.menu.is_none()
            && !self.vr_active()
            && !self.settings.triple.enabled
    }

    /// After each duty update: open the report when the trip `key` (line, tour, trip in the
    /// tour; `trip` its place in the duty) has just been served to its end. Once a trip.
    pub(crate) fn trip_finished(&mut self, key: (String, String, usize), trip: usize) {
        if self.new_ui.report_shown.as_ref() == Some(&key) {
            return;
        }
        self.new_ui.report_shown = Some(key);
        if self.vr_active() || self.settings.triple.enabled {
            return;
        }
        let Some((title, rows, totals)) = self.journey.as_ref().and_then(|j| j.report(trip)) else {
            return;
        };
        let lan = self.in_lan_session();
        // (the vehicle's keys are let go: their key-ups would come while the report has them)
        self.release_vehicle_keys();
        self.new_ui.close_add();
        self.new_ui.clear_win_input();
        self.new_ui.report = Some(Report {
            frame: ReportFrame {
                title,
                note: if lan {
                    "the game keeps running"
                } else {
                    "paused"
                },
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
