//! The "Add vehicle" window of the new interface, opened by the Alt bar's first button.
//!
//! It lists what the game menu's "Place a vehicle" list offers (the vehicles, then a
//! vehicle's liveries and depot files) in one window, and "Place" does what the end of that
//! list does: the vehicle follows the mouse until a click puts it down. The window never
//! pauses the game. Esc closes it; a click outside it goes to the game as usual.
//!
//! The overlay draws the window inside the renderer, so the clicks and the wheel over it are
//! handed to the overlay ([`WinInput`]) and what they chose comes back as events the game
//! applies before the next frame.

use crate::app::App;
use crate::new_ui::scale_for;
use glam::Vec2;
use omsi_ui::ingame::{self, AddVehicle, AddVehicleEvent};
use omsi_ui::widgets::Ui;
use winit::keyboard::KeyCode;

/// A press or release of the left button, or a turn of the wheel, over the window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum WinInput {
    Press,
    Release,
    Wheel(f32),
}

/// What the overlay draws of the window, owned so that it can cross to the renderer.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct AddFrame {
    vehicles: Vec<String>,
    vehicle: Option<usize>,
    liveries: Vec<String>,
    livery: usize,
    depots: Vec<String>,
    depot: usize,
}

impl AddFrame {
    pub(crate) fn draw(&self, ui: &mut Ui, size: Vec2) -> Option<AddVehicleEvent> {
        ingame::add_vehicle(
            ui,
            size,
            &AddVehicle {
                vehicles: &self.vehicles,
                vehicle: self.vehicle,
                liveries: &self.liveries,
                livery: self.livery,
                depots: &self.depots,
                depot: self.depot,
            },
        )
    }
}

/// The window's state on the game's side.
#[derive(Debug, Default)]
pub(crate) struct AddWindow {
    /// (label, vehicle file) as the place list has them.
    vehicles: Vec<(String, String)>,
    vehicle: Option<usize>,
    /// The chosen vehicle's liveries by name; the first line is "Random livery".
    liveries: Vec<String>,
    livery: usize,
    /// The chosen vehicle's depot files as (label, file name); the first line is the map's.
    depots: Vec<(String, String)>,
    depot: usize,
    /// A press over the window was taken: its release goes to the window too.
    pressed: bool,
}

const RANDOM_LIVERY: &str = "Random livery";
const MAP_DEPOT: &str = "The map's depot file";

impl AddWindow {
    pub(crate) fn new(vehicles: Vec<(String, String)>) -> AddWindow {
        AddWindow {
            vehicles,
            liveries: vec![RANDOM_LIVERY.to_string()],
            depots: vec![(MAP_DEPOT.to_string(), String::new())],
            ..Default::default()
        }
    }

    pub(crate) fn frame(&self) -> AddFrame {
        AddFrame {
            vehicles: self.vehicles.iter().map(|v| v.0.clone()).collect(),
            vehicle: self.vehicle,
            liveries: self.liveries.clone(),
            livery: self.livery,
            depots: self.depots.iter().map(|d| d.0.clone()).collect(),
            depot: self.depot,
        }
    }

    /// Choose the vehicle at `i` with its `liveries` and `depots` (as the place list reads
    /// them); the livery and the depot file go back to the first line.
    pub(crate) fn choose(
        &mut self,
        i: usize,
        liveries: Vec<String>,
        depots: Vec<(String, String)>,
    ) {
        if i >= self.vehicles.len() {
            return;
        }
        self.vehicle = Some(i);
        self.liveries = std::iter::once(RANDOM_LIVERY.to_string())
            .chain(liveries)
            .collect();
        self.depots = std::iter::once((MAP_DEPOT.to_string(), String::new()))
            .chain(depots)
            .collect();
        self.livery = 0;
        self.depot = 0;
    }

    /// The window lost the mouse: the release of a press it took will not come.
    pub(crate) fn forget_press(&mut self) {
        self.pressed = false;
    }

    pub(crate) fn vehicle_file(&self, i: usize) -> Option<&str> {
        self.vehicles.get(i).map(|v| v.1.as_str())
    }

    pub(crate) fn set_livery(&mut self, i: usize) {
        if i < self.liveries.len() {
            self.livery = i;
        }
    }

    pub(crate) fn set_depot(&mut self, i: usize) {
        if i < self.depots.len() {
            self.depot = i;
        }
    }

    /// What "Place" puts down: the vehicle's file, its livery (`None`: random) and its depot
    /// file (`None`: the map's).
    pub(crate) fn placement(&self) -> Option<(String, Option<String>, Option<String>)> {
        let (_, file) = self.vehicles.get(self.vehicle?)?;
        let livery = (self.livery > 0).then(|| self.liveries[self.livery].clone());
        let depot = (self.depot > 0).then(|| self.depots[self.depot].1.clone());
        Some((file.clone(), livery, depot))
    }
}

impl App {
    /// Whether the "Add vehicle" window is showing: it is open and neither the Esc menu nor a
    /// layout of its own (triple screen, headset) is in the way.
    pub(crate) fn add_window_active(&self) -> bool {
        self.new_ui.add.is_some()
            && self.new_ui.report.is_none()
            && self.game_menu.is_none()
            && self.menu.is_none()
            && !self.vr_active()
            && !self.settings.triple.enabled
    }

