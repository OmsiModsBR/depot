//! The "Add vehicle" window of the new interface, opened by the Alt bar's first button.
//!
//! It lists what the game menu's "Place a vehicle" list offers (the manufacturers, then a
//! manufacturer's models, then a model's liveries and depot files) in one window, and "Place" does what the end of that
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
    /// The window lost the mouse: a press it holds ends without a click.
    Lost,
}

/// What the overlay draws of the window, owned so that it can cross to the renderer.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct AddFrame {
    makers: Vec<String>,
    maker: Option<usize>,
    vehicles: Vec<String>,
    vehicle: Option<usize>,
    liveries: Vec<String>,
    livery: usize,
    depots: Vec<String>,
    depot: usize,
    numbers: Vec<String>,
    number: usize,
    plate: String,
    plate_focused: bool,
}

impl AddFrame {
    pub(crate) fn draw(&self, ui: &mut Ui, size: Vec2) -> Option<AddVehicleEvent> {
        ingame::add_vehicle(
            ui,
            size,
            &AddVehicle {
                makers: &self.makers,
                maker: self.maker,
                vehicles: &self.vehicles,
                vehicle: self.vehicle,
                liveries: &self.liveries,
                livery: self.livery,
                depots: &self.depots,
                depot: self.depot,
                numbers: &self.numbers,
                number: self.number,
                plate: &self.plate,
                plate_focused: self.plate_focused,
                plate_hint: &omsi_ui::tr("Automatic"),
            },
        )
    }
}

/// The window's state on the game's side.
#[derive(Debug, Default)]
pub(crate) struct AddWindow {
    /// The manufacturers with their models as (label, vehicle file), as the place list has them.
    makers: Vec<(String, Vec<(String, String)>)>,
    maker: Option<usize>,
    /// The chosen model, among the chosen manufacturer's.
    vehicle: Option<usize>,
    /// The chosen vehicle's liveries by name; the first line is "Random livery".
    liveries: Vec<String>,
    livery: usize,
    /// The chosen vehicle's depot files as (label, file name); the first line is the map's.
    depots: Vec<(String, String)>,
    depot: usize,
    /// The chosen vehicle's fleet numbers (its `[number]` list) and the one picked.
    numbers: Vec<String>,
    number: usize,
    /// The registration typed (empty: the vehicle's own) and whether its field has the keyboard.
    plate: String,
    plate_focused: bool,
    /// A press over the window was taken: its release goes to the window too.
    pressed: bool,
}

/// The longest registration the field takes.
const PLATE_MAX: usize = 16;
const RANDOM_LIVERY: &str = "Random livery";
/// Why no vehicle is added now.
pub(crate) const PLACING: &str = "Put down the vehicle on the mouse first";
const MAP_DEPOT: &str = "The map's depot file";

impl AddWindow {
    pub(crate) fn new(makers: Vec<(String, Vec<(String, String)>)>) -> AddWindow {
        AddWindow {
            makers,
            liveries: vec![RANDOM_LIVERY.to_string()],
            depots: vec![(MAP_DEPOT.to_string(), String::new())],
            ..Default::default()
        }
    }

    pub(crate) fn frame(&self) -> AddFrame {
        AddFrame {
            makers: self.makers.iter().map(|m| m.0.clone()).collect(),
            maker: self.maker,
            vehicles: self.models().iter().map(|v| v.0.clone()).collect(),
            vehicle: self.vehicle,
            liveries: self.liveries.clone(),
            livery: self.livery,
            depots: self.depots.iter().map(|d| d.0.clone()).collect(),
            depot: self.depot,
            numbers: self.numbers.clone(),
            number: self.number,
            plate: self.plate.clone(),
            plate_focused: self.plate_focused,
        }
    }

    /// The chosen manufacturer's models (none before one is chosen).
    fn models(&self) -> &[(String, String)] {
        self.maker
            .and_then(|m| self.makers.get(m))
            .map(|m| m.1.as_slice())
            .unwrap_or(&[])
    }

    /// Choose the manufacturer at `i`: no model is chosen then, and the livery and the depot
    /// file go back to the first lines. Returns the model's index when the manufacturer has
    /// only one (the window chooses it for the player).
    pub(crate) fn choose_maker(&mut self, i: usize) -> Option<usize> {
        if i >= self.makers.len() {
            return None;
        }
        self.maker = Some(i);
        self.vehicle = None;
        self.liveries = vec![RANDOM_LIVERY.to_string()];
        self.depots = vec![(MAP_DEPOT.to_string(), String::new())];
        self.livery = 0;
        self.depot = 0;
        self.set_numbers(Vec::new());
        (self.models().len() == 1).then_some(0)
    }

