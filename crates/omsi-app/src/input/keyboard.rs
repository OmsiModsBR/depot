//! The keyboard: a key press in the game, the menus, the choosers and the text fields.

use crate::app::App;
use crate::keys;
use crate::player::{digit_of, fallback_action};
use crate::situation::apply_situation;
use crate::startup::shift_held_now;
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
#[cfg(feature = "lan")]
use crate::lan;

/// Printable fallback for a route number when the window backend supplies no text event.
/// Normal typing uses the actual text event so Shift/layout-specific symbols are kept.
pub(crate) fn route_char(code: KeyCode) -> Option<char> {
    let symbol = match code {
        KeyCode::Minus | KeyCode::NumpadSubtract => Some('-'),
        KeyCode::Equal | KeyCode::NumpadAdd => Some('+'),
        KeyCode::Slash | KeyCode::NumpadDivide => Some('/'),
        KeyCode::NumpadMultiply => Some('*'),
        KeyCode::Period | KeyCode::NumpadDecimal => Some('.'),
        KeyCode::Comma => Some(','),
        KeyCode::Semicolon => Some(';'),
        KeyCode::Quote => Some('\''),
        KeyCode::BracketLeft => Some('['),
        KeyCode::BracketRight => Some(']'),
        KeyCode::Backslash => Some('\\'),
        KeyCode::Backquote => Some('`'),
        _ => None,
    };
    if symbol.is_some() {
        return symbol;
    }
    let name = format!("{code:?}");
    let c = name.strip_prefix("Digit").or_else(|| name.strip_prefix("Numpad")).or_else(|| name.strip_prefix("Key"))?;
    let mut chars = c.chars();
    let ch = chars.next()?;
    (chars.next().is_none() && ch.is_ascii_alphanumeric()).then(|| ch.to_ascii_uppercase())
}

/// How long a route number typed by hand may be. Eight characters were too few: Hong Kong
/// buses take commands through it (`paper_sign_1_name=ABC.png`, `adddept_sign=1`, #1518).
pub(crate) const ROUTE_NUMBER_MAX: usize = 64;

/// The keys that fly the free camera (and, with no bus of one's own, the view).
pub(crate) fn flies_free_camera(code: KeyCode) -> bool {
    matches!(code, KeyCode::KeyW | KeyCode::KeyA | KeyCode::KeyS | KeyCode::KeyD | KeyCode::KeyQ | KeyCode::KeyE | KeyCode::Space | KeyCode::ShiftLeft | KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::ArrowUp | KeyCode::ArrowDown)
}

/// `App::key_left_free` for a key's scan code.
pub(crate) fn key_left_free(scan: Option<i32>, action: &str, own: &std::collections::HashSet<i32>, game: &[omsi_content::KeyBinding]) -> bool {
    !scan.is_some_and(|s| own.contains(&s)) && !game.iter().any(|b| b.scan_code != 0 && b.action.eq_ignore_ascii_case(action))
}

