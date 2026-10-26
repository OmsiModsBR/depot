//! The "Add vehicle" window of the new interface, opened by the Alt bar's first button.
//!
//! It offers what the game menu's "Place a vehicle" list offers (the manufacturers, then a
//! manufacturer's models, then a model's paints and depot files) in dropdowns, with a picture
//! of the vehicle chosen and its author's description, and "Place" does what the end of that
//! list does: the vehicle follows the mouse until a click puts it down. The window never
//! pauses the game. Esc closes it; a click outside it goes to the game as usual (while a
//! dropdown's list is open, the click closes the list).
//!
//! The picture is the one the launcher drew ahead of time of that vehicle and paint (its
//! Vehicles page), cut to the vehicle; none is drawn here. A live 3D view of the vehicle, as
//! the launcher's preview draws it, comes only when the player turns it on with the box's
//! button ([`LiveView`]): its helper reads the vehicle and draws it beside the game.
//!
//! The overlay draws the window inside the renderer, so the clicks and the wheel over it are
//! handed to the overlay ([`WinInput`]) and what they chose comes back as events the game
//! applies before the next frame.

use crate::app::App;
use crate::new_ui::scale_for;
use glam::Vec2;
use omsi_ui::ingame::{self, AddVehicle, AddVehicleEvent, Preview};
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

/// A picture of a vehicle drawn ahead of time, read for the window: RGBA, and where the
/// vehicle stands in it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Picture {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
    pub part: [f32; 4],
}

/// What names the picture of `bus` in `paint` under `root`.
pub(crate) fn picture_key(root: &std::path::Path, bus: &str, paint: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (root, bus, paint).hash(&mut h);
    h.finish()
}

/// The picture the launcher drew ahead of time of `bus` in `paint` (the empty paint: the
/// bus's own), cut to the vehicle; none when there is none (read off the game's thread: it
/// reads a file and walks the bus's folder).
pub(crate) fn read_picture(root: &std::path::Path, bus: &str, paint: &str) -> Option<Picture> {
    let img = crate::launcher::preview::kept_thumb(root, bus, paint)?;
    let (w, h) = img.dimensions();
    let rgba = img.into_raw();
    let part = ingame::subject_part(w as usize, h as usize, &rgba).unwrap_or([0.0, 0.0, 1.0, 1.0]);
    Some(Picture { w, h, rgba, part })
}

/// What the overlay draws of the window, owned so that it can cross to the renderer.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct AddFrame {
    makers: Vec<String>,
    maker: usize,
    vehicles: Vec<String>,
    vehicle: usize,
    liveries: Vec<String>,
    livery: usize,
    depots: Vec<String>,
    depot: usize,
    numbers: Vec<String>,
    number: usize,
    plate: String,
    plate_focused: bool,
    description: String,
    /// Which picture the window shows (see [`picture_key`]).
    picture: Option<u64>,
    /// The live 3D view, while it is on: the vehicle and paint it should show
    /// (`preview::tag`), and whether it could not be shown.
    live: Option<(u64, bool)>,
}

impl AddFrame {
    /// The player has the live 3D view on.
    pub(crate) fn live_on(&self) -> bool {
        self.live.is_some()
    }

    /// What the picture box shows, given the picture the overlay has: which one it is, and
    /// its texture, width over height and the vehicle's part (none: there is none of it).
    /// With the live view on, its frame the overlay has (which vehicle and paint it shows,
    /// and its texture) instead.
    pub(crate) fn preview(
        &self,
        have: Option<(u64, Option<(usize, f32, [f32; 4])>)>,
        live: Option<(u64, usize)>,
    ) -> Preview {
        if let Some((want, failed)) = self.live {
            return match live {
                _ if failed => Preview::Failed,
                Some((tag, tex)) if tag == want => Preview::Live { tex },
                _ => Preview::Starting,
            };
        }
        match (self.picture, have) {
            (Some(want), Some((key, Some((tex, aspect, part))))) if want == key => {
                Preview::Picture { tex, aspect, part }
            }
            (Some(want), Some((key, None))) if want == key => Preview::Missing,
            (None, _) => Preview::Missing,
            _ => Preview::Loading,
        }
    }

    pub(crate) fn draw(
        &self,
        ui: &mut Ui,
        size: Vec2,
        preview: Preview,
    ) -> Option<AddVehicleEvent> {
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
                description: &self.description,
                preview,
                live: self.live.is_some(),
            },
        )
    }
}