    /// Choose the model at `i` of the chosen manufacturer with its `liveries` and `depots` (as the place list reads
    /// them); the livery and the depot file go back to the first line.
    pub(crate) fn choose(
        &mut self,
        i: usize,
        liveries: Vec<String>,
        depots: Vec<(String, String)>,
    ) {
        if i >= self.models().len() {
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
        self.set_numbers(Vec::new());
    }

    /// The fleet numbers of the vehicle just chosen; the first is picked and the typed
    /// registration goes (it belonged to the other vehicle).
    pub(crate) fn set_numbers(&mut self, numbers: Vec<String>) {
        self.numbers = numbers;
        self.number = 0;
        self.plate.clear();
        self.plate_focused = false;
    }

    /// Step the fleet number by `by`, wrapping around the list.
    pub(crate) fn step_number(&mut self, by: i32) {
        let n = self.numbers.len() as i32;
        if n > 1 {
            self.number = (self.number as i32 + by).rem_euclid(n) as usize;
        }
    }

    pub(crate) fn focus_plate(&mut self) {
        self.plate_focused = true;
    }

    /// Text typed into the registration's field. True when the field took it.
    pub(crate) fn type_plate(&mut self, text: &str) -> bool {
        if !self.plate_focused {
            return false;
        }
        for c in text.chars().filter(|c| !c.is_control()) {
            if self.plate.chars().count() < PLATE_MAX {
                self.plate.push(c);
            }
        }
        true
    }

    /// The fleet number and the registration to put down (`None`: the vehicle's own).
    pub(crate) fn identity(&self) -> (Option<String>, Option<String>) {
        let number = self.numbers.get(self.number).cloned();
        let plate = Some(self.plate.trim().to_string()).filter(|p| !p.is_empty());
        (number, plate)
    }

    pub(crate) fn vehicle_file(&self, i: usize) -> Option<&str> {
        self.models().get(i).map(|v| v.1.as_str())
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
        let (_, file) = self.models().get(self.vehicle?)?;
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
        // (one vehicle on the mouse at a time: a second would leave the first where it was)
        if self.placing.is_some() {
            self.service_msg = Some((PLACING.into(), 3.0));
            return;
        }
        self.load_vehicle_list();
        let vehicles = crate::game_lists::place_choices(self);
        if vehicles.iter().all(|m| m.1.is_empty()) {
            self.service_msg = Some(("No vehicles found".into(), 3.0));
            return;
        }
        self.new_ui.close_add();
        // (one of the bar's windows at a time)
        self.new_ui.close_tool();
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
            // (hidden behind the Esc menu or the report: the overlay lets go of a press it
            // had, so its release is the game's)
            if let Some(win) = self.new_ui.add.as_mut() {
                win.pressed = false;
            }
            return false;
        }
        let over = self.over_add_window();
        let Some(win) = self.new_ui.add.as_mut() else {
            return false;
        };
        if pressed {
            // (a press outside: its release is the game's too)
            win.pressed = over;
            if !over {
                return false;
            }
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
        if !pressed {
            // (the release of a key the field took does not reach the game either)
            return self.new_ui.add_taken.remove(&code);
        }
        if !self.add_window_active() {
            return false;
        }
        if code == KeyCode::Escape {
            self.new_ui.close_add();
            return true;
        }
        let Some(win) = self.new_ui.add.as_mut().filter(|w| w.plate_focused) else {
            return false;
        };
        match code {
            KeyCode::ShiftLeft
            | KeyCode::ShiftRight
            | KeyCode::ControlLeft
            | KeyCode::ControlRight
            | KeyCode::AltLeft
            | KeyCode::AltRight => return false,
            KeyCode::Enter | KeyCode::NumpadEnter => win.plate_focused = false,
            KeyCode::Backspace | KeyCode::Delete => {
                win.plate.pop();
            }
            _ => {}
        }
        self.new_ui.add_taken.insert(code);
        true
    }

    /// Text typed while the registration's field has the keyboard. True when it took it.
    pub(crate) fn add_text(&mut self, text: &str, code: Option<KeyCode>) -> bool {
        if !self.add_window_active() || !text.chars().any(|c| !c.is_control()) {
            return false;
        }
        let Some(win) = self.new_ui.add.as_mut() else {
            return false;
        };
        if !win.type_plate(text) {
            return false;
        }
        if let Some(code) = code {
            self.new_ui.add_taken.insert(code);
        }
        true
    }

    /// Apply what the clicks in the window chose (before each frame).
    pub(crate) fn add_events(&mut self) {
        for e in self.new_ui.take_add_events() {
            let Some(win) = self.new_ui.add.as_mut() else {
                return;
            };
            match e {
                AddVehicleEvent::Maker(i) => {
                    // (a manufacturer with one model has it chosen: nothing else to pick)
                    let only = win.choose_maker(i);
                    if let Some(m) = only {
                        let Some(file) = win.vehicle_file(m).map(String::from) else {
                            continue;
                        };
                        let liveries = crate::game_lists::place_liveries(self, &file);
                        let depots = crate::game_lists::place_depots(self, &file);
                        let numbers = crate::game_lists::place_numbers(self, &file);
                        if let Some(win) = self.new_ui.add.as_mut() {
                            win.choose(m, liveries, depots);
                            win.set_numbers(numbers);
                        }
                    }
                }
                AddVehicleEvent::Vehicle(i) => {
                    let Some(file) = win.vehicle_file(i).map(String::from) else {
                        continue;
                    };
                    let liveries = crate::game_lists::place_liveries(self, &file);
                    let depots = crate::game_lists::place_depots(self, &file);
                    let numbers = crate::game_lists::place_numbers(self, &file);
                    if let Some(win) = self.new_ui.add.as_mut() {
                        win.choose(i, liveries, depots);
                        win.set_numbers(numbers);
                    }
                }
                AddVehicleEvent::Livery(i) => win.set_livery(i),
                AddVehicleEvent::Depot(i) => win.set_depot(i),
                AddVehicleEvent::Number(by) => win.step_number(by),
                AddVehicleEvent::Plate => win.focus_plate(),
                AddVehicleEvent::Place => {
                    if self.placing.is_some() {
                        self.service_msg = Some((PLACING.into(), 3.0));
                        continue;
                    }
                    let placement = win.placement();
                    let (number, plate) = win.identity();
                    self.new_ui.close_add();
                    if let Some((bus, livery, depot)) = placement {
                        self.swap_pending = false;
                        self.place_vehicle_with(&bus, livery, depot, number, plate);
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
        let mut w = AddWindow::new(vec![
            (
                "Maker".into(),
                vec![
                    ("Bus A".into(), "Vehicles/A/a.bus".into()),
                    ("Bus B".into(), "Vehicles/B/b.bus".into()),
                ],
            ),
            (
                "Other".into(),
                vec![("Bus C".into(), "Vehicles/C/c.bus".into())],
            ),
        ]);
        w.choose_maker(0);
        w
    }

    #[test]
    fn the_models_are_those_of_the_chosen_manufacturer() {
        let mut w = window();
        assert_eq!(w.frame().makers, vec!["Maker".to_string(), "Other".into()]);
        assert_eq!(
            w.frame().vehicles,
            vec!["Bus A".to_string(), "Bus B".into()]
        );
        // another manufacturer: its models, and no model chosen yet; one model is offered to
        // be chosen at once
        w.choose(1, vec!["Red".into()], vec![]);
        assert_eq!(w.choose_maker(1), Some(0));
        assert_eq!(w.frame().vehicles, vec!["Bus C".to_string()]);
        assert_eq!(w.placement(), None);
        assert_eq!(w.frame().liveries, vec![RANDOM_LIVERY.to_string()]);
        assert_eq!(w.choose_maker(7), None);
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

    #[test]
    fn the_number_steps_and_the_plate_is_typed() {
        let mut w = window();
        w.set_numbers(vec!["10".into(), "11".into(), "12".into()]);
        w.step_number(-1);
        assert_eq!(w.identity(), (Some("12".into()), None));
        w.step_number(1);
        assert!(
            !w.type_plate("ab"),
            "the field has no keyboard before it is clicked"
        );
        w.focus_plate();
        assert!(w.type_plate(" B-XY 12\n"));
        assert_eq!(w.identity(), (Some("10".into()), Some("B-XY 12".into())));
        // another vehicle forgets both
        w.set_numbers(vec!["7".into()]);
        assert_eq!(w.identity(), (Some("7".into()), None));
        assert!(!w.plate_focused);
    }
}