impl App {
    /// A key of the window, or of an `OMSI_INPUT` script.
    pub(crate) fn on_key(&mut self, event_loop: &ActiveEventLoop, code: KeyCode, pressed: bool, repeat: bool) {
        if self.vr_nav_edit.is_some() {
            if !pressed { self.keys.remove(&code); }
            if matches!(code, KeyCode::ControlLeft | KeyCode::ControlRight | KeyCode::ShiftLeft | KeyCode::ShiftRight) && pressed {
                self.keys.insert(code);
            }
            if pressed && !repeat {
                match code {
                    KeyCode::Escape | KeyCode::Enter => self.finish_vr_nav_edit(),
                    KeyCode::KeyR => self.vr_nav_adjust("reset", 1.0),
                    _ => {}
                }
            }
            return;
        }
        // the new interface's end-of-trip report has the keys while it shows
        if self.report_key(code, pressed) {
            return;
        }
        // Esc closes the new interface's "Add vehicle" window before anything else (the city
        // map, the mirror editor)
        if self.add_key(code, pressed) {
            return;
        }
        // The mirror panels (see mirror_hud.rs): Ctrl+M shows or hides them, Ctrl+Shift+M
        // starts and ends their editor; in the editor Insert, Delete, C and Esc are its keys.
        if self.in_cab && self.game_menu.is_none() && self.player.is_some() {
            let ctrl = self.keys.contains(&KeyCode::ControlLeft) || self.keys.contains(&KeyCode::ControlRight);
            let shift = self.keys.contains(&KeyCode::ShiftLeft) || self.keys.contains(&KeyCode::ShiftRight);
            if pressed && !repeat && code == KeyCode::KeyM && ctrl {
                if let Some(p) = self.player.as_ref() {
                    let msg = if shift { self.mirror_hud.toggle_edit(p) } else { self.mirror_hud.toggle(p) };
                    self.service_msg = Some((msg, if shift { 6.0 } else { 3.0 }));
                }
                return;
            }
            // (in the editor the arrows aim the mirror under the cursor; see the frame)
            if matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::PageUp | KeyCode::PageDown | KeyCode::Minus | KeyCode::Equal | KeyCode::NumpadAdd | KeyCode::NumpadSubtract) && self.mirror_hud.arrow(code, pressed) {
                return;
            }
            // R puts the mirror under the cursor back as the bus has it, Shift+R every mirror
            if self.mirror_hud.editing() && code == KeyCode::KeyR {
                if pressed && !repeat {
                    let size = self.hud_size();
                    let which = self.mirror_hud.cam_under(self.hud_cursor(), size);
                    let msg = match self.player.as_mut() {
                        Some(p) if shift => {
                            let n = p.vehicle.ty.def.cameras_reflexion.len();
                            p.mirror_offsets = vec![[0.0; 2]; n];
                            p.mirror_shifts = vec![[0.0; 3]; n];
                            p.mirror_fovs = vec![0.0; n];
                            p.mirrors_dirty = true;
                            "Every mirror is back as the bus has it".to_string()
                        }
                        Some(p) if which.is_some() => {
                            let i = which.unwrap_or(0);
                            if let Some(o) = p.mirror_offsets.get_mut(i) {
                                *o = [0.0; 2];
                            }
                            if let Some(s) = p.mirror_shifts.get_mut(i) {
                                *s = [0.0; 3];
                            }
                            if let Some(f) = p.mirror_fovs.get_mut(i) {
                                *f = 0.0;
                            }
                            p.mirrors_dirty = true;
                            format!("Mirror {} is back as the bus has it (Shift+R: every mirror)", i + 1)
                        }
                        _ => "R: put the cursor on a mirror panel (Shift+R: every mirror)".to_string(),
                    };
                    self.service_msg = Some((msg, 3.0));
                }
                return;
            }
            if self.mirror_hud.editing() && matches!(code, KeyCode::BracketLeft | KeyCode::BracketRight | KeyCode::Semicolon | KeyCode::Quote) {
                if pressed {
                    let size = self.hud_size();
                    self.mirror_hud.size_key(code, self.hud_cursor(), size);
                }
                return;
            }
            if self.mirror_hud.editing() && matches!(code, KeyCode::Insert | KeyCode::Delete | KeyCode::Backspace | KeyCode::KeyC | KeyCode::Escape) {
                if pressed && !repeat {
                    let size = self.hud_size();
                    if let Some(p) = self.player.as_ref() {
                        if let Some(msg) = self.mirror_hud.key(code, p, self.hud_cursor(), size) {
                            self.service_msg = Some((msg, 4.0));
                        }
                    }
                }
                return;
            }
        }
        // Escape closes the city map first (it would end the session)
        if pressed && code == KeyCode::Escape {
            if let Some(n) = self.navigator.as_mut().filter(|n| n.map_open()) {
                n.toggle_map();
                return;
            }
            // and gives the mouse back to the bus when the plugins' panels have it
            if self.release_plugin_focus() {
                return;
            }
        }
        let event_key = PhysicalKey::Code(code);
        if let (Some(m), PhysicalKey::Code(code)) = (self.menu.as_mut(), event_key) {
            if pressed {
                if code == KeyCode::Escape {
                    crate::platform::exit(event_loop);
                }
                m.key(code);
                if m.start {
                    self.args.map = m
                        .maps
                        .get(m.map)
                        .map(|x| x.1.clone())
                        .unwrap_or(self.args.map.clone());
                    self.args.bus = m.vehicles.get(m.vehicle).map(|x| x.1.clone());
                    self.args.time = format!("{:02}:00", m.hour);
                    self.args.traffic = m.traffic;
                    self.args.passengers = m.passengers;
                    self.args.schedule = m.schedule;
                    self.args.day_of_year = Some(m.day);
                    if m.weather > 0 {
                        self.args.weather = m.weathers.get(m.weather).map(|w| w.1.clone());
                    }
                    if m.situation > 0 {
                        self.args.situation = m.situations.get(m.situation).map(|s| s.1.clone());
                        if let Err(e) = apply_situation(&mut self.args) {
                            log::error!("{e:#}");
                        }
                    }
                    self.menu = None;
                    self.hud = None;
                    self.load_world_now(event_loop);
                }
            }
            return;
        }
        if let PhysicalKey::Code(code) = event_key {
            let pressed = pressed;
            // LAN chat: its keys (`chat_open`, '/' or '`', and `chat_toggle`, V, in keyboard.cfg's
            // [game]: the player can move them, #130) open the line and show or hide the
            // chat, and while the line is open the keys are its own
            #[cfg(feature = "lan")]
            if let Some(l) = self.online.lan.as_mut() {
                let held = |a: KeyCode, b: KeyCode| self.keys.contains(&a) || self.keys.contains(&b);
                let chord = omsi_content::input::chord(
                    held(KeyCode::ShiftLeft, KeyCode::ShiftRight),
                    held(KeyCode::ControlLeft, KeyCode::ControlRight),
                    held(KeyCode::AltLeft, KeyCode::AltRight),
                );
                let bound = if held(KeyCode::SuperLeft, KeyCode::SuperRight) {
                    None
                } else {
                    keys::dik_code(code).and_then(|scan| self.game_keys.iter()
                        .find(|b| b.scan_code == scan && b.matches(chord) && b.action.to_ascii_lowercase().starts_with("chat_"))
                        .map(|b| b.action.clone()))
                };
                if lan::chat_key(l, &mut self.online.remotes, code, pressed, repeat, bound.as_deref()) {
                    return;
                }
            }
            if pressed && !repeat {
                self.keys.insert(code);
            } else if !pressed {
                self.keys.remove(&code);
            }
            // the Alt bar of the new interface: a tap of Alt, and its keys while the keyboard is on it
            if self.alt_key(code, pressed, repeat) {
                return;
            }
            // Alt+Enter: full screen on and off
            if pressed && !repeat && matches!(code, KeyCode::Enter | KeyCode::NumpadEnter) && (self.keys.contains(&KeyCode::AltLeft) || self.keys.contains(&KeyCode::AltRight)) {
                if self.spanned {
                    log::info!("triple screen: the window spans three monitors, Alt+Enter is left alone");
                } else if let Some(win) = self.window.as_ref() {
                    win.set_fullscreen(if win.fullscreen().is_some() { None } else { Some(winit::window::Fullscreen::Borderless(None)) });
                }
                return;
            }
            #[cfg(windows)]
            if pressed && !repeat && (self.vr.is_some() || self.settings.vr_requested()) {
                let modifier = omsi_content::input::chord(
                    self.keys.contains(&KeyCode::ShiftLeft) || self.keys.contains(&KeyCode::ShiftRight),
                    self.keys.contains(&KeyCode::ControlLeft) || self.keys.contains(&KeyCode::ControlRight),
                    self.keys.contains(&KeyCode::AltLeft) || self.keys.contains(&KeyCode::AltRight),
                );
                let action = keys::dik_code(code).and_then(|scan| self.game_keys.iter()
                    .find(|b| b.scan_code == scan && b.matches(modifier) && b.action.starts_with("vr_"))
                    .map(|b| b.action.clone()));
                if let Some(action) = action {
                    if self.game_action(&action) { return; }
                }
            }
            // Shift+number works a door the way a key bound to its trigger in
            // `Inputs/keyboard.cfg` does in OMSI: `<trigger>` when it goes down and
            // `<trigger>_off` when it comes up. The door buttons of automatic-door buses
            // are push buttons held between the two (the MAN Lion's City A21's
            // `cockpit_tuertaster1`); never let go, the button stayed pressed and, once
            // the door release was on, the front door opened and closed for ever.
            if !pressed {
                let released: Vec<Vec<String>> = if digit_of(code).is_some() {
                    self.door_key_triggers.remove(&code).into_iter().collect()
                } else if matches!(code, KeyCode::ShiftLeft | KeyCode::ShiftRight) {
                    self.door_key_triggers.drain().map(|(_, g)| g).collect()
                } else {
                    Vec::new()
                };
                if let Some(p) = self.player.as_mut() {
                    for fired in &released {
                        p.door_key_off(fired);
                    }
                }
            }
            // placing a vehicle with the mouse: its keys first (Escape takes it away)
            if self.game_menu.is_none() && self.placing_key(code, pressed) {
                return;
            }
            // the game menu: Escape opens it (and pauses, except in a LAN session, which
            // goes on for the others), and while it is open the keys are its own
            if self.game_menu.is_some() {
                if pressed && !repeat {
                    self.menu_key(event_loop, code);
                }
                return;
            }
            // the object editor takes its keys first (Escape leaves it)
            if pressed && self.editor.is_some() && self.editor_key(code) {
                return;
            }
            if pressed && !repeat && code == KeyCode::Escape {
                self.open_game_menu();
                return;
            }
            // a tutorial's pages: Enter / Page Down on, Page Up back, Ctrl+T hides them
            let in_lan = self.in_lan_session();
            if let (true, Some(t)) = (pressed, self.tutorial.as_mut()) {
                let ctrl = self.keys.contains(&KeyCode::ControlLeft) || self.keys.contains(&KeyCode::ControlRight);
                match code {
                    KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::PageDown if !t.hidden && !in_lan => {
                        t.next();
                        return;
                    }
                    KeyCode::PageUp if !t.hidden => {
                        t.back();
                        return;
                    }
                    KeyCode::KeyT if ctrl => {
                        t.hidden = !t.hidden;
                        return;
                    }
                    _ => {}
                }
            }
            let ctrl = self.keys.contains(&KeyCode::ControlLeft) || self.keys.contains(&KeyCode::ControlRight);
            let alt = self.keys.contains(&KeyCode::AltLeft) || self.keys.contains(&KeyCode::AltRight);
            let shift_now = self.keys.contains(&KeyCode::ShiftLeft) || self.keys.contains(&KeyCode::ShiftRight);
            // getting up (Ctrl+Shift+G) and, on foot, the walker's keys
            if self.foot_key(code, pressed, repeat, ctrl, shift_now) {
                return;
            }
            // OMSI's global actions as `Inputs/keyboard.cfg` binds them ([game]); a key our
            // driving layout uses keeps that meaning (with the OMSI layout, every binding
            // counts)
            if pressed && !repeat {
                // (a modifier key pressed is a key of its own, not its own modifier: Shift
                // bound to gear_up in keyboard.cfg came as Shift+Shift and matched nothing,
                // #1477; OMSI fires it)
                let m = omsi_content::input::chord(
                    shift_now && !matches!(code, KeyCode::ShiftLeft | KeyCode::ShiftRight),
                    ctrl && !matches!(code, KeyCode::ControlLeft | KeyCode::ControlRight),
                    alt && !matches!(code, KeyCode::AltLeft | KeyCode::AltRight),
                );
                let own = keys::dik_code(code).is_some_and(|s| self.own_keys.contains(&s));
                let ours = self.args.drive_keys != "omsi"
                    && m == 0
                    && !own
                    && (fallback_action(code, &self.args.drive_keys).is_some()
                    || matches!(code, KeyCode::KeyZ | KeyCode::KeyX | KeyCode::KeyC | KeyCode::KeyI | KeyCode::KeyL));
                // plain Left/Right are OMSI's view_interiorcam_minus/plus, except when a wheel
                // steers: then the arrows glance (held, the head turns) and only Ctrl+Left/Right
                // switch the interior camera, below. (Where the arrows drive, `ours` skips this.)
                // (unless the settings ask for the cameras on them all the same, #1345)
                let plain_arrow = matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight) && !ctrl
                    && !self.settings.arrows_switch_cams
                    && self.controllers.as_ref().is_some_and(|c| c.wheel_steering());
                // (the keys that fly the camera are the camera's, unmodified: S, OMSI's
                // view_toggle_viewpoint, threw the free camera back to the driver's view,
                // and with no bus of one's own every view flies - #868; a chord such as
                // Ctrl+S, OMSI's quicksave, stays a [game] key)
                let flying = m == 0
                    && flies_free_camera(code)
                    && (self.view == "free" || (self.player.is_none() && self.on_foot.is_none()));
                // (Ctrl+Alt+arrows turn the mirror looked at: not Ctrl+arrow's gear or camera)
                let mirror_aim = ctrl && alt && matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::ArrowUp | KeyCode::ArrowDown);
                if let Some(scan) = keys::dik_code(code).filter(|_| !ours && !flying && !mirror_aim) {
                    let action = self.game_keys.iter().find(|b| b.scan_code == scan && b.matches(m)
                        && !b.action.starts_with("vr_")
                        && !(plain_arrow && b.action.starts_with("view_interiorcam_"))).map(|b| b.action.clone());
                    // (a key bound in [game] and in [vehicles] does both, as in OMSI: the
                    // parking brake put on Space, the stock view_reset_all_directions key,
                    // reset the view and never reached the bus - #745)
                    let vehicle_too = self.player.as_ref().is_some_and(|p| p.bindings.iter().any(|b| b.scan_code == scan && b.matches(m)));
                    log::debug!("key {code:?} (DIK {scan}, chord {m}): [game] {action:?}, a key of the bus too: {vehicle_too}; [game] keys on it: {:?}", self.game_keys.iter().filter(|b| b.scan_code == scan).collect::<Vec<_>>());
                    // OMSI's `exit` (Ctrl+Q, or what the player put it on): the game ends as
                    // the menu's Quit ends it. It was no action here at all, so the key did
                    // nothing (#817)
                    if action.as_deref() == Some("exit") {
                        self.finish_vr_nav_edit();
                        self.game_menu = None;
                        self.finish_session();
                        crate::platform::exit(event_loop);
                        return;
                    }
                    if let Some(a) = action {
                        if self.game_action(&a) && !vehicle_too {
                            return;
                        }
                    }
                }
            }
            if pressed && !repeat {
                match code {
                    // OMSI's `toggel_mouse_ctrl` (O): steering and pedals with the mouse
                    KeyCode::KeyO if !ctrl && !alt && !shift_now && self.key_left_free(code, "toggel_mouse_ctrl") => {
                        self.game_action("toggel_mouse_ctrl");
                        return;
                    }
                    // (a manual gearbox's Ctrl+Up / Ctrl+Down are the [game] keys `gear_up` and
                    // `gear_down` now, which can be moved: see `with_game_defaults`, #907)
                    // (Ctrl+Alt+arrows turn the mirror looked at, see the frame)
                    KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::ArrowUp | KeyCode::ArrowDown if ctrl && alt => return,
                    // the interior cameras: Ctrl+Left/Right as well (the arrows drive) - unless
                    // the player gave that combination to something else (#907)
                    KeyCode::ArrowLeft | KeyCode::ArrowRight if ctrl && !self.chord_bound(code, shift_now, ctrl, alt) => {
                        self.game_action(if code == KeyCode::ArrowLeft { "view_interiorcam_minus" } else { "view_interiorcam_plus" });
                        return;
                    }
                    // OMSI's `screenshot` (Ctrl+Shift+P: 25 / 6), and F12 as most games have it
                    KeyCode::KeyP if ctrl && shift_now => {
                        self.take_screenshot();
                        return;
                    }
                    // (F12 alone only where the bus has no key of its own on it: in OMSI's
                    // keyboard.cfg it is the pram/wheelchair button, which it took away)
                    KeyCode::F12 if !self.player.as_ref().is_some_and(|p| p.bindings.iter().any(|b| b.scan_code == 88 && b.chord() == 0 && p.vehicle.ty.program.trigger(&b.action).is_some())) => {
                        self.take_screenshot();
                        return;
                    }

                    // the duty's next stop given up (#1015), as the game menu's line ("H" for
                    // Haltestelle: Ctrl+Shift+N is the VR navigator's)
                    KeyCode::KeyH if ctrl && shift_now && !alt && self.duty.is_some() && !self.chord_bound(code, shift_now, ctrl, alt) => {
                        self.skip_next_stop();
                        return;
                    }
                    // the object editor (`crate::editor`)
                    KeyCode::KeyE if ctrl && shift_now => {
                        self.toggle_editor();
                        return;
                    }
                    // OMSI's `sim_pause`
                    KeyCode::KeyP if !ctrl && !alt && !shift_now => {
                        self.toggle_pause();
                        return;
                    }
                    // OMSI's `quicksave` (Alt+S)
                    KeyCode::KeyS if alt && !ctrl => {
                        self.quick_save();
                        return;
                    }
                    // OMSI's `view_toggle_informationdisplay` (Ctrl+Y)
                    // OMSI's `view_toggle_informationdisplay` (Shift+Y: 21 / 2)
                    KeyCode::KeyY if shift_now && !ctrl => {
                        self.set_info_bar(!self.info_bar);
                        return;
                    }
                    // OMSI's `view_set_schedule` (Insert: 210 / 1, the key's state every frame),
                    // only where keyboard.cfg has no entry for it: an entry is the player's
                    // binding, handled above, and one with scan code 0 is unbound - Insert opened
                    // the timetable all the same (#1245)
                    KeyCode::Insert if !shift_now && !ctrl && !self.game_keys.iter().any(|b| b.action.eq_ignore_ascii_case("view_set_schedule")) => {
                        self.timetable = !self.timetable;
                        return;
                    }
                    _ => {}
                }
            }
            // the extra keys of the ready-made layouts (below): not with Custom controls, not on
            // a key the player bound, not with a modifier held
            let extras = self.args.drive_keys != "omsi"
                && !keys::dik_code(code).is_some_and(|s| self.own_keys.contains(&s))
                && !self.keys.iter().any(|k| matches!(k, KeyCode::ControlLeft | KeyCode::ControlRight | KeyCode::AltLeft | KeyCode::AltRight | KeyCode::ShiftLeft | KeyCode::ShiftRight));
            if pressed && !repeat {
                // Z / X / C: indicator left / hazard / right, where the hand rests
                // (OMSI's own layout wants Shift and the numpad for them). Each is a
                // toggle: pressing the same key again turns it back off, tracked in
                // `blinker_key_state` since the scripts expose separate "set"/"off"
                // triggers for left/right rather than a toggle (hazard already has a
                // dedicated toggle trigger, `blinker_warn_toggle`).
                if self.view != "free"
                    && extras
                    && matches!(code, KeyCode::KeyZ | KeyCode::KeyX | KeyCode::KeyC)
                {
                    let want: u8 = match code {
                        KeyCode::KeyZ => 1,
                        KeyCode::KeyC => 2,
                        _ => 3,
                    };
                    self.blinker(want);
                }
                // Shift + 1..9: open or close that physical door, front to back (see
                // `door_trigger_groups`); plain digits are left alone (some buses put
                // gears or numbered presets on them, `kw_s_1`/`automatic_1`).
                if self.view != "free" && shift_held_now(&self.keys) && !keys::dik_code(code).is_some_and(|s| self.own_shift.contains(&s)) {
                    if let Some(n) = digit_of(code) {
                        if let Some(p) = self.player.as_mut() {
                            let fire = p.door_key(n);
                            if !fire.is_empty() {
                                self.door_key_triggers.insert(code, fire);
                            }
                        }
                    }
                }
                // I: every saloon light circuit of the bus at once (OMSI has a key for
                // each: 7, 8, 9 - see Player::toggle_saloon_lights).
                if self.view != "free"
                    && !repeat
                    && extras
                    && code == KeyCode::KeyI
                {
                    if let Some(p) = self.player.as_mut() {
                        let msg = p.toggle_saloon_lights();
                        self.service_msg = Some((msg, 3.0));
                    }
                }
                // (F1-F4 where keyboard.cfg has no view keys; a key the player gave to
                // something else, or a view key moved elsewhere, leaves them alone, #701)
                match code {
                    KeyCode::F1 if self.key_left_free(code, "view_set_driver") => self.view = "driver".into(),
                    KeyCode::F2 if self.key_left_free(code, "view_set_passenger") => self.view = "pax".into(),
                    KeyCode::F3 if self.key_left_free(code, "view_set_outside") => self.view = "outside".into(),
                    KeyCode::F4 if self.key_left_free(code, "view_set_map") => {
                        // the free camera starts where the current view is looking
                        self.view = "free".into();
                        self.ego = false;
                    }
                    KeyCode::KeyU
                    if self.keys.contains(&KeyCode::ShiftLeft)
                        || self.keys.contains(&KeyCode::ShiftRight) =>
                        {
                            // Shift+U: toggle a bus's service state by itself (start a shut
                            // bus, shut down a running one), and set its IBIS to the current
                            // duty as the driver would do while putting it into service.
                            if let Some(p) = self.player.as_mut() {
                                let msg = p.start_up();
                                self.service_msg = Some((msg, 6.0));
                                if let Some(d) = self.duty.as_ref() {
                                    let (trip, stop) = d.trip_for_ibis();
                                    p.set_duty_destination(trip, stop);
                                }
                            }
                        }

                    KeyCode::KeyR
                    if self.keys.contains(&KeyCode::ShiftLeft)
                        || self.keys.contains(&KeyCode::ShiftRight) =>
                        {
                            // Shift+R: the next internet radio station (see radio.rs)
                            let msg = self.radio.next_station();
                            self.service_msg = Some((msg, 4.0));
                        }
                    KeyCode::KeyM
                    if self.keys.contains(&KeyCode::ShiftLeft)
                        || self.keys.contains(&KeyCode::ShiftRight) =>
                        {
                            // Shift+M: the city map (M alone is the starter)
                            if let Some(n) = self.navigator.as_mut() {
                                n.toggle_map();
                            }
                        }
                    KeyCode::KeyN
                    if self.keys.contains(&KeyCode::ShiftLeft)
                        || self.keys.contains(&KeyCode::ShiftRight) =>
                        {
                            // Shift+N: navigator → navigator with the schedule → off (N alone is
                            // the gearbox's neutral)
                            if self.cycle_navigator() {
                                return;
                            }
                        }
                    KeyCode::F11 => {
                        // (Ctrl+F11; F11 alone is OMSI's pedestrian view)
                        // where am I: so a place that looks wrong can be named
                        if let Some(cam) = self.camera.as_ref() {
                            let ts = omsi_map::tile_size();
                            let (tx, ty) = (
                                (cam.position.x / ts).floor() as i32,
                                (cam.position.y / ts).floor() as i32,
                            );
                            let line = format!(
                                "Position {:.0}, {:.0}, {:.1}   tile {tx}_{ty}   heading {:.0} deg",
                                cam.position.x, cam.position.y, cam.position.z, cam.yaw
                            );
                            log::info!(
                                "{line}  (--cam {:.0},{:.0},{:.0},{:.0},{:.0})",
                                cam.position.x,
                                cam.position.y,
                                cam.position.z,
                                cam.yaw,
                                cam.pitch
                            );
                            self.service_msg = Some((line, 12.0));
                        }
                    }
                    KeyCode::F9 => {
                        // write the run into the driver's personnel file
                        let line = self.career.summary();
                        if self.career.path.is_some() {
                            if let Err(e) = self.career.save() {
                                log::warn!("writing the personnel file: {e}");
                            }
                        } else {
                            log::info!("this run: {line}");
                        }
                        self.service_msg = Some((line, 8.0));
                    }
                    // F5-F8 are the destination sign and roller blind keys of OMSI's
                    // keyboard.cfg (bus_linie_minus/plus, bus_ziel_minus/plus,
                    // bus_rollband_setL1..T): they go to the bus below and nowhere else. The
                    // depot services are in the game menu (Esc), as in OMSI's menu.
                    _ => {}
                }
            }
            // the arrow keys drive when a bus is being driven (the free camera keeps them)
            // (a key the player bound to something else is theirs, not the preset's)
            let own = keys::dik_code(code).is_some_and(|s| self.own_keys.contains(&s));
            let wheel = self.controllers.as_ref().is_some_and(|c| c.wheel_steering());
            let wasd = if own {
                "omsi"
            } else if wheel {
                // (with a wheel steering, the arrow keys are OMSI's: they look around)
                match self.args.drive_keys.as_str() {
                    "arrows" | "omsi" => "omsi",
                    _ => "wasd",
                }
            } else {
                self.args.drive_keys.as_str()
            };
            let shift_held =
                self.keys.contains(&KeyCode::ShiftLeft) || self.keys.contains(&KeyCode::ShiftRight);
            if let Some(p) = self.player.as_mut() {
                let ctrl_alt_held = (self.keys.contains(&KeyCode::ControlLeft) || self.keys.contains(&KeyCode::ControlRight)) && (self.keys.contains(&KeyCode::AltLeft) || self.keys.contains(&KeyCode::AltRight));
                if self.view != "free" && !repeat && !shift_held && !(ctrl_alt_held && pressed) {
                    if let Some(a) = fallback_action(code, wasd) {
                        p.axes.set(a, pressed);
                    }
                } else if !pressed {
                    // a driving key let go always lets go: released while Shift was held (or
                    // in the free view) it stayed "pressed", and the wheel went on turning to
                    // full lock until that key was pressed again (#1040, #1050, #1053)
                    if let Some(a) = fallback_action(code, wasd) {
                        p.axes.set(a, false);
                    }
                }
            }
            // A driving key held with shift is the vehicle key it covers: Shift+W is
            // OMSI's wiper key, Shift+D selects the automatic's D, Shift+S the
            // viewpoint - otherwise a bus driven with WASD could never be put in gear.
            let shift =
                self.keys.contains(&KeyCode::ShiftLeft) || self.keys.contains(&KeyCode::ShiftRight);
            let covers_vehicle_key = fallback_action(code, wasd).is_some() && self.view != "free";
            let driving_key = covers_vehicle_key && !shift;
            // (the keys that fly the free camera are the camera's: W switched the wipers on
            // while flying)
            let fly_key = self.view == "free" && flies_free_camera(code);
            if let (Some(p), Some(scan)) = (
                self.player.as_mut(),
                keys::dik_code(code).filter(|_| !driving_key && !fly_key),
            ) {
                if !repeat {
                    let m = if covers_vehicle_key {
                        0
                    } else {
                        // (a modifier key is a key of its own here, not its own modifier, #1477)
                        omsi_content::input::chord(
                            shift && !matches!(code, KeyCode::ShiftLeft | KeyCode::ShiftRight),
                            (self.keys.contains(&KeyCode::ControlLeft) || self.keys.contains(&KeyCode::ControlRight))
                                && !matches!(code, KeyCode::ControlLeft | KeyCode::ControlRight),
                            (self.keys.contains(&KeyCode::AltLeft) || self.keys.contains(&KeyCode::AltRight))
                                && !matches!(code, KeyCode::AltLeft | KeyCode::AltRight),
                        )
                    };
                    p.key(scan, m, pressed);
                }
            }
        }
    }

    /// Whether a key the game gives `action` by itself (F1 the driver's view, O the mouse
    /// steering) is still free for it: neither bound by the player to something of their
    /// own nor `action` bound to another key in keyboard.cfg.
    pub(crate) fn key_left_free(&self, code: KeyCode, action: &str) -> bool {
        key_left_free(keys::dik_code(code), action, &self.own_keys, &self.game_keys)
    }

    pub(crate) fn icao_edit_key(&mut self,code:KeyCode){
        match code{
            KeyCode::Escape=>{self.menu_edit=None;self.menu_edit_icao=false;if let Some(w)=self.window.as_ref(){w.set_ime_allowed(false);}},
            KeyCode::Backspace|KeyCode::Delete=>{if let Some(d)=self.menu_edit.as_mut(){d.pop();}},
            KeyCode::Enter|KeyCode::NumpadEnter=>{self.apply_icao_edit();return;},
            _=>{}
        }
        self.refresh_list();
    }

    pub(crate) fn icao_edit_text(&mut self,text:&str){
        if !self.menu_edit_icao{return}
        if let Some(d)=self.menu_edit.as_mut(){
            for c in text.chars().filter(|c|c.is_ascii_alphabetic()){
                if d.len()>=4{break} d.push(c.to_ascii_uppercase());
            }
        }
        self.refresh_list();
    }

    pub(crate) fn time_edit_key(&mut self, code: KeyCode) {
        let digit = match code {
            KeyCode::Digit0 | KeyCode::Numpad0 => Some('0'),
            KeyCode::Digit1 | KeyCode::Numpad1 => Some('1'),
            KeyCode::Digit2 | KeyCode::Numpad2 => Some('2'),
            KeyCode::Digit3 | KeyCode::Numpad3 => Some('3'),
            KeyCode::Digit4 | KeyCode::Numpad4 => Some('4'),
            KeyCode::Digit5 | KeyCode::Numpad5 => Some('5'),
            KeyCode::Digit6 | KeyCode::Numpad6 => Some('6'),
            KeyCode::Digit7 | KeyCode::Numpad7 => Some('7'),
            KeyCode::Digit8 | KeyCode::Numpad8 => Some('8'),
            KeyCode::Digit9 | KeyCode::Numpad9 => Some('9'),
            _ => None,
        };
        match code {
            KeyCode::Escape => self.menu_edit = None,
            KeyCode::Backspace | KeyCode::Delete => {
                if let Some(d) = self.menu_edit.as_mut() {
                    d.pop();
                }
            }
            KeyCode::Enter | KeyCode::NumpadEnter => {
                self.apply_time_edit();
                return;
            }
            _ => {
                if let (Some(c), Some(d)) = (digit, self.menu_edit.as_mut()) {
                    if d.len() < 6 {
                        d.push(c);
                    }
                }
            }
        }
        self.refresh_list();
    }

    /// A key while a route number is typed in the destination list (#836). Printable
    /// text comes through `route_edit_text` so keyboard layouts and symbols are preserved;
    /// physical key codes remain a fallback for platforms that do not provide text.
    pub(crate) fn route_edit_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Escape => self.menu_edit = None,
            KeyCode::Backspace | KeyCode::Delete => {
                if let Some(t) = self.menu_edit.as_mut() {
                    t.pop();
                }
            }
            KeyCode::Enter | KeyCode::NumpadEnter => {
                if let Some(t) = self.menu_edit.take() {
                    crate::game_lists::set_route_by_hand(self, &t);
                    self.close_game_menu();
                }
                return;
            }
            _ => {
                if let (Some(c), Some(t)) = (route_char(code), self.menu_edit.as_mut()) {
                    if t.chars().count() < ROUTE_NUMBER_MAX {
                        t.push(c);
                    }
                }
            }
        }
        self.refresh_list();
    }

    /// Text entered in OMSI's free route-number field. It is intentionally not restricted
    /// to letters and digits: add-on displays use values such as `-10` and other symbols.
    pub(crate) fn route_edit_text(&mut self, text: &str) {
        if !matches!(self.list_kind, Some(crate::game_lists::ListKind::RouteNumbers)) || self.menu_edit.is_none() {
            return;
        }
        if let Some(t) = self.menu_edit.as_mut() {
            for c in text.chars().filter(|c| !c.is_control()) {
                if t.chars().count() >= ROUTE_NUMBER_MAX {
                    break;
                }
                t.push(c);
            }
        }
        self.refresh_list();
    }

    /// A key while the vehicle chooser is open.
    pub(crate) fn chooser_key(&mut self, code: KeyCode) {
        if self.dropdown.is_some() {
            self.dropdown_key(code);
            return;
        }
        if self.menu_edit.is_some() {
            if self.menu_edit_icao {
                self.icao_edit_key(code);
            } else if matches!(self.list_kind, Some(crate::game_lists::ListKind::RouteNumbers)) {
                self.route_edit_key(code);
            } else {
                self.time_edit_key(code);
            }
            return;
        }
        let n = self.admin_list.as_ref().unwrap_or(&self.vehicle_list).len().max(1);
        let sel = self.chooser.unwrap_or(0);
        self.menu_top = None;
        match code {
            KeyCode::Escape if crate::game_controller_menu::is_controller_list(self.list_kind.as_ref()) || matches!(self.list_kind, Some(crate::game_lists::ListKind::Events | crate::game_lists::ListKind::Keyboard(_))) => {
                if let Some(kind) = self.list_kind.clone() {
                    if let Some(back) = crate::game_lists::run(self, &kind, "back") {
                        self.open_list(back);
                    }
                }
            }
            KeyCode::Escape => {
                if self.tours_list() {
                    self.open_list(crate::game_lists::ListKind::Lines);
                } else {
                    self.chooser = None;
                    self.admin_list = None;
                    self.list_kind = None;
                }
            }
            KeyCode::ArrowUp | KeyCode::KeyW => self.chooser = Some(self.chooser_next(sel, n - 1)),
            KeyCode::ArrowDown | KeyCode::KeyS => self.chooser = Some(self.chooser_next(sel, 1)),
            KeyCode::ArrowLeft | KeyCode::KeyA if self.settings_list() => self.list_adjust(sel, crate::game_lists::Move::Dec),
            KeyCode::ArrowRight | KeyCode::KeyD if self.settings_list() => self.list_adjust(sel, crate::game_lists::Move::Inc),
            KeyCode::Tab if self.settings_list() => {
                let back = self.keys.contains(&KeyCode::ShiftLeft) || self.keys.contains(&KeyCode::ShiftRight);
                self.settings_tab_step(!back);
            }
            // (the stop to start from: - and +)
            KeyCode::Minus | KeyCode::Slash | KeyCode::NumpadSubtract if self.tours_list() => self.tour_stop_step(false),
            KeyCode::Equal | KeyCode::BracketRight | KeyCode::NumpadAdd if self.tours_list() => self.tour_stop_step(true),
            // (the trip of the tour: the arrows go to the one leaving before or after, as OMSI's)
            KeyCode::ArrowLeft | KeyCode::KeyA if self.tours_list() => self.trip_step(false),
            KeyCode::ArrowRight | KeyCode::KeyD if self.tours_list() => self.trip_step(true),
            KeyCode::ArrowLeft | KeyCode::KeyA => self.chooser_adjust(sel, "-"),
            KeyCode::ArrowRight | KeyCode::KeyD => self.chooser_adjust(sel, "+"),
            KeyCode::PageUp if self.settings_list() => self.settings_tab_step(false),
            KeyCode::PageDown if self.settings_list() => self.settings_tab_step(true),
            KeyCode::PageUp => self.chooser = Some(sel.saturating_sub(15)).map(|k| if self.is_heading(k) { self.chooser_next(k, 1) } else { k }),
            KeyCode::PageDown => self.chooser = Some((sel + 15).min(n - 1)).map(|k| if self.is_heading(k) { self.chooser_next(k, 1) } else { k }),
            KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space => self.chooser_pick(sel),
            _ => {}
        }
    }

    /// A key while a drop-down is open: the arrows choose, Enter takes, Esc closes it.
    pub(crate) fn dropdown_key(&mut self, code: KeyCode) {
        let Some(d) = self.dropdown.as_mut() else { return };
        let n = d.items.len().max(1);
        match code {
            KeyCode::Escape => {
                self.dropdown = None;
                return;
            }
            KeyCode::ArrowUp | KeyCode::KeyW => d.sel = (d.sel + n - 1) % n,
            KeyCode::ArrowDown | KeyCode::KeyS => d.sel = (d.sel + 1) % n,
            KeyCode::PageUp => d.sel = d.sel.saturating_sub(5),
            KeyCode::PageDown => d.sel = (d.sel + 5).min(n - 1),
            KeyCode::Home => d.sel = 0,
            KeyCode::End => d.sel = n - 1,
            KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space => {
                let i = d.sel;
                self.dropdown_pick(i);
                return;
            }
            _ => return,
        }
        self.dd_reveal();
    }

    /// The drop-down's chosen entry is in view.
    pub(crate) fn dd_reveal(&mut self) {
        let rows = self.ui.as_ref().map(|u| u.dd_rows).unwrap_or(8).max(1);
        if let Some(d) = self.dropdown.as_mut() {
            if d.sel < d.top {
                d.top = d.sel;
            } else if d.sel >= d.top + rows {
                d.top = d.sel + 1 - rows;
            }
        }
    }

    pub(crate) fn menu_key(&mut self, event_loop: &ActiveEventLoop, code: KeyCode) {
        self.menu_kbd = true;
        if self.key_capture.is_some() {
            match code {
                KeyCode::ShiftLeft | KeyCode::ShiftRight | KeyCode::ControlLeft | KeyCode::ControlRight | KeyCode::AltLeft | KeyCode::AltRight => {}
                KeyCode::Escape => self.cancel_key_capture(),
                KeyCode::Delete | KeyCode::Backspace => self.apply_key_capture(None, 0),
                _ => match crate::keys::dik_code(code) {
                    Some(scan) => {
                        let chord = omsi_content::input::chord(
                            self.keys.contains(&KeyCode::ShiftLeft) || self.keys.contains(&KeyCode::ShiftRight),
                            self.keys.contains(&KeyCode::ControlLeft) || self.keys.contains(&KeyCode::ControlRight),
                            self.keys.contains(&KeyCode::AltLeft) || self.keys.contains(&KeyCode::AltRight),
                        );
                        self.apply_key_capture(Some(scan), chord);
                    }
                    None => self.service_msg = Some(("That key has no DirectInput scan code".into(), 3.0)),
                },
            }
            return;
        }
        if self.chooser.is_some() {
            self.chooser_key(code);
            return;
        }
        let n = self.game_menu_items().len();
        let sel = self.game_menu.unwrap_or(0);
        let modified = self.keys.iter().any(|key| {
            matches!(*key, KeyCode::ControlLeft | KeyCode::ControlRight | KeyCode::AltLeft | KeyCode::AltRight | KeyCode::ShiftLeft | KeyCode::ShiftRight)
        });
        self.menu_top = None;
        match code {
            // P changes only the simulation state, even while a menu is open.
            KeyCode::KeyP if !modified => self.toggle_pause(),
            KeyCode::Escape => self.close_game_menu(),
            KeyCode::ArrowUp | KeyCode::KeyW => {
                let k = if self.new_menu_active() { self.new_menu_step(sel, false) } else { self.menu_step(sel, n, false) };
                self.game_menu = Some(k);
            }
            KeyCode::ArrowDown | KeyCode::KeyS => {
                let k = if self.new_menu_active() { self.new_menu_step(sel, true) } else { self.menu_step(sel, n, true) };
                self.game_menu = Some(k);
            }
            KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space => self.menu_choose(event_loop, sel),
            _ => {}
        }
    }

    /// OMSI's `sim_pause`: the simulation stands still, the camera and the picture go on.
    /// Shift a manual gearbox up or down: the first of the usual trigger names the bus's
    /// scripts have (pressed and let go). False when it has none.
    /// Whether `code` with these modifiers is one of the keyboard file's keys, of the game's
    /// or of the bus's: then a built-in shortcut on it stands back.
    pub(crate) fn chord_bound(&self, code: KeyCode, shift: bool, ctrl: bool, alt: bool) -> bool {
        let m = omsi_content::input::chord(shift, ctrl, alt);
        let Some(scan) = keys::dik_code(code) else { return false };
        self.game_keys.iter().any(|b| b.scan_code == scan && b.matches(m))
            || self.player.as_ref().is_some_and(|p| p.bindings.iter().any(|b| b.scan_code == scan && b.matches(m)))
    }
}
