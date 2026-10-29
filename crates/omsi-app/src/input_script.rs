//! `OMSI_INPUT`: scripted keyboard, mouse and camera input for window runs, and its key names.

use crate::actions::dump_display_textures;
use crate::app::App;
use crate::describe;
use crate::output::{ORBIT_MAX, ORBIT_MIN};
use std::path::{Path, PathBuf};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::KeyCode;

impl App {
    /// A key of the input script by name: a letter, a digit, F1..F12, or one of the named keys.
    pub(crate) fn script_key(name: &str) -> Option<KeyCode> {
        use KeyCode::*;
        pub(crate) const LETTERS: [KeyCode; 26] = [
            KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM, KeyN,
            KeyO, KeyP, KeyQ, KeyR, KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ,
        ];
        pub(crate) const DIGITS: [KeyCode; 10] = [
            Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9,
        ];
        pub(crate) const FKEYS: [KeyCode; 12] = [F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12];
        let b = name.as_bytes();
        if b.len() == 1 && b[0].is_ascii_alphabetic() {
            return Some(LETTERS[(b[0].to_ascii_uppercase() - b'A') as usize]);
        }
        if b.len() == 1 && b[0].is_ascii_digit() {
            return Some(DIGITS[(b[0] - b'0') as usize]);
        }
        if let Some(n) = name.strip_prefix('F').and_then(|n| n.parse::<usize>().ok()) {
            return FKEYS.get(n.wrapping_sub(1)).copied();
        }
        Some(match name {
            "Shift" => ShiftLeft,
            "Ctrl" => ControlLeft,
            "Alt" => AltLeft,
            "." => Period,
            "," => Comma,
            "Up" => ArrowUp,
            "Down" => ArrowDown,
            "Left" => ArrowLeft,
            "Right" => ArrowRight,
            "Enter" => Enter,
            "Escape" => Escape,
            "Backspace" => Backspace,
            "Space" => Space,
            "PageUp" => PageUp,
            "PageDown" => PageDown,
            "Insert" => Insert,
            "Home" => Home,
            "End" => End,
            "Delete" => Delete,
            "[" => BracketLeft,
            "]" => BracketRight,
            _ => return None,
        })
    }