/// The window's state on the game's side.
#[derive(Debug, Default)]
pub(crate) struct AddWindow {
    /// The manufacturers with their models as (label, vehicle file), as the place list has them.
    makers: Vec<(String, Vec<(String, String)>)>,
    maker: usize,
    /// The chosen model, among the chosen manufacturer's.
    vehicle: usize,
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
    /// The chosen vehicle's description, as its author wrote it.
    description: String,
    /// The picture asked for (see [`picture_key`]).
    picture: Option<u64>,
    /// A press over the window was taken: its release goes to the window too.
    pressed: bool,
}

/// A line of the paint or garage list as the window shows it: the first ("Random livery",
/// "The map's depot file") translated, the vehicle's own names as they are.
fn own_line(i: usize, line: &str) -> String {
    if i == 0 {
        omsi_ui::tr(line).into_owned()
    } else {
        line.to_string()
    }
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
            // (the first lines are the interface's own, in the player's language; the rest are
            // the game's names, drawn as they are)
            liveries: self
                .liveries
                .iter()
                .enumerate()
                .map(|(i, l)| own_line(i, l))
                .collect(),
            livery: self.livery,
            depots: self
                .depots
                .iter()
                .enumerate()
                .map(|(i, d)| own_line(i, &d.0))
                .collect(),
            depot: self.depot,
            numbers: self.numbers.clone(),
            number: self.number,
            plate: self.plate.clone(),
            plate_focused: self.plate_focused,
            description: self.description.clone(),
            picture: self.picture,
            live: None,
        }
    }

    /// The chosen manufacturer's models.
    fn models(&self) -> &[(String, String)] {
        self.makers
            .get(self.maker)
            .map(|m| m.1.as_slice())
            .unwrap_or(&[])
    }

    /// Where the manufacturer `maker` and its vehicle file `file` are in the lists (the
    /// first model of the manufacturer when its file is gone; none when the manufacturer is).
    pub(crate) fn find(&self, maker: &str, file: &str) -> Option<(usize, usize)> {
        let m = self.makers.iter().position(|m| m.0 == maker)?;
        let v = self.makers[m]
            .1
            .iter()
            .position(|v| v.1 == file)
            .unwrap_or(0);
        Some((m, v))
    }

    /// The manufacturer and the vehicle file chosen now.
    pub(crate) fn chosen(&self) -> Option<(String, String)> {
        let maker = self.makers.get(self.maker)?.0.clone();
        let file = self.models().get(self.vehicle)?.1.clone();
        Some((maker, file))
    }

    /// Choose the manufacturer at `i`: its first model is chosen with it (see [`choose`]).
    /// Returns that model's file, for the caller to read its lists.
    pub(crate) fn choose_maker(&mut self, i: usize) -> Option<String> {
        if i >= self.makers.len() {
            return None;
        }
        self.maker = i;
        self.vehicle = 0;
        self.vehicle_file(0).map(String::from)
    }

    /// Choose the model at `i` of the chosen manufacturer with its `liveries`, `depots` (as
    /// the place list reads them), fleet numbers and description; the livery, the depot file
    /// and the number go back to the first line and the typed registration goes.
    pub(crate) fn choose(
        &mut self,
        i: usize,
        liveries: Vec<String>,
        depots: Vec<(String, String)>,
        numbers: Vec<String>,
        description: String,
    ) {
        if i >= self.models().len() {
            return;
        }
        self.vehicle = i;
        self.liveries = std::iter::once(RANDOM_LIVERY.to_string())
            .chain(liveries)
            .collect();
        self.depots = std::iter::once((MAP_DEPOT.to_string(), String::new()))
            .chain(depots)
            .collect();
        self.livery = 0;
        self.depot = 0;
        self.numbers = numbers;
        self.number = 0;
        self.plate.clear();
        self.plate_focused = false;
        self.description = description;
    }

    /// Pick the fleet number at line `i`.
    pub(crate) fn set_number(&mut self, i: usize) {
        if i < self.numbers.len() {
            self.number = i;
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

    /// The vehicle file and the paint whose picture the window shows: the paint chosen, the
    /// bus's own (the empty name) for "Random livery".
    pub(crate) fn picture_look(&self) -> Option<(String, String)> {
        let file = self.models().get(self.vehicle)?.1.clone();
        let paint = if self.livery > 0 {
            self.liveries[self.livery].trim().to_string()
        } else {
            String::new()
        };
        Some((file, paint))
    }

    pub(crate) fn set_picture(&mut self, key: u64) {
        self.picture = Some(key);
    }

    /// What "Place" puts down: the vehicle's file, its livery (`None`: random) and its depot
    /// file (`None`: the map's).
    pub(crate) fn placement(&self) -> Option<(String, Option<String>, Option<String>)> {
        let (_, file) = self.models().get(self.vehicle)?;
        let livery = (self.livery > 0).then(|| self.liveries[self.livery].clone());
        let depot = (self.depot > 0).then(|| self.depots[self.depot].1.clone());
        Some((file.clone(), livery, depot))
    }
}

/// The window's live 3D view: the launcher's preview helper started for it, and the vehicle
/// and paint it shows in the light of the game's time.
pub(crate) struct LiveView {
    preview: crate::launcher::preview::Preview,
    look: Option<crate::launcher::preview::Look>,
    last: std::time::Instant,
}

impl LiveView {
    fn new() -> LiveView {
        LiveView {
            preview: crate::launcher::preview::Preview::for_game(),
            look: None,
            last: std::time::Instant::now(),
        }
    }

    /// What the window draws of it: the vehicle and paint wanted, and whether it failed.
    fn frame(&self) -> (u64, bool) {
        let tag = self.look.as_ref().map_or(0, crate::launcher::preview::tag);
        let failed = self.preview.gave_up() || (self.preview.error.is_some() && !self.preview.busy);
        (tag, failed)
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
        let mut win = AddWindow::new(vehicles);
        // (it opens on the vehicle chosen last, else on the first manufacturer's first model)
        let (maker, model) = self
            .new_ui
            .add_last
            .as_ref()
            .and_then(|(m, f)| win.find(m, f))
            .unwrap_or((0, 0));
        win.choose_maker(maker);
        self.new_ui.add = Some(win);
        self.add_choose(model);
    }

    /// Choose the model at `i` of the window's manufacturer: read its paints, depot files,
    /// fleet numbers and description, and ask for its picture.
    fn add_choose(&mut self, i: usize) {
        let Some(file) = self
            .new_ui
            .add
            .as_ref()
            .and_then(|w| w.vehicle_file(i))
            .map(String::from)
        else {
            return;
        };
        let liveries = crate::game_lists::place_liveries(self, &file);
        let depots = crate::game_lists::place_depots(self, &file);
        let numbers = crate::game_lists::place_numbers(self, &file);
        let description = crate::game_lists::place_description(self, &file);
        if let Some(win) = self.new_ui.add.as_mut() {
            win.choose(i, liveries, depots, numbers, description);
            self.new_ui.add_last = win.chosen();
        }
        self.add_picture();
    }

    /// What the live view shows: the vehicle and paint chosen in the window (the bus's own
    /// paint for "Random livery"), on the map driven, in the light of the game's hour and day
    /// and the weather it started with.
    fn add_live_look(&self) -> Option<crate::launcher::preview::Look> {
        let (bus, paint) = self.new_ui.add.as_ref()?.picture_look()?;
        let (day, month) = self.clock.day_month();
        Some(crate::launcher::preview::Look {
            root: self.args.root.clone(),
            map: self.args.map.clone(),
            bus,
            paint,
            weather: self.args.weather.clone().unwrap_or_default(),
            time: (self.clock.time / 60.0) as i32,
            date: format!("{:04}-{month:02}-{day:02}", self.clock.year),
        })
    }

    /// Keep the live view on the vehicle and paint chosen (a change of either starts the
    /// helper reading the new one; the light stays as it was).
    fn add_live_follow(&mut self) {
        let look = self.add_live_look();
        let Some(live) = self.new_ui.add_live.as_mut() else {
            return;
        };
        let same = |a: &crate::launcher::preview::Look, b: &crate::launcher::preview::Look| {
            (&a.bus, &a.paint) == (&b.bus, &b.paint)
        };
        if let Some(look) = look {
            if !live.look.as_ref().is_some_and(|l| same(l, &look)) {
                live.preview.want(look.clone());
                live.look = Some(look);
            }
        }
    }

    /// Where the live view is drawn in the window, in pixels (the picture box's inside).
    fn add_live_size(&self) -> Option<(u32, u32)> {
        let s = self.surface.as_ref()?;
        let (w, h) = (s.config.width, s.config.height);
        let scale = scale_for(w, h);
        let b = ingame::add_vehicle_box(Vec2::new(w as f32, h as f32) / scale).inset(1.0);
        Some((
            ((b.w * scale) as u32).max(16),
            ((b.h * scale) as u32).max(16),
        ))
    }

    /// Before each frame: the live view's helper while the window shows the view (it ends a
    /// moment after the window is hidden behind the Esc menu or the report), and its new
    /// frame to the overlay.
    pub(crate) fn add_live_update(&mut self) {
        if self.new_ui.add.is_none() {
            self.new_ui.stop_add_live();
            return;
        }
        let shown = self
            .add_window_active()
            .then(|| self.add_live_size())
            .flatten();
        let Some(live) = self.new_ui.add_live.as_mut() else {
            return;
        };
        let now = std::time::Instant::now();
        let dt = now.duration_since(live.last).as_secs_f32().min(0.5);
        live.last = now;
        live.preview.update(dt, shown);
        let frame = live.preview.take_frame();
        let showing = live.preview.showing();
        if let (Some((w, h, px)), Some(tag)) = (frame, showing) {
            self.new_ui.offer_add_live((tag, w, h, px));
        }
    }

    /// Ask for the picture of the vehicle and paint chosen in the window.
    fn add_picture(&mut self) {
        let Some((bus, paint)) = self.new_ui.add.as_ref().and_then(AddWindow::picture_look) else {
            return;
        };
        let root = self.args.root.clone();
        let key = self.new_ui.want_add_picture(&root, &bus, &paint);
        if let Some(win) = self.new_ui.add.as_mut() {
            win.set_picture(key);
        }
    }

    /// The window as it is drawn now.
    pub(crate) fn add_frame(&self) -> Option<AddFrame> {
        let mut frame = self
            .add_window_active()
            .then(|| self.new_ui.add.as_ref().map(AddWindow::frame))
            .flatten()?;
        frame.live = self.new_ui.add_live.as_ref().map(LiveView::frame);
        Some(frame)
    }

    /// Whether the cursor is over the window, or a dropdown of it has its list open (the list
    /// may stand out of the window, and a click anywhere closes it).
    fn over_add_window(&self) -> bool {
        if self.new_ui.add_modal() {
            return true;
        }
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
                    // (a manufacturer chosen comes with its first model)
                    if win.choose_maker(i).is_some() {
                        self.add_choose(0);
                        self.add_live_follow();
                    }
                }
                AddVehicleEvent::Vehicle(i) => {
                    self.add_choose(i);
                    self.add_live_follow();
                }
                AddVehicleEvent::Livery(i) => {
                    win.set_livery(i);
                    self.add_picture();
                    self.add_live_follow();
                }
                AddVehicleEvent::Depot(i) => win.set_depot(i),
                AddVehicleEvent::Number(i) => win.set_number(i),
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
                // (the live view only when the player asks for it)
                AddVehicleEvent::Live(true) => {
                    if self.new_ui.add_live.is_none() {
                        self.new_ui.add_live = Some(LiveView::new());
                    }
                    self.add_live_follow();
                }
                AddVehicleEvent::Live(false) => self.new_ui.stop_add_live(),
                AddVehicleEvent::Orbit(dx, dy) => {
                    if let Some(l) = self.new_ui.add_live.as_mut() {
                        l.preview.orbit(dx, dy);
                    }
                }
                AddVehicleEvent::Zoom(k) => {
                    if let Some(l) = self.new_ui.add_live.as_mut() {
                        l.preview.zoom_by(k);
                    }
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
        w.choose(0, vec![], vec![], vec![], String::new());
        w
    }

    #[test]
    fn a_manufacturer_comes_with_its_first_model() {
        let mut w = window();
        assert_eq!(w.frame().makers, vec!["Maker".to_string(), "Other".into()]);
        assert_eq!(
            w.frame().vehicles,
            vec!["Bus A".to_string(), "Bus B".into()]
        );
        w.choose(1, vec!["Red".into()], vec![], vec![], "About B".into());
        assert_eq!(w.choose_maker(1).as_deref(), Some("Vehicles/C/c.bus"));
        assert_eq!(w.frame().vehicles, vec!["Bus C".to_string()]);
        assert_eq!(w.frame().vehicle, 0);
        assert_eq!(w.choose_maker(7), None);
        assert_eq!(
            w.chosen(),
            Some(("Other".into(), "Vehicles/C/c.bus".into()))
        );
    }

    #[test]
    fn the_window_finds_the_vehicle_chosen_last() {
        let w = window();
        assert_eq!(w.find("Maker", "Vehicles/B/b.bus"), Some((0, 1)));
        assert_eq!(w.find("Other", "Vehicles/gone.bus"), Some((1, 0)));
        assert_eq!(w.find("Gone", "Vehicles/A/a.bus"), None);
    }

    #[test]
    fn the_first_lines_mean_random_livery_and_the_maps_depot_file() {
        let mut w = window();
        assert_eq!(w.placement(), Some(("Vehicles/A/a.bus".into(), None, None)));
        w.choose(
            1,
            vec!["Red".into()],
            vec![("Depot B  (b.hof)".into(), "b.hof".into())],
            vec![],
            String::new(),
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
        w.choose(0, vec![], vec![], vec![], String::new());
        assert_eq!(w.placement(), Some(("Vehicles/A/a.bus".into(), None, None)));
        // a line that is not there changes nothing
        w.set_livery(5);
        w.choose(9, vec![], vec![], vec![], String::new());
        assert_eq!(w.placement(), Some(("Vehicles/A/a.bus".into(), None, None)));
    }

    #[test]
    fn the_number_is_picked_and_the_plate_is_typed() {
        let mut w = window();
        w.choose(
            0,
            vec![],
            vec![],
            vec!["10".into(), "11".into(), "12".into()],
            String::new(),
        );
        w.set_number(2);
        assert_eq!(w.identity(), (Some("12".into()), None));
        w.set_number(9);
        assert_eq!(w.identity(), (Some("12".into()), None));
        assert!(
            !w.type_plate("ab"),
            "the field has no keyboard before it is clicked"
        );
        w.focus_plate();
        assert!(w.type_plate(" B-XY 12\n"));
        assert_eq!(w.identity(), (Some("12".into()), Some("B-XY 12".into())));
        // another vehicle forgets both
        w.choose(1, vec![], vec![], vec!["7".into()], String::new());
        assert_eq!(w.identity(), (Some("7".into()), None));
        assert!(!w.frame().plate_focused);
    }

    #[test]
    fn the_picture_is_of_the_paint_chosen_and_the_buss_own_for_a_random_one() {
        let mut w = window();
        w.choose(1, vec![" Red ".into()], vec![], vec![], "About B".into());
        assert_eq!(
            w.picture_look(),
            Some(("Vehicles/B/b.bus".into(), String::new()))
        );
        w.set_livery(1);
        assert_eq!(
            w.picture_look(),
            Some(("Vehicles/B/b.bus".into(), "Red".into()))
        );
        assert_eq!(w.frame().description, "About B");
    }

    #[test]
    fn the_picture_box_waits_for_its_own_picture() {
        let mut f = window().frame();
        assert_eq!(f.preview(None, None), Preview::Missing, "nothing asked for");
        f.picture = Some(7);
        assert_eq!(f.preview(None, None), Preview::Loading);
        assert_eq!(
            f.preview(Some((6, None)), None),
            Preview::Loading,
            "another's"
        );
        assert_eq!(f.preview(Some((7, None)), None), Preview::Missing);
        assert_eq!(
            f.preview(Some((7, Some((3, 1.5, [0.1, 0.2, 0.9, 0.8])))), None),
            Preview::Picture {
                tex: 3,
                aspect: 1.5,
                part: [0.1, 0.2, 0.9, 0.8]
            }
        );
        let root = std::path::Path::new("C:/OMSI 2");
        assert_ne!(
            picture_key(root, "Vehicles/A/a.bus", ""),
            picture_key(root, "Vehicles/A/a.bus", "Red")
        );
    }

    #[test]
    fn the_live_view_shows_its_own_frame_or_says_how_it_stands() {
        let mut f = window().frame();
        f.picture = Some(7);
        let pic = Some((7, Some((3, 1.5, [0.0, 0.0, 1.0, 1.0]))));
        assert!(!f.live_on());
        assert!(
            matches!(f.preview(pic, Some((9, 4))), Preview::Picture { .. }),
            "off: the picture, whatever frame is left"
        );
        f.live = Some((9, false));
        assert!(f.live_on());
        assert_eq!(f.preview(pic, None), Preview::Starting);
        assert_eq!(
            f.preview(pic, Some((8, 4))),
            Preview::Starting,
            "a frame of the vehicle shown before"
        );
        assert_eq!(f.preview(pic, Some((9, 4))), Preview::Live { tex: 4 });
        f.live = Some((9, true));
        assert_eq!(f.preview(pic, Some((9, 4))), Preview::Failed);
    }
}
