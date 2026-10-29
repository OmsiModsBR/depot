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

use super::super::domain::add::{AddFrame, AddWindow, Picture, PLACING};
use super::super::domain::win_input::WinInput;
use super::super::interface::output::{scale_for, App};
use glam::Vec2;
use omsi_ui::ingame::{self, AddVehicle, AddVehicleEvent, Preview};
use omsi_ui::widgets::Ui;
use winit::keyboard::KeyCode;

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

impl AddFrame {
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