    /// `OMSI_INPUT`: scripted window input, so the very same handlers the mouse and keyboard
    /// reach can be driven from the command line and checked without a hand on the mouse:
    /// `t=3 move 1045,826; t=3.2 press; t=3.5 drag 0,-80; t=4 release; t=4.5 log bremse_feststell;
    /// t=5 key F3` - coordinates in logical pixels, `drag` relative, `key` a winit key name;
    /// also `look yaw,pitch` (turn the head to), `turn dx,dy` (turn the view by degrees, as a
    /// right-button drag does, the free and outside cameras too), `set name=value` (a script
    /// variable), `trigger name`, `log name` (with what the HUD says about the cursor),
    /// `dumptex <folder>` (the display pictures), `shot <file>` (the window's picture as a
    /// PNG) and `type <text>` (into the LAN chat line that `key V` opened; `key Enter`
    /// sends it).
    pub(crate) fn run_input_script(&mut self, event_loop: &ActiveEventLoop) {
        if self.input_script.is_empty() {
            return;
        }
        let t = self.started.elapsed().as_secs_f32();
        let scale = self
            .window
            .as_ref()
            .map(|w| w.scale_factor() as f32)
            .unwrap_or(1.0);
        while let Some((at, cmd)) = self.input_script.first().cloned() {
            if t < at {
                break;
            }
            self.input_script.remove(0);
            let mut parts = cmd.split_whitespace();
            let verb = parts.next().unwrap_or("");
            let arg = parts.next().unwrap_or("");
            let xy = || -> (f32, f32) {
                let mut it = arg.split(',').filter_map(|v| v.trim().parse::<f32>().ok());
                (it.next().unwrap_or(0.0), it.next().unwrap_or(0.0))
            };
            log::info!("input script t={t:.1}: {cmd}");
            match verb {
                "move" => {
                    let (x, y) = xy();
                    self.on_cursor(x * scale, y * scale);
                }
                // `weather`: the next weather, as the admin menu's "Next weather"
                "weather" => self.next_weather(),
                "drag" => {
                    let (dx, dy) = xy();
                    let (x, y) = self.cursor;
                    self.on_cursor(x + dx * scale, y + dy * scale);
                }
                // `look yaw,pitch`: turn the head, as a right-drag does
                // `touch down|move|up x,y[,id]`: a finger (logical pixels), as the phone's
                // screen gives it to the on-screen controls (OMSI_TOUCH=1 shows them)
                "touch" => {
                    let rest = parts.next().unwrap_or("");
                    let mut it = rest.split(',').filter_map(|v| v.trim().parse::<f32>().ok());
                    let (x, y) = (it.next().unwrap_or(0.0), it.next().unwrap_or(0.0));
                    let id = it.next().unwrap_or(0.0) as u64;
                    self.script_touch(event_loop, arg, x * scale, y * scale, id);
                }
                "look" => self.look = xy(),
                // `orbit <m>`: how far the outside camera stands off, as the mouse wheel sets it
                "orbit" => self.orbit = xy().0.clamp(ORBIT_MIN, ORBIT_MAX),
                // `set name=value`: put a script variable somewhere (a switch half way)
                "set" => {
                    if let (Some((k, v)), Some(p)) = (arg.split_once('='), self.player.as_mut()) {
                        let ok = p.vehicle.set_var(k.trim(), v.trim().parse().unwrap_or(0.0));
                        log::info!("input script: set {k} -> {ok}");
                    }
                }
                "trigger" => {
                    if let Some(p) = self.player.as_mut() {
                        let ok = p.vehicle.trigger(arg);
                        log::info!("input script: trigger {arg} -> {ok}");
                    }
                }
                // `wheel <notches>`: the mouse wheel, where the placing and the menu take it
                "wheel" => {
                    let n = xy().0;
                    if self.editor.is_some() && self.game_menu.is_none() {
                        self.editor_wheel(n);
                    } else if self.placing.is_some() && self.game_menu.is_none() {
                        self.placing_wheel(n);
                    } else if self.game_menu.is_some() {
                        self.menu_wheel(n);
                    } else {
                        self.wheel(n);
                    }
                    log::info!("input script: wheel {n}: menu line {:?}, chooser {:?}, placing heading {:?}", self.game_menu, self.chooser, self.placing.as_ref().map(|p| p.heading));
                }
                // `click`: a left click where the cursor is, through the window's own path;
                // `click down` / `click up` only press or let go (a drag in between)
                "click" => {
                    let (press, release) = (arg != "up", arg != "down");
                    if self.placing.is_some() && self.game_menu.is_none() {
                        self.placing_click();
                    } else if self.game_menu.is_some() {
                        // (on the menu as the window's button: its lines, its arrows)
                        if press {
                            self.left_button(event_loop, true);
                        }
                        if release {
                            self.left_button(event_loop, false);
                        }
                    } else {
                        if press {
                            self.on_left(true);
                        }
                        if release {
                            self.on_left(false);
                        }
                    }
                    log::info!(
                        "input script: click: placing {:?}, placed at {:?}",
                        self.placing.as_ref().map(|p| (p.at, p.blocked)),
                        self.placed
                            .last()
                            .map(|q| (q.vehicle.position, q.vehicle.heading))
                    );
                }
                // `both down|up`: both mouse buttons held (OMSI's mouse zoom) or let go
                "both" => {
                    if arg == "down" {
                        self.buttons_held = (true, true);
                        let started = self.start_both_drag();
                        log::info!("input script: both buttons: zoom drag {started}");
                    } else {
                        self.buttons_held = (false, false);
                        self.both_drag = None;
                        log::info!(
                            "input script: both buttons up: zoom {:?}, orbit {:.1}",
                            self.view_zoom.get(&self.view),
                            self.orbit
                        );
                    }
                }
                // `right down|up`: the right mouse button, through the window's own path
                "right" => {
                    self.on_right(arg == "down");
                    log::info!("input script: right button {arg}: zoom drag {}, look {}, zoom {:?}, orbit {:.1}", self.both_drag.is_some(), self.mouse_look, self.view_zoom.get(&self.view), self.orbit);
                }
                "press" => self.on_left(true),
                "release" => self.on_left(false),
                // `type <text>`: characters into the open LAN chat line (after `key V`)
                "type" => {
                    let text = cmd.split_once(' ').map(|x| x.1).unwrap_or("");
                    {
                        let _ = text;
                        log::warn!("input script: the chat line is not open");
                    }
                }
                // `turn dx,dy`: turn the view by degrees, as a right-button drag does
                "turn" => {
                    let (dx, dy) = xy();
                    self.look_by(dx, dy);
                }
                // `focus 0|1`, `minimize`, `restore`: the window losing and getting back the
                // keyboard and the mouse, as the window's own events do it
                "focus" if arg == "0" => self.input_lost(),
                "focus" => self.input_back(),
                "minimize" => {
                    self.window_hidden = true;
                    self.input_lost();
                }
                "restore" => {
                    self.window_hidden = false;
                    self.input_back();
                }
                "key" | "keydown" | "keyup" => {
                    let Some(code) = Self::script_key(arg) else {
                        log::warn!("input script: unknown key {arg}");
                        continue;
                    };
                    if verb != "keyup" {
                        self.on_key(event_loop, code, true, false);
                    }
                    if verb != "keydown" {
                        self.on_key(event_loop, code, false, false);
                    }
                }
                // `log pose`: where the bus and the camera are, and whether the bus has ground
                "log" if arg == "pose" => {
                    let bus = self.player.as_ref().map(|p| {
                        (
                            p.vehicle.position,
                            p.vehicle
                                .ground
                                .as_ref()
                                .and_then(|g| g(p.vehicle.position.x, p.vehicle.position.y)),
                        )
                    });
                    let tiles = self
                        .world
                        .as_ref()
                        .map(|w| w.loaded_tiles().len())
                        .unwrap_or(0);
                    log::info!("input script: bus at {:?} (ground {:?}), camera at {:?}, {tiles} tiles loaded", bus.map(|b| b.0), bus.and_then(|b| b.1), self.camera.as_ref().map(|c| c.position));
                    // the camera in the bus's own frame (x right, y forward, z up)
                    if let (Some(p), Some(c)) = (self.player.as_ref(), self.camera.as_ref()) {
                        let d = c.position - p.vehicle.position;
                        let h = p.vehicle.heading.to_radians();
                        let (fwd, right) = (
                            glam::DVec2::new(h.sin(), h.cos()),
                            glam::DVec2::new(h.cos(), -h.sin()),
                        );
                        log::info!("input script: view {} camera in the bus ({:.2}, {:.2}, {:.2}), on foot {:?}", self.view, d.truncate().dot(right), d.truncate().dot(fwd), d.z, self.on_foot.as_ref().map(|f| f.pos));
                    }
                }
                // `log mouse`: the mouse steering's state
                "log" if arg == "mouse" => {
                    log::info!(
                        "input script: mouse steering {} look {} menu {:?} paused {} focused {} steer {:.3}",
                        self.mouse_drive,
                        self.mouse_look,
                        self.game_menu,
                        self.paused,
                        self.window_focused,
                        self.mouse_steer.0
                    );
                }
                "log" => {
                    let v = self
                        .player
                        .as_ref()
                        .map(|p| (p.vehicle.var(arg), p.vehicle.str_var(arg)));
                    let names = describe::names(&self.args.root, &self.settings.language);
                    let shown = self
                        .hover
                        .as_deref()
                        .map(|h| names.control(h))
                        .or_else(|| self.hover_part.as_deref().map(|p| names.part(p)));
                    log::info!(
                        "input script: {arg} = {:?}  hover {:?} / {:?} shown as {:?}",
                        v,
                        self.hover,
                        self.hover_part,
                        shown
                    );
                }
                // `menu <what>`: a line of the game menu by its id (`menu remove`, `menu switch`),
                // `menu pick:<n>`: line n of the list open
                "menu" => {
                    if let Some(n) = arg
                        .strip_prefix("pick:")
                        .and_then(|n| n.parse::<usize>().ok())
                    {
                        self.chooser_pick(n);
                    } else {
                        if self.game_menu.is_none() {
                            self.open_game_menu();
                        }
                        // (a line of the vehicle or world pages is done directly)
                        match self.game_menu_items().iter().position(|m| m.0 == arg) {
                            Some(k) => self.menu_choose(event_loop, k),
                            None => {
                                if !self.page_action(arg) {
                                    log::warn!("input script: no menu line {arg}");
                                }
                            }
                        }
                    }
                    let riders = self.humans.as_ref().map(|h| {
                        (
                            h.people_in(crate::humans::BusId::Player),
                            self.placed
                                .iter()
                                .map(|q| {
                                    h.people_in(crate::humans::BusId::Ai(
                                        crate::humans::placed_bus_id(q.uid),
                                    ))
                                })
                                .collect::<Vec<_>>(),
                        )
                    });
                    log::info!("input script: menu {arg}: player {:?}, on foot {:?}, placed {}, people in the bus / the placed ones {:?}", self.player.as_ref().map(|p| p.vehicle.position), self.on_foot.as_ref().map(|f| f.pos), self.placed.len(), riders);
                }
                // `shot <file>`: the window's own view into a PNG, drawn from the scene the
                // window is showing (the only way to see what the window path renders)
                "shot" => self.shot = Some((PathBuf::from(arg), true)),
                // `dumptex <folder>`: the player's display pictures as the window has them
                "dumptex" => {
                    if let Some(p) = self.player.as_ref() {
                        dump_display_textures(&p.vehicle, Path::new(arg));
                    }
                }
                _ => log::warn!("input script: unknown command {cmd}"),
            }
        }
    }
}

/// `OMSI_INPUT="t=3 move 1045,826; t=3.2 press; ..."` → (time, command) pairs.
pub(crate) fn parse_input_script() -> Vec<(f32, String)> {
    let Some(v) = omsi_cfg::flags::INPUT.text() else {
        return Vec::new();
    };
    let mut out: Vec<(f32, String)> = v
        .split(';')
        .filter_map(|item| {
            let item = item.trim();
            let rest = item.strip_prefix("t=")?;
            let (t, cmd) = rest.split_once(' ')?;
            Some((t.trim().parse::<f32>().ok()?, cmd.trim().to_string()))
        })
        .collect();
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out
}