    /// Open the window (the Alt bar's "Add vehicle"), with the vehicles the place list has.
    pub(crate) fn open_add_vehicle(&mut self) {
        self.load_vehicle_list();
        let vehicles = crate::game_lists::place_choices(self);
        if vehicles.is_empty() {
            self.service_msg = Some(("No vehicles found".into(), 3.0));
            return;
        }
        self.new_ui.close_add();
        self.new_ui.add = Some(AddWindow::new(vehicles));
    }

    /// The window as it is drawn now.
    pub(crate) fn add_frame(&self) -> Option<AddFrame> {
        self.add_window_active()
            .then(|| self.new_ui.add.as_ref().map(AddWindow::frame))
            .flatten()
    }

    /// Whether the cursor is over the window.
    fn over_add_window(&self) -> bool {
        let Some(s) = self.surface.as_ref() else {
            return false;
        };
        let (w, h) = (s.config.width, s.config.height);
        let scale = scale_for(w, h);
        let rect = ingame::add_vehicle_rect(Vec2::new(w as f32, h as f32) / scale);
        rect.contains(Vec2::new(self.cursor.0, self.cursor.1) / scale)
    }

    /// A press or release of the left button. True when the window takes it: a press over the
    /// window, and the release of a press it took. The game does not see it then.
    pub(crate) fn add_click(&mut self, pressed: bool) -> bool {
        if !self.add_window_active() {
            return false;
        }
        let over = self.over_add_window();
        let Some(win) = self.new_ui.add.as_mut() else {
            return false;
        };
        if pressed {
            if !over {
                return false;
            }
            win.pressed = true;
            self.new_ui.push_input(WinInput::Press);
            true
        } else if std::mem::take(&mut win.pressed) {
            self.new_ui.push_input(WinInput::Release);
            true
        } else {
            false
        }
    }

    /// The wheel over the window scrolls its list (the game does not see it).
    pub(crate) fn add_wheel(&mut self, amount: f32) -> bool {
        if !self.add_window_active() || !self.over_add_window() {
            return false;
        }
        self.new_ui.push_input(WinInput::Wheel(amount));
        true
    }

    /// Esc closes the window (it does not open the Esc menu then).
    pub(crate) fn add_key(&mut self, code: KeyCode, pressed: bool) -> bool {
        if !(self.add_window_active() && pressed && code == KeyCode::Escape) {
            return false;
        }
        self.new_ui.close_add();
        true
    }

    /// Apply what the clicks in the window chose (before each frame).
    pub(crate) fn add_events(&mut self) {
        for e in self.new_ui.take_add_events() {
            let Some(win) = self.new_ui.add.as_mut() else {
                return;
            };
            match e {
                AddVehicleEvent::Vehicle(i) => {
                    let Some(file) = win.vehicle_file(i).map(String::from) else {
                        continue;
                    };
                    let liveries = crate::game_lists::place_liveries(self, &file);
                    let depots = crate::game_lists::place_depots(self, &file);
                    if let Some(win) = self.new_ui.add.as_mut() {
                        win.choose(i, liveries, depots);
                    }
                }
                AddVehicleEvent::Livery(i) => win.set_livery(i),
                AddVehicleEvent::Depot(i) => win.set_depot(i),
                AddVehicleEvent::Place => {
                    let placement = win.placement();
                    self.new_ui.close_add();
                    if let Some((bus, livery, depot)) = placement {
                        self.swap_pending = false;
                        self.place_vehicle(&bus, livery, depot);
                    }
                    return;
                }
                AddVehicleEvent::Cancel => {
                    self.new_ui.close_add();
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window() -> AddWindow {
        AddWindow::new(vec![
            ("Maker  ·  Bus A".into(), "Vehicles/A/a.bus".into()),
            ("Maker  ·  Bus B".into(), "Vehicles/B/b.bus".into()),
        ])
    }

    #[test]
    fn nothing_is_placed_before_a_vehicle_is_chosen() {
        let w = window();
        assert_eq!(w.placement(), None);
        assert_eq!(w.frame().liveries, vec![RANDOM_LIVERY.to_string()]);
        assert_eq!(w.frame().depots, vec![MAP_DEPOT.to_string()]);
    }

    #[test]
    fn the_first_lines_mean_random_livery_and_the_maps_depot_file() {
        let mut w = window();
        w.choose(
            1,
            vec!["Red".into()],
            vec![("Depot B  (b.hof)".into(), "b.hof".into())],
        );
        assert_eq!(w.placement(), Some(("Vehicles/B/b.bus".into(), None, None)));
        w.set_livery(1);
        w.set_depot(1);
        assert_eq!(
            w.placement(),
            Some((
                "Vehicles/B/b.bus".into(),
                Some("Red".into()),
                Some("b.hof".into())
            ))
        );
        // another vehicle starts again on the first lines
        w.choose(0, vec![], vec![]);
        assert_eq!(w.placement(), Some(("Vehicles/A/a.bus".into(), None, None)));
        // a line that is not there changes nothing
        w.set_livery(5);
        w.choose(9, vec![], vec![]);
        assert_eq!(w.placement(), Some(("Vehicles/A/a.bus".into(), None, None)));
    }
}
