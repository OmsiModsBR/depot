//! What the game does when asked by a key, a menu, a page or a plugin: the game actions,
//! placing and switching vehicles, saving and loading.

use crate::app::App;
use crate::camera_util::ORBIT_DEFAULT;
use crate::cli::Args;
use crate::input::part_in_reach;
use crate::player::Player;
use crate::services::{at_petrol_station, run_services};
use crate::situation::build_situation;
use crate::spawn::spawn_player;
use omsi_geometry::util::wrap_deg;
use std::path::{Path, PathBuf};
use winit::event_loop::ActiveEventLoop;

/// Global actions a controller button should send to the game instead of to the bus script.
pub(crate) fn is_game_action(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.starts_with("view_")
        || matches!(
            name.as_str(),
            "sim_pause"
                | "screenshot"
                | "quicksave"
                | "toggel_mouse_ctrl"
                | "toggel_ctrler"
                | "voice_radio"
        )
}

/// The variable a gear lever's gate triggers keep the gear in: `antrieb_getr_aktugang` (the
/// LiAZ, whose gates only move the lever and leave the gear to its frame), else
/// `antrieb_getr_gang` (the stock cars' antrieb.osc), else one named for the gear that both
/// `kw_s_1` and `kw_s_2` set.
pub(crate) fn gate_gear_var(program: &omsi_script::Program) -> Option<String> {
    for known in ["antrieb_getr_aktugang", "antrieb_getr_gang"] {
        if program.var(known).is_some() {
            return Some(known.to_string());
        }
    }
    let mut names: Vec<String> = program
        .var_names()
        .into_iter()
        .filter(|n| n.contains("gang") || n.contains("gear"))
        .filter(|n| {
            let by = program.triggers_setting(n);
            by.iter().any(|t| t == "kw_s_1") && by.iter().any(|t| t == "kw_s_2")
        })
        .collect();
    names.sort_by_key(|n| (n.len(), n.clone()));
    names.into_iter().next()
}

#[cfg(test)]
pub(crate) mod gear_lever_tests {
    /// The stock cars' gates keep the gear in `antrieb_getr_gang` (#866).
    #[test]
    fn the_gear_is_read_where_the_gates_store_it() {
        let program = |vars: &str, osc: &str| {
            let dir = std::env::temp_dir().join(format!(
                "omsi_gates_{}_{}",
                std::process::id(),
                vars.len()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            let (vl, sc) = (dir.join("varlist.txt"), dir.join("antrieb.osc"));
            std::fs::write(&vl, vars).unwrap();
            std::fs::write(&sc, osc).unwrap();
            let p = omsi_script::compile(&omsi_script::CompileInput {
                varlists: vec![vl],
                scripts: vec![sc],
                ..Default::default()
            });
            let _ = std::fs::remove_dir_all(&dir);
            p
        };
        let stock = program("antrieb_getr_gang\n", "{trigger:kw_s_1_fest}\n{trigger:kw_s_1}\n1 (S.L.antrieb_getr_gang)\n{end}\n{end}\n{trigger:kw_s_2}\n2 (S.L.antrieb_getr_gang)\n{end}\n");
        assert_eq!(
            super::gate_gear_var(&stock).as_deref(),
            Some("antrieb_getr_gang")
        );
        // a lever moved by every gate is not the gear
        let own = program("lever_moved\nmy_gear\n", "{trigger:kw_s_1}\n1 (S.L.lever_moved)\n1 (S.L.my_gear)\n{end}\n{trigger:kw_s_2}\n1 (S.L.lever_moved)\n2 (S.L.my_gear)\n{end}\n");
        assert_eq!(super::gate_gear_var(&own).as_deref(), Some("my_gear"));
    }
}

/// Write a vehicle's `[scripttexture]` images and `[texttexture]` pictures (with the text
/// they show in the log) into `dir`, to check destination displays and the IBIS.
pub(crate) fn dump_display_textures(vehicle: &omsi_sim::VehicleInstance, dir: &Path) {
    let _ = std::fs::create_dir_all(dir);
    for (i, st) in vehicle.host.script_textures.iter().enumerate() {
        let _ = image::save_buffer(
            dir.join(format!("scripttex_{i}.png")),
            &st.rgba,
            st.width,
            st.height,
            image::ColorType::Rgba8,
        );
    }
    for (i, tt) in vehicle.text_textures.iter().enumerate() {
        let text = tt.last_text.clone().unwrap_or_default();
        log::info!(
            "text texture {i} ({} in \"{}\"): {text:?}",
            tt.def.variable,
            tt.def.font
        );
        let (w, h) = (tt.def.width.max(1) as u32, tt.def.height.max(1) as u32);
        if tt.atlas.is_some() {
            let _ = image::save_buffer(
                dir.join(format!("texttex_{i}.png")),
                &tt.image(&text),
                w,
                h,
                image::ColorType::Rgba8,
            );
        }
    }
}

/// The folder of a map's save slots, inside the map's folder in the content folder (the
/// launcher reads it as well: `omsi_launcher_lib::saved_situations`).
pub(crate) const SAVES: &str = "Saves";

/// Save `sit` to `out`; where that folder takes no file, to `rel` under the fallback folder
/// (`startup::save_fallback_dir`) instead (#1673). The file written.
pub(crate) fn save_or_fallback(
    sit: &omsi_content::situation::Situation,
    out: &Path,
    rel: &Path,
) -> std::io::Result<PathBuf> {
    let first = out
        .parent()
        .map(std::fs::create_dir_all)
        .unwrap_or(Ok(()))
        .and_then(|_| sit.save(out));
    let e = match first {
        Ok(()) => return Ok(out.to_path_buf()),
        Err(e) => e,
    };
    let Some(alt) = crate::startup::save_fallback_dir()
        .map(|f| f.join(rel))
        .filter(|a| a != out)
    else {
        return Err(e);
    };
    log::warn!(
        "saving {}: {e}; saved into {} instead",
        out.display(),
        alt.display()
    );
    if let Some(d) = alt.parent() {
        std::fs::create_dir_all(d)?;
    }
    sit.save(&alt)?;
    Ok(alt)
}

impl App {
    /// Save the personnel file and the session summary (once: every caller ends the game,
    /// and the frames the loop still runs before it stops count no more time).
    pub(crate) fn finish_session(&mut self) {
        crate::game_lists::flush_settings(true);
        self.exiting = true;
        // (the tiles loaded on the way added to what the map lacks)
        if let Some(w) = self.world.clone() {
            let mut none = None;
            crate::app::report_missing_content(&w, &mut none);
        }
        // PluginFinalize, as OMSI calls it on the way out
        #[cfg(feature = "plugins")]
        if let Some(mut p) = self.plugins.take() {
            p.finalize();
        }
        if self.player.is_none() || self.career.seconds <= 0.0 {
            return;
        }
        // the situation to continue next time (OMSI writes it when a map is left; once, as
        // the rest: `career.seconds` is zero after the first time)
        self.save_last_situation();
        if self.career.path.is_some() {
            if let Err(e) = self.career.save() {
                log::warn!("writing the personnel file: {e}");
            }
        }
        let bus = self.args.bus.clone().unwrap_or_default();
        if let Err(e) = self.career.write_session(
            &self.args.map,
            &bus,
            self.args.line.as_deref(),
            self.args.tour.as_deref(),
        ) {
            log::warn!("writing the session summary: {e}");
        }
        self.career.seconds = 0.0;
    }

    /// Shift+N: the navigator, the navigator with the schedule, off. True in VR (where the
    /// key is used up).
    pub(crate) fn cycle_navigator(&mut self) -> bool {
        if self.vr_active() {
            if !self.vr_nav_profile().enabled {
                self.vr_nav_adjust("enabled", 1.0);
            } else if self.navigator.as_ref().is_some_and(|n| n.schedule) {
                if let Some(n) = self.navigator.as_mut() {
                    n.schedule = false;
                }
                self.vr_nav_adjust("enabled", 1.0);
            } else if let Some(n) = self.navigator.as_mut() {
                n.schedule = true;
            }
            return true;
        }
        if let Some(n) = self.navigator.as_mut() {
            match (n.enabled, n.schedule) {
                (true, false) => n.schedule = true,
                (true, true) => {
                    n.enabled = false;
                    n.schedule = false;
                }
                _ => n.enabled = true,
            }
        }
        false
    }

    /// The window's `CursorMoved`: only the cursor is taken; the switch under it is named
    /// once a frame (`RedrawRequested`). A gaming mouse sends 500-8000 moves a second, and a
    /// ray through every cockpit mesh for each of them kept the event queue from ever
    /// draining - no frame was drawn while the mouse moved.
    /// The indicator lever: 1 left, 2 right, 3 the hazard lights - each a toggle, as the
    /// Z / C / X keys and the phone's buttons work it.
    pub(crate) fn blinker(&mut self, want: u8) {
        if let Some(player) = self.player.as_mut() {
            player.toggle_indicator(want);
        }
    }

    /// Let go of every key the vehicle holds: the keyboard's driving keys and pedals, the
    /// vehicle keys of `Inputs/keyboard.cfg` (their `<trigger>_off` fires) and the
    /// Shift+number door buttons.
    pub(crate) fn release_vehicle_keys(&mut self) {
        if let Some(p) = self.player.as_mut() {
            let held: Vec<_> = p.held_keys.keys().copied().collect();
            for scan in held {
                p.key(scan, 0, false);
            }
            p.axes.release_all();
            for fired in self
                .door_key_triggers
                .drain()
                .map(|(_, g)| g)
                .collect::<Vec<_>>()
            {
                p.door_key_off(&fired);
            }
        }
        self.door_key_triggers.clear();
    }

    /// The window lost the keyboard and the mouse (focus gone to another window, minimised,
    /// hidden, a phone sending the app to the background): no key-up or button-up comes for
    /// what is held now, so all of it is let go here - the vehicle's keys, a switch held
    /// with the mouse, looking round, the mouse's zoom - and the mouse steering holds the
    /// wheel and the brake where they are with its throttle off. Until the window has the
    /// focus again the mouse and the keyboard work nothing (see `input_away`). A game
    /// controller's axes stay theirs.
    pub(crate) fn input_lost(&mut self) {
        self.input_away = true;
        self.release_vehicle_keys();
        self.keys.clear();
        // (the Alt bar's pending tap and keys: their releases will not come)
        self.new_ui.alt.reset_input();
        // (the "Add vehicle" window closes: a press it took has no release coming, and
        // the overlay would go on dragging what was under it)
        self.new_ui.close_add();
        // (the Alt bar's other windows stay, but forget a press and the keys whose releases
        // will not come)
        if self.new_ui.tool.window.is_some() {
            self.new_ui.tool.forget_input();
            self.new_ui.clear_win_input();
            self.new_ui.push_input(crate::new_ui_add::WinInput::Lost);
        }
        // (the report stays, but forgets a press whose release will not come)
        if let Some(r) = self.new_ui.report.as_mut() {
            r.forget_press();
            self.new_ui.clear_win_input();
            self.new_ui.push_input(crate::new_ui_add::WinInput::Lost);
        }
        if let Some(p) = self.player.as_mut() {
            p.release();
        }
        self.dragging = false;
        self.buttons_held = (false, false);
        self.both_drag = None;
        self.mouse_look = false;
        self.steer_cursor = None;
        self.mouse_pedals.0 = 0.0;
    }

    /// The window has the focus again: the mouse steering eases from where the wheel stands
    /// to the cursor (as when it is switched on) instead of jumping there. A key still held
    /// from before counts only once it is pressed again.
    pub(crate) fn input_back(&mut self) {
        if !self.input_away {
            return;
        }
        self.input_away = false;
        self.mouse_steer = (
            self.player
                .as_ref()
                .map(|p| p.vehicle.controls().steering)
                .unwrap_or(0.0),
            1.0,
        );
    }

    /// Put the vehicle file `bus` down beside the camera or the bus driven, in `paint` (a
    /// scheme's name; None: at random) with the depot file `hof` (None: the map's).
    /// The driven vehicle read again from its files and put where it stands (#728).
    pub(crate) fn reload_driven_vehicle(&mut self) {
        let Some(p) = self.player.as_ref() else {
            self.service_msg = Some(("There is no vehicle to reload: you are on foot".into(), 3.0));
            return;
        };
        // (the file under its content root, as the vehicle lists name it: a whole path was
        // taken for one under the game's folder)
        let file = &p.vehicle.ty.def.path;
        let bus = omsi_cfg::content_roots()
            .iter()
            .chain(std::iter::once(&self.args.root))
            .find_map(|r| file.strip_prefix(r).ok())
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/");
        let paint = p
            .vehicle
            .host
            .paint_scheme
            .flatten()
            .and_then(|i| p.vehicle.ty.paint_schemes.get(i))
            .map(|s| s.name.clone());
        // (the depot file by its file name, as `find_hof` looks for it)
        let hof = p.vehicle.host.hof.as_ref().and_then(|h| {
            h.path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .or_else(|| Some(h.name.clone()))
        });
        let before = p.uid;
        self.swap_pending = true;
        self.place_vehicle(&bus, paint, hof);
        if let Some(p) = self.player.as_ref().filter(|p| p.uid != before) {
            let name = format!(
                "{} {}",
                p.vehicle.ty.def.manufacturer, p.vehicle.ty.def.type_name
            );
            self.service_msg = Some((format!("Reloaded from its files: {}", name.trim()), 4.0));
        }
    }

    /// `q` (just spawned where the driven vehicle stands) becomes the one driven, and the
    /// one driven until now goes, with whoever rode in it (#728).
    pub(crate) fn replace_driven_vehicle(&mut self, q: Player) {
        let uid = q.uid;
        self.placed.insert(0, q);
        self.switch_vehicle();
        if !self.player.as_ref().is_some_and(|p| p.uid == uid) {
            return;
        }
        let Some(mut old) = self.placed.pop() else {
            return;
        };
        if let (Some(a), Some(mut ss)) = (self.audio.as_ref(), old.sounds.take()) {
            ss.stop_all(a);
        }
        if let (Some(w), Some(r), Some(scene)) = (
            self.world.clone(),
            self.renderer.as_ref(),
            self.scene.as_mut(),
        ) {
            if let Some(h) = self.humans.as_mut() {
                h.evict(
                    crate::humans::BusId::Ai(crate::humans::placed_bus_id(old.uid)),
                    &w,
                );
            }
            if let Some(mut d) = old.driver.take() {
                d.hide(r, scene);
            }
            w.release_vehicle(r, scene, old.render);
            for t in old.trailer_renders {
                w.release_vehicle(r, scene, t);
            }
        }
    }

    pub(crate) fn place_vehicle(&mut self, bus: &str, paint: Option<String>, hof: Option<String>) {
        // (in the driven vehicle's place, see `swap_pending`)
        let swap = std::mem::take(&mut self.swap_pending) && self.player.is_some();
        let name = self
            .vehicle_list
            .iter()
            .find(|v| v.1 == bus)
            .map(|v| v.0.clone())
            .unwrap_or_else(|| bus.to_string());
        // (a server's own buses only - its `vehicles` list, #1183 - whoever asks: the lists,
        // a plugin, the input script)
        if crate::lan::server_offers().is_some_and(|o| !crate::lan::offers(&o, bus)) {
            self.service_msg = Some((format!("The server does not offer {name}"), 4.0));
            return;
        }
        let bus = bus.to_string();
        let (Some(w), Some(r), Some(scene), Some(cam)) = (
            self.world.clone(),
            self.renderer.as_ref(),
            self.scene.as_mut(),
            self.camera.as_ref(),
        ) else {
            return;
        };
        let (x, y, heading) = match (self.view.as_str(), self.player.as_ref()) {
            (_, Some(p)) if swap => (
                p.vehicle.position.x,
                p.vehicle.position.y,
                p.vehicle.heading,
            ),
            ("free", _) | (_, None) => {
                let f = cam.forward();
                let flat = glam::DVec2::new(f.x as f64, f.y as f64).normalize_or_zero();
                let at = cam.position.truncate() + flat * 15.0;
                (at.x, at.y, cam.yaw as f64)
            }
            (_, Some(p)) => {
                let h = p.vehicle.heading.to_radians();
                let right = glam::DVec2::new(h.cos(), -h.sin());
                let at = p.vehicle.position.truncate() + right * 5.0;
                (at.x, at.y, p.vehicle.heading)
            }
        };
        let one = Args {
            bus: Some(bus.clone()),
            spawn: Some(format!("{x},{y},{heading}")),
            situation_vars: Vec::new(),
            situation_strvars: Vec::new(),
            situation_odometer_km: None,
            situation_script_textures: None,
            situation_others: Vec::new(),
            line: None,
            tour: None,
            trip: None,
            autostart: false,
            paint,
            hof: hof.or(self.args.hof.clone()),
            ..self.args.clone()
        };
        match spawn_player(&one, &w, r, scene) {
            Ok(Some(q)) if swap => {
                log::info!("{bus} takes the driven vehicle's place at ({x:.1}, {y:.1})");
                self.replace_driven_vehicle(q);
            }
            Ok(Some(q)) => {
                log::info!("placed {bus} at ({x:.1}, {y:.1})");
                let uid = q.uid;
                self.placed.push(q);
                // (then put down with the mouse, where the player wants it)
                self.begin_placing(uid, heading);
                let _ = name;
            }
            Ok(None) => {}
            Err(e) => self.service_msg = Some((format!("Could not place {name}: {e:#}"), 5.0)),
        }
    }

    /// Couple a standing vehicle to the back of the one driven: its front coupling within
    /// 2.5 m of the train's rear coupling, facing the same way.
    pub(crate) fn couple(&mut self) {
        let (Some(w), Some(r), Some(scene), Some(p)) = (
            self.world.clone(),
            self.renderer.as_ref(),
            self.scene.as_mut(),
            self.player.as_mut(),
        ) else {
            return;
        };
        // the train's rear coupling in the world
        let rear = match p.vehicle.trailers.last() {
            Some(t) => {
                let c = if t.reversed {
                    t.ty.def.coupling_front.as_ref()
                } else {
                    t.ty.def.coupling_back.as_ref()
                };
                c.map(|c| {
                    (
                        t.world_transform()
                            .transform_point3(glam::Vec3::from(c.pos)),
                        t.heading,
                    )
                })
            }
            None => p.vehicle.ty.def.coupling_back.as_ref().map(|c| {
                (
                    p.vehicle
                        .world_transform()
                        .transform_point3(glam::Vec3::from(c.pos)),
                    p.vehicle.heading,
                )
            }),
        };
        let Some((rear, rear_heading)) = rear else {
            self.service_msg = Some(("This vehicle has no coupling at its back".into(), 4.0));
            return;
        };
        let found = self.placed.iter().position(|q| {
            let Some(c) = q.vehicle.ty.def.coupling_front.as_ref() else {
                return false;
            };
            let front = q
                .vehicle
                .world_transform()
                .transform_point3(glam::Vec3::from(c.pos));
            let dh = wrap_deg(q.vehicle.heading - rear_heading).abs();
            (front - rear).truncate().length() < 2.5 && dh < 35.0
        });
        let Some(k) = found else {
            self.service_msg = Some((
                "Nothing to couple: back up to a trailer's coupling (within 2.5 m, in line)".into(),
                4.0,
            ));
            return;
        };
        let q = self.placed.remove(k);
        let ty = q.vehicle.ty.clone();
        if let (Some(a), Some(mut ss)) = (self.audio.as_ref(), q.sounds) {
            ss.stop_all(a);
        }
        w.release_vehicle(r, scene, q.render);
        for tr in q.trailer_renders {
            w.release_vehicle(r, scene, tr);
        }
        p.vehicle.attach_trailer_ex(ty.clone(), false);
        p.trailer_renders
            .push(w.add_vehicle_part(r, scene, &ty, None, &p.render));
        p.hand_coupled += 1;
        self.service_msg = Some((
            omsi_ui::tr("Coupled: %{bus}").replace(
                "%{bus}",
                &format!("{} {}", ty.def.manufacturer, ty.def.type_name),
            ),
            3.0,
        ));
    }

    /// Uncouple the last part coupled by hand: it stays where it is, a vehicle of its own.
    pub(crate) fn uncouple(&mut self) {
        let (Some(w), Some(r), Some(scene)) = (
            self.world.clone(),
            self.renderer.as_ref(),
            self.scene.as_mut(),
        ) else {
            return;
        };
        let Some(p) = self.player.as_mut() else {
            return;
        };
        if p.hand_coupled == 0 {
            self.service_msg = Some((
                "Nothing coupled by hand (an articulated bus's rear section stays)".into(),
                4.0,
            ));
            return;
        }
        let Some(t) = p.vehicle.detach_last_trailer() else {
            return;
        };
        if let Some(tr) = p.trailer_renders.pop() {
            w.release_vehicle(r, scene, tr);
        }
        p.hand_coupled -= 1;
        let one = Args {
            bus: Some(t.ty.def.path.to_string_lossy().to_string()),
            spawn: Some(format!(
                "{},{},{},{}",
                t.position.x, t.position.y, t.heading, t.position.z
            )),
            situation_vars: Vec::new(),
            situation_strvars: Vec::new(),
            situation_odometer_km: None,
            situation_script_textures: None,
            situation_others: Vec::new(),
            line: None,
            tour: None,
            trip: None,
            autostart: false,
            paint: None,
            ..self.args.clone()
        };
        match spawn_player(&one, &w, r, scene) {
            Ok(Some(q)) => {
                self.placed.push(q);
                self.service_msg = Some(("Uncoupled".into(), 3.0));
            }
            Ok(None) => {}
            Err(e) => log::warn!("uncoupled part: {e:#}"),
        }
    }

    /// Do what line `k` of the game menu says.
    pub(crate) fn menu_choose(&mut self, event_loop: &ActiveEventLoop, k: usize) {
        // (a click in an open list keeps the scroll where it is: no jump to the line)
        self.menu_top = if self.chooser.is_some() {
            self.ui.as_ref().map(|u| u.menu_start as f32)
        } else {
            None
        };
        if self.chooser.is_some() {
            self.chooser_pick(k);
            return;
        }
        // (a greyed-out line does nothing)
        if self.menu_item_off(k) {
            return;
        }
        let Some(id) = self.game_menu_items().get(k).map(|m| m.0) else {
            return;
        };
        match id {
            "resume" => self.close_game_menu(),
            "options" => self.open_list(crate::game_lists::ListKind::Options(0)),
            "controls" => self.open_list(crate::game_lists::ListKind::Controls),
            "camera" => {
                let tab = crate::game_lists::options_tab(self, "Camera");
                self.open_list(crate::game_lists::ListKind::Options(tab));
            }
            "vehicle" => self.open_list(crate::game_lists::ListKind::Vehicle(0)),
            "world" => self.open_list(crate::game_lists::ListKind::World(0)),
            "copycode" => {
                self.close_game_menu();
                self.copy_server_code();
            }
            "admin" => self.open_list(crate::game_lists::ListKind::Admin),
            "duty" => self.open_list(crate::game_lists::ListKind::Lines),
            "map" => {
                self.close_game_menu();
                if let Some(n) = self.navigator.as_mut() {
                    if !n.map_open() {
                        n.toggle_map();
                    }
                }
            }
            "save" => {
                self.quick_save();
                self.close_game_menu();
            }
            "saveslot" => {
                self.save_slot();
                self.close_game_menu();
            }
            "shot" => {
                self.close_game_menu();
                self.take_screenshot();
            }
            "skipstop" => {
                self.close_game_menu();
                self.skip_next_stop();
            }
            // the route ends here: free drive, as the list of lines has it
            "endduty" => {
                crate::game_lists::end_duty(self);
                self.close_game_menu();
            }
            "tobus" => {
                self.close_game_menu();
                self.back_to_bus();
            }
            "load" => {
                self.game_menu = None;
                if self.load_quicksave() {
                    self.finish_session();
                    crate::platform::exit(event_loop);
                }
            }
            "quit" => {
                self.game_menu = None;
                self.finish_session();
                crate::platform::exit(event_loop);
            }
            // (the rest are the lines of the vehicle and world pages)
            other => {
                self.page_action(other);
            }
        }
    }

    /// The duty gives up the stop it is due at and goes on with the one after it (the game
    /// menu's "Skip the next stop", Ctrl+Shift+H): the IBIS moves on with it, as it does
    /// when a bus page sets the next stop.
    pub(crate) fn skip_next_stop(&mut self) {
        let Some(d) = self.duty.as_mut() else { return };
        let Some(name) = d.skip_next() else {
            self.service_msg = Some(("The trip is over: no stop to skip".into(), 3.0));
            return;
        };
        log::info!("duty: stop '{name}' skipped, next stop {}", d.next_stop);
        if let Some(p) = self.player.as_mut() {
            let (trip, k) = d.trip_for_ibis();
            p.ibis_to_stop(trip, k);
        }
        self.service_msg = Some((format!("Stop skipped: {name}"), 3.0));
    }

    /// The timetable window's "Skip this stop" (or "Keep this stop") on stop `k` of the
    /// current trip: the stop due now is skipped at once, one still to come is marked to be
    /// skipped as soon as it is the next stop, or unmarked (see `PlayerDuty::toggle_skip`).
    pub(crate) fn skip_stop_at(&mut self, k: usize) {
        let Some(d) = self.duty.as_mut() else { return };
        let Some((name, now)) = d.toggle_skip(k) else {
            return;
        };
        if now {
            log::info!("duty: stop '{name}' skipped, next stop {}", d.next_stop);
            if let Some(p) = self.player.as_mut() {
                let (trip, k) = d.trip_for_ibis();
                p.ibis_to_stop(trip, k);
            }
            self.service_msg = Some((format!("Stop skipped: {name}"), 3.0));
        } else if d.skip_marked(k) {
            log::info!("duty: stop '{name}' to be skipped");
            self.service_msg = Some((format!("Will be skipped: {name}"), 3.0));
        } else {
            self.service_msg = Some((format!("Stop kept: {name}"), 3.0));
        }
    }

    /// Read the vehicles that can be placed, once (the place list and the "Add vehicle" window).
    pub(crate) fn load_vehicle_list(&mut self) {
        if self.vehicle_list.is_empty() {
            let menu = crate::menu::Menu::new(&self.args.root, &self.args.map);
            self.vehicle_meta = menu
                .vehicles
                .iter()
                .zip(menu.vehicle_meta)
                .map(|(v, meta)| (v.1.clone(), meta))
                .collect();
            self.vehicle_list = menu.vehicles;
            // (alphabetical)
            self.vehicle_list.sort_by_key(|v| v.0.to_lowercase());
            crate::mt::protect(self.vehicle_list.iter().map(|v| v.0.as_str()));
        }
    }

    /// The actions of the vehicle and world pages (and of what the plugins and the input
    /// script ask of the menu by name). False when `id` is none of them.
    pub(crate) fn page_action(&mut self, id: &str) -> bool {
        match id {
            "swap" | "place" => {
                // (a plain "Place a vehicle" puts one beside; "Swap" in the driven one's place)
                self.swap_pending = id == "swap" && self.player.is_some();
                self.load_vehicle_list();
                if self.vehicle_list.is_empty() {
                    self.service_msg = Some(("No vehicles found".into(), 3.0));
                } else if crate::lan::server_offers().is_some_and(|o| {
                    !self
                        .vehicle_list
                        .iter()
                        .any(|v| crate::lan::offers(&o, &v.1))
                }) {
                    self.service_msg = Some((
                        "The server offers none of the vehicles installed here".into(),
                        4.0,
                    ));
                } else {
                    // (as the launcher's bus step: the manufacturer, then the type)
                    self.open_list(crate::game_lists::ListKind::PlaceMaker);
                }
            }
            "couple" => {
                self.close_game_menu();
                self.couple();
            }
            "uncouple" => {
                self.close_game_menu();
                self.uncouple();
            }
            "tobus" => {
                self.close_game_menu();
                self.back_to_bus();
            }
            "remove" => {
                self.close_game_menu();
                self.remove_driven_vehicle();
            }
            "reload" => {
                self.close_game_menu();
                self.reload_driven_vehicle();
            }
            "clearplaced" => {
                self.close_game_menu();
                self.remove_placed_vehicles();
            }
            "getout" => {
                self.close_game_menu();
                self.get_up();
            }
            "reset" => {
                self.close_game_menu();
                if let Some(p) = self.player.as_ref() {
                    let (at, heading) = (p.vehicle.position, p.vehicle.heading);
                    crate::teleport::teleport(self, at, heading);
                    self.service_msg = Some(("The vehicle stands on its wheels again".into(), 3.0));
                }
            }
            "teleport" => {
                self.close_game_menu();
                if let Some(n) = self.navigator.as_mut() {
                    if !n.map_open() {
                        n.toggle_map();
                    }
                    self.teleport_pick = true;
                    self.service_msg = Some((
                        "Click a street on the map: the bus is put there".into(),
                        6.0,
                    ));
                }
            }
            "driver" => self.open_list(crate::game_lists::ListKind::Drivers),
            "number" => self.open_list(crate::game_lists::ListKind::Numbers),
            "dest" => self.open_list(crate::game_lists::ListKind::Destinations),
            "hof" => self.open_list(crate::game_lists::ListKind::Hofs),
            "tplist" => self.open_list(crate::game_lists::ListKind::Spots),
            "editor" => {
                self.close_game_menu();
                self.toggle_editor();
            }
            "timetable" => {
                self.timetable = !self.timetable;
                self.close_game_menu();
            }
            "info" => {
                self.set_info_bar(!self.info_bar);
                self.close_game_menu();
            }
            "refuel" | "wash" | "repair" => {
                self.close_game_menu();
                self.run_service(id);
            }
            "weather" => {
                self.close_game_menu();
                self.next_weather();
            }
            "metar_once" => self.load_metar_once(),
            "metar_refresh" => self.refresh_metar_now(),
            "weather_custom" => self.current_weather_as_custom(),
            "switch" => {
                self.close_game_menu();
                self.switch_vehicle();
            }
            "later" | "earlier" | "later10" | "earlier10" => {
                self.close_game_menu();
                if self.is_lan_client() {
                    self.service_msg =
                        Some(("In a LAN session the host sets the clock".into(), 3.0));
                } else {
                    self.shift_clock(match id {
                        "later" => 3600.0,
                        "earlier" => -3600.0,
                        "later10" => 600.0,
                        _ => -600.0,
                    });
                }
            }
            _ => return false,
        }
        true
    }

    /// Drive another of the vehicles standing in the world (a situation's): the one driven
    /// now stays where it is with everything as it was, and its sounds go to the next one.
    pub(crate) fn switch_vehicle(&mut self) {
        if self.placed.is_empty() {
            self.service_msg = Some(("There is no other vehicle to drive".into(), 3.0));
            return;
        }
        let Some(mut now) = self.player.take() else {
            // on foot without a bus: the first placed one's wheel
            self.take_placed(0);
            return;
        };
        if let (Some(a), Some(mut ss)) = (self.audio.as_ref(), now.sounds.take()) {
            ss.stop_all(a);
        }
        let mut next = self.placed.remove(0);
        if let Some(a) = self.audio.as_ref() {
            next.load_sounds(a);
        }
        // the riders stay in the bus left; the people know the new one's cabin
        if let Some(h) = self.humans.as_mut() {
            h.player_bus_swapped(now.uid, next.uid, &mut next.vehicle);
        }
        next.vehicle.host.auto_clutch = if self.settings.auto_clutch { 1.0 } else { 0.0 };
        self.placed.push(now);
        let name = format!(
            "{} {}",
            next.vehicle.ty.def.manufacturer, next.vehicle.ty.def.type_name
        );
        if let Some(cam) = self.camera.as_ref() {
            self.camera = Some(next.camera(&self.view, cam));
        }
        self.player = Some(next);
        self.look = (0.0, 0.0);
        self.service_msg = Some((format!("Now driving: {}", name.trim()), 4.0));
    }

    /// Start the game again on the quicksave (`Situations/quicksave.osn` of the content
    /// folder): a fresh process, as OMSI loads a situation into a fresh world. False when
    /// there is none.
    pub(crate) fn load_quicksave(&mut self) -> bool {
        let dir = crate::startup::content_dir()
            .unwrap_or_else(|| self.args.root.clone())
            .join("Situations");
        let mut file = dir.join("quicksave.osn");
        // (the newer of the content folder's and the fallback folder's, see `save_or_fallback`)
        if let Some(f) =
            crate::startup::save_fallback_dir().map(|f| f.join("Situations").join("quicksave.osn"))
        {
            let age = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
            if f.exists() && (!file.exists() || age(&f) > age(&file)) {
                file = f;
            }
        }
        if !file.exists() {
            self.service_msg = Some(("No quicksave yet (Ctrl+S saves one)".into(), 4.0));
            return false;
        }
        let Ok(exe) = std::env::current_exe() else {
            return false;
        };
        let mut cmd = std::process::Command::new(exe);
        cmd.arg("--root")
            .arg(&self.args.root)
            .arg("--no-menu")
            .arg("--situation")
            .arg(&file);
        // (the duty typed by itself goes on being typed: `--autostart` with a situation)
        if self.player.as_ref().is_some_and(|p| p.duty_typed) {
            cmd.arg("--autostart");
        }
        match cmd.spawn() {
            Ok(_) => {
                log::info!("loading {} in a new game", file.display());
                true
            }
            Err(e) => {
                self.service_msg = Some((format!("Could not start the game again: {e}"), 5.0));
                false
            }
        }
    }

    /// The running fuel pump or bus wash, one frame of it: its trigger with the frame's
    /// time; done once the tank (the dirt) has not changed for `SERVICE_SETTLE` seconds, and
    /// stopped by driving off.
    pub(crate) fn tick_service(&mut self, dt: f32) {
        const SERVICE_SETTLE: f32 = 1.5;
        let Some((kind, idle)) = self.pumping else {
            return;
        };
        let Some(p) = self.player.as_mut() else {
            self.pumping = None;
            return;
        };
        let (name, var) = if kind == "refuel" {
            ("veh_tank", "engine_tank_content")
        } else {
            ("veh_wash", "Dirt_Wiped")
        };
        if p.vehicle.motion().velocity_kmh().abs() > 2.0 {
            self.pumping = None;
            self.service_msg = Some((
                if kind == "refuel" {
                    "Refuelling stopped"
                } else {
                    "Washing stopped"
                }
                .into(),
                4.0,
            ));
            return;
        }
        let before = p.vehicle.var(var).unwrap_or(0.0);
        p.vehicle.service(name, dt);
        let now = p.vehicle.var(var).unwrap_or(0.0);
        let idle = if (now - before).abs() > 1e-4 {
            0.0
        } else {
            idle + dt
        };
        if idle > SERVICE_SETTLE {
            self.pumping = None;
            let (text, value) = if kind == "refuel" {
                ("refuelled: %{litres} l in the tank", format!("{now:.0}"))
            } else {
                ("washed: dirt %{dirt}%", format!("{:.0}", now * 100.0))
            };
            let fill = |t: &str| t.replace("%{litres}", &value).replace("%{dirt}", &value);
            log::info!("{}", fill(text));
            self.service_msg = Some((fill(&omsi_ui::tr(text)), 6.0));
        } else {
            self.pumping = Some((kind, idle));
            if kind == "refuel" {
                self.service_msg = Some((
                    omsi_ui::tr("Refuelling: %{litres} l")
                        .replace("%{litres}", &format!("{now:.0}")),
                    1.0,
                ));
            }
        }
    }

    /// One of the depot services of the game menu: "refuel", "wash" or "repair".
    pub(crate) fn run_service(&mut self, kind: &str) {
        let Some(w) = self.world.clone() else { return };
        let Some(p) = self.player.as_mut() else {
            return;
        };
        // The pump and the wash run as OMSI's do, `veh_tank` / `veh_wash` every frame
        // while they are on, the tank filling litre by litre (`tick_service`); it had been
        // full the moment the menu was clicked (#1785). (Off a station: the message below.)
        if matches!(kind, "refuel" | "wash") && at_petrol_station(&w, &p.vehicle) {
            let name = if kind == "refuel" {
                "veh_tank"
            } else {
                "veh_wash"
            };
            if !p.vehicle.service(name, 0.0) {
                self.service_msg = Some((
                    if kind == "refuel" {
                        format!("this vehicle has no fuel pump handling ({name})")
                    } else {
                        format!("this vehicle has no bus wash handling ({name})")
                    },
                    6.0,
                ));
                return;
            }
            if kind == "wash" {
                p.vehicle.dirt = 0.0;
                p.vehicle.set_engine_var("Dirt_Norm", 0.0);
            }
            self.pumping = Some((if kind == "refuel" { "refuel" } else { "wash" }, 0.0));
            self.service_msg = Some((
                if kind == "refuel" {
                    "Refuelling... (drive off to stop)"
                } else {
                    "Washing..."
                }
                .into(),
                4.0,
            ));
            return;
        }
        let one = Args {
            refuel: kind == "refuel",
            wash: kind == "wash",
            repair: kind == "repair",
            ..self.args.clone()
        };
        let at_station = at_petrol_station(&w, &p.vehicle);
        let mut clock = self.clock.clone();
        let msg = run_services(
            &one,
            &mut p.vehicle,
            &mut clock,
            w.global.repair_time_min,
            at_station,
        );
        while clock.time >= 86400.0 {
            clock.time -= 86400.0;
            clock.day_of_year = clock.day_of_year % omsi_sim::clock::days_in_year(clock.year) + 1;
        }
        #[cfg(feature = "lan")]
        let client = self
            .online
            .lan
            .as_ref()
            .is_some_and(|l| l.role == omsi_net::Role::Client);
        #[cfg(not(feature = "lan"))]
        let client = false;
        if !self.settings.time_sync || client {
            self.clock = clock;
        }
        p.vehicle.host.clock = self.clock.clone();
        for line in &msg {
            log::info!("{line}");
        }
        if let Some(line) = msg.into_iter().next() {
            self.service_msg = Some((line, 6.0));
        }
    }

    #[cfg(windows)]
    pub(crate) fn vr_action(&mut self, name: &str) -> bool {
        if name == "vr_toggle_mode" {
            self.vr_zoom_active = false;
            if !self.settings.vr_requested() {
                return false;
            }
            if self.vr.is_some() {
                self.vr = None;
                self.service_msg = Some(("Desktop mode".into(), 2.0));
            } else if let Some(renderer) = self.renderer.as_ref() {
                match crate::openxr::Vr::new(
                    renderer,
                    self.settings.vr_scale,
                    self.settings.vr_desktop_mirror,
                ) {
                    Ok(vr) => {
                        self.vr = Some(vr);
                        self.service_msg = Some(("VR mode".into(), 2.0));
                    }
                    Err(e) => {
                        log::error!("OpenXR could not restart: {e:#}");
                        self.service_msg =
                            Some((format!("{}: {e}", omsi_ui::tr("Could not start VR")), 5.0));
                    }
                }
            }
            self.hover_key = None;
            if matches!(
                self.list_kind,
                Some(crate::game_lists::ListKind::Options(_))
            ) {
                self.refresh_list();
            }
            return true;
        }
        if self.vr.is_none() {
            return false;
        }
        match name {
            "vr_recenter" => {
                self.vr.as_mut().unwrap().recenter();
                self.look = (0.0, 0.0);
                self.service_msg = Some(("VR view recentered".into(), 2.0));
            }
            "vr_toggle_desktop_mirror" => {
                let visible = self.vr.as_mut().unwrap().toggle_desktop_mirror();
                self.settings.vr_desktop_mirror = visible;
                self.service_msg = Some((
                    if visible {
                        "Desktop VR mirror on"
                    } else {
                        "Desktop VR mirror off"
                    }
                    .into(),
                    2.0,
                ));
            }
            "vr_toggle_navigator" => self.vr_nav_adjust("enabled", 1.0),
            "vr_position_navigator" => self.start_vr_nav_edit(),
            _ => return false,
        }
        true
    }

    /// One of OMSI's global key actions; false when it is not one this game does.
    pub(crate) fn game_action(&mut self, name: &str) -> bool {
        #[cfg(windows)]
        if self.vr_action(name) {
            return true;
        }
        match name {
            "sim_pause" => self.toggle_pause(),
            "screenshot" => self.take_screenshot(),
            "quicksave" => self.quick_save(),
            "view_set_ego" => {
                // on foot from where the camera is (beside the bus in the driver's view)
                if let (Some(cam), Some(p)) = (self.camera.as_mut(), self.player.as_ref()) {
                    if self.view != "free" {
                        let h = (p.vehicle.heading as f32 - 90.0).to_radians();
                        cam.position = p.vehicle.position
                            + glam::DVec3::new(h.sin() as f64, h.cos() as f64, 0.0) * 2.5;
                        cam.yaw = p.vehicle.heading as f32;
                        cam.pitch = 0.0;
                    }
                }
                self.view = "free".into();
                self.ego = true;
                self.service_msg = Some(("On foot: W A S D walk, Shift runs, right mouse button looks (F1 back to the bus)".into(), 5.0));
            }
            "view_set_driver" => self.view = "driver".into(),
            "view_set_passenger" => self.view = "pax".into(),
            "view_set_outside" => self.view = "outside".into(),
            // the cabin (the driver's or the passenger's) and the outside, one press apart:
            // what a single button on a controller wants. `view_toggle_viewpoint` is the
            // four-mode cycle, with the map in it, and stays where it is.
            "view_toggle_interior" => {
                if !self.ego {
                    self.view = if self.view == "outside" {
                        "driver".into()
                    } else {
                        "outside".into()
                    };
                }
            }
            "view_set_map" => {
                // OMSI's map view (F4) is a camera flown over the map; the city map of the
                // navigator stays on Shift+M
                if self.view != "free" {
                    if let (Some(cam), Some(p)) = (self.camera.as_mut(), self.player.as_ref()) {
                        let h = (p.vehicle.heading as f32).to_radians();
                        cam.position = p.vehicle.position
                            + glam::DVec3::new(
                                -(h.sin() as f64) * 25.0,
                                -(h.cos() as f64) * 25.0,
                                30.0,
                            );
                        cam.yaw = p.vehicle.heading as f32;
                        cam.pitch = -45.0;
                    }
                }
                self.view = "free".into();
                self.ego = false;
            }
            // the timetable and the ticket desk each have a camera of their own in the bus
            // (`[view_schedule]`, `[view_ticketselling]`): the key switches the driver's view
            // to it and back
            "view_set_schedule" | "view_set_ticketselling" => {
                let schedule = name == "view_set_schedule";
                if schedule {
                    self.timetable = !self.timetable;
                }
                if let Some(p) = self.player.as_mut() {
                    let def = &p.vehicle.ty.def;
                    let cam = if schedule {
                        def.view_schedule
                    } else {
                        def.view_ticketselling
                    };
                    let n = def.cameras_driver.len().max(1);
                    // (the choice counts from the standard camera)
                    if let Some(c) = cam
                        .filter(|c| *c < def.cameras_driver.len())
                        .map(|c| (c + n - def.camera_std % n) % n)
                    {
                        let back = p.cam_before_special.take();
                        if self.view == "driver" && p.cam_choice.0 == c {
                            p.cam_choice.0 = back.unwrap_or(0);
                        } else {
                            p.cam_before_special = Some(p.cam_choice.0);
                            p.cam_choice.0 = c;
                            self.view = "driver".into();
                        }
                        self.sync_view_look();
                    } else if !schedule {
                        self.service_msg = Some(("This bus has no ticket desk camera".into(), 3.0));
                    }
                }
            }
            "view_toggle_informationdisplay" => self.set_info_bar(!self.info_bar),
            // (OMSI's camera reset puts back the field of view with the
            // direction: the zoom goes as well, #244)
            "view_reset_direction" => {
                // F1 eases home (look + zoom glide) from the values in place:
                // zeroing them first would flash a frame of the destination.
                if self.view == "driver"
                    && self.settings.driverview_smooth
                    && (self.look != (0.0, 0.0) || self.view_zoom.contains_key(&self.view))
                {
                    let zoom = self.view_zoom.get(&self.view).copied().unwrap_or(1.0);
                    let key = self.look_key();
                    self.f1_reset = Some((self.look, zoom, 0.0, key));
                } else {
                    self.f1_reset = None;
                    self.look = (0.0, 0.0);
                    self.view_zoom.remove(&self.view);
                }
                #[cfg(windows)]
                if let Some(vr) = self.vr.as_mut() {
                    vr.recenter();
                }
            }
            // (Space in Inputs/keyboard.cfg: every view looks ahead again, and back to the
            // standard camera - "center")
            "view_reset_all_directions" => {
                // F1 eases home (look + zoom glide) from the values in place:
                // zeroing them first would flash a frame of the destination.
                // Everything else snaps. The glide belongs to the standard
                // camera (cam reset first), so a mid-glide switch finalizes it.
                let zoom = self.view_zoom.get(&self.view).copied().unwrap_or(1.0);
                let eyed = self.view == "driver"
                    && self.settings.driverview_smooth
                    && (self.look != (0.0, 0.0) || self.view_zoom.contains_key(&self.view));
                if let Some(p) = self.player.as_mut() {
                    p.cam_choice = (0, 0);
                }
                self.orbit = ORBIT_DEFAULT;
                if eyed {
                    self.view_looks.clear();
                    self.view_zoom.retain(|k, _| k == "driver");
                    let key = self.look_key();
                    self.f1_reset = Some((self.look, zoom, 0.0, key));
                    // the bookkeeping follows the camera change at once: left
                    // stale, the next swap would write the old look straight
                    // back into the previous camera's slot.
                    self.look_view = self.look_key();
                } else {
                    self.f1_reset = None;
                    self.look = (0.0, 0.0);
                    self.view_looks.clear();
                    self.view_zoom.clear();
                }
            }
            // the next (or the previous) view mode, driver - passenger - outside - map and
            // round again; nothing on foot (OMSI steps with (mode + 1) and 3,
            // the inverse likewise)
            "view_toggle_viewpoint" | "view_toggle_viewpoint_inverse" => {
                if self.ego {
                    return true;
                }
                let mode = match self.view.as_str() {
                    "driver" => 0,
                    "pax" => 1,
                    "outside" => 2,
                    _ => 3,
                };
                let next = if name == "view_toggle_viewpoint" {
                    (mode + 1) % 4
                } else {
                    (mode + 3) % 4
                };
                return self.game_action(
                    [
                        "view_set_driver",
                        "view_set_passenger",
                        "view_set_outside",
                        "view_set_map",
                    ][next],
                );
            }
            // a manual gearbox (Ctrl+Up / Ctrl+Down unless moved; a controller's button)
            "gear_up" | "gear_down" => {
                self.shift_gear(name == "gear_up");
            }
            "view_interiorcam_plus" | "view_interiorcam_minus" => {
                let Some(p) = self.player.as_mut() else {
                    return true;
                };
                // (the interior cameras only cycle in the interior: from outside the keys
                // would change an invisible camera)
                if !matches!(self.view.as_str(), "driver" | "pax") {
                    return true;
                }
                let (count, pax) = if self.view == "pax" {
                    (p.pax_camera_count(), true)
                } else {
                    (p.driver_camera_count(), false)
                };
                if count > 1 {
                    let c = if pax {
                        &mut p.cam_choice.1
                    } else {
                        &mut p.cam_choice.0
                    };
                    *c = if name == "view_interiorcam_minus" {
                        (*c + count - 1) % count
                    } else {
                        (*c + 1) % count
                    };
                    let n = *c + 1;
                    // (the camera left keeps its look, the one taken finds its own again)
                    self.sync_view_look();
                    self.service_msg = Some((
                        if pax {
                            format!("Passenger camera {n} of {count}")
                        } else {
                            format!("Driver camera {n} of {count}")
                        },
                        2.0,
                    ));
                }
            }
            "toggel_mouse_ctrl" => {
                self.set_mouse_drive(!self.mouse_drive);
                let msg = if self.mouse_drive {
                    "Mouse steering on: across steers, up is the throttle, down the brake (O turns it off)"
                } else {
                    "Mouse steering off"
                };
                self.service_msg = Some((msg.into(), 4.0));
            }
            "toggel_ctrler" => {
                if let Some(c) = self.controllers.as_mut() {
                    c.enabled = !c.enabled;
                    let msg = if !c.any() {
                        "No game controller found"
                    } else if c.enabled {
                        "Game controller on"
                    } else {
                        "Game controller off"
                    };
                    self.service_msg = Some((msg.into(), 3.0));
                }
            }
            _ => return false,
        }
        true
    }

    pub(crate) fn shift_gear(&mut self, up: bool) -> bool {
        let names: &[&str] = if up {
            &[
                "kw_s_plus",
                "upshift",
                "gear_up",
                "gearup",
                "shift_up",
                "gang_hoch",
                "schalten_hoch",
                "manual_up",
            ]
        } else {
            &[
                "kw_s_minus",
                "downshift",
                "gear_down",
                "geardown",
                "shift_down",
                "gang_runter",
                "schalten_runter",
                "manual_down",
            ]
        };
        let Some(p) = self.player.as_mut() else {
            return false;
        };
        // a gear lever with a trigger per gate (`kw_s_1`..`kw_s_10`, `kw_s_N`, `kw_s_R`: the
        // LiAZ MKPP - its `kw_s_plus` never fires, the script's condition is broken): the
        // next gate from the gear engaged, with the clutch down as the gates want it
        if p.vehicle.ty.program.trigger("kw_s_1").is_some() {
            let Some(cur) = p.gate_gear() else {
                return false;
            };
            let to = if up { cur + 1 } else { cur - 1 };
            if !p.shift_gate_to(to) {
                return false;
            }
            self.service_msg = Some((
                omsi_ui::tr("Gear %{gear}").replace(
                    "%{gear}",
                    &match to {
                        0 => "N".to_string(),
                        -1 => "R".to_string(),
                        n => n.to_string(),
                    },
                ),
                1.5,
            ));
            return true;
        }
        let Some(n) = names
            .iter()
            .find(|n| p.vehicle.ty.program.trigger(n).is_some())
        else {
            self.service_msg = Some(("This vehicle has no manual gearbox to shift".into(), 2.0));
            return false;
        };
        p.vehicle.trigger(n);
        p.vehicle.trigger(&format!("{n}_off"));
        true
    }

    /// The information bar on or off, and kept so for the next session (#1164).
    pub(crate) fn set_info_bar(&mut self, on: bool) {
        self.info_bar = on;
        if self.settings.info_bar != on {
            self.settings.info_bar = on;
            crate::game_lists::remember_setting("info_bar", if on { "1" } else { "0" });
        }
    }

    pub(crate) fn toggle_pause(&mut self) {
        // (a LAN session goes on for the others: it cannot be paused)
        if self.in_lan_session() {
            self.service_msg = Some(("A LAN session cannot be paused".into(), 3.0));
            return;
        }
        self.paused = !self.paused;
        if self.game_menu.is_some() {
            // Keep the state a menu close should restore in step with P.
            self.menu_prev_pause = self.paused;
        }
    }

    /// Put the bus on the street nearest the world point `at` (the city map's Ctrl+click),
    /// facing along it.
    pub(crate) fn place_bus_at(&mut self, at: glam::DVec2) {
        if self.is_lan_client() {
            self.service_msg = Some((
                "In a LAN session only the host moves vehicles on the map".into(),
                4.0,
            ));
            return;
        }
        let p = glam::DVec3::new(at.x, at.y, 0.0);
        // the traffic's lanes (the tiles loaded around the bus), else the navigator's of the
        // whole map: a street far off on a big map was "no street" until the bus had been
        // flown there (#235). (the height of the point does not matter: the nearest by the
        // ground plan)
        let nets = [
            self.traffic.as_ref().map(|t| &t.net),
            self.navigator.as_ref().and_then(|n| n.map_net()),
        ];
        let Some((net, (lane, s, _))) = nets.into_iter().flatten().find_map(|net| {
            net.nearest_lane(p, omsi_sim::traffic::LaneKind::Street)
                .filter(|(_, _, d)| *d <= 300.0)
                .map(|f| (net, f))
        }) else {
            self.service_msg = Some(("No street near that point".into(), 3.0));
            return;
        };
        let l = &net.lanes[lane];
        let (pos, heading) = l.at(s);
        let heading = heading as f64;
        crate::teleport::teleport(self, pos, heading);
        self.service_msg = Some(("The bus stands where the map was clicked".into(), 3.0));
    }

    /// The world follows the sim date as OMSI's does at the day's change:
    /// the chrono scenarios in force (the tiles they change are read again) and the
    /// season's textures - also when the weather turns to snow or thaws (every loaded tile
    /// is read again with the other texture folder).
    pub(crate) fn follow_date(&mut self) {
        let Some(w) = self.world.clone() else { return };
        let date = self.clock.date_code();
        let snow = self.weather.as_ref().is_some_and(|x| x.snow);
        let on_road = self.weather.as_ref().is_some_and(|x| x.snow_on_road);
        let season = crate::world_load::season_folder_on(
            &self.args,
            &w.global,
            self.clock.day_of_year,
            snow,
            on_road,
        )
        .1;
        let Some((was_date, was_season)) = self.world_day.clone() else {
            self.world_day = Some((date, omsi_texture::season_folder()));
            return;
        };
        if was_date == date && was_season == season {
            return;
        }
        self.world_day = Some((date, season.clone()));
        let changed = if was_date != date {
            w.set_date(date)
        } else {
            Vec::new()
        };
        let (Some(st), Some(r), Some(scene)) = (
            self.streamer.as_mut(),
            self.renderer.as_ref(),
            self.scene.as_mut(),
        ) else {
            return;
        };
        if was_season != season {
            log::info!(
                "season: the textures of {:?} now (were {:?})",
                season,
                was_season
            );
            omsi_texture::set_season_folder(season);
            omsi_cfg::content_changed();
            st.reload(r, scene, None, self.audio.as_ref());
        } else if !changed.is_empty() {
            st.reload(r, scene, Some(&changed), self.audio.as_ref());
        }
    }

    /// `laststn.osn` in the map's folder (the content folder's copy: the original is never
    /// written), as OMSI keeps it: the launcher offers to continue it. Not
    /// in a tutorial, a LAN session or without a bus of one's own.
    pub(crate) fn save_last_situation(&mut self) -> Option<std::path::PathBuf> {
        if self.tutorial.is_some() || self.in_lan_session() || self.player.is_none() {
            return None;
        }
        let (Some(w), Some(cam)) = (self.world.as_ref(), self.camera.as_ref()) else {
            return None;
        };
        let dir = std::path::Path::new(&self.args.map.replace('\\', "/"))
            .parent()
            .map(|d| d.to_path_buf())?;
        let base = crate::startup::content_dir()?;
        let dir = base.join(dir);
        let _ = std::fs::create_dir_all(&dir);
        let out = dir.join("laststn.osn");
        let sit = build_situation(
            &self.args,
            w,
            &self.clock,
            self.args.weather.as_deref(),
            self.player.as_ref(),
            &self.placed,
            cam,
            self.duty.as_ref(),
            "Last situation",
        );
        match sit.save(&out) {
            Ok(()) => {
                crate::situation::save_script_textures(&out, &self.situation_vehicles());
                log::info!("saved the last situation {}", out.display());
                Some(out)
            }
            Err(e) => {
                log::warn!("saving {}: {e}", out.display());
                None
            }
        }
    }

    /// The vehicles a situation saves, in the order `build_situation` writes them: the
    /// one driven, then the ones placed besides it.
    fn situation_vehicles(&self) -> Vec<&omsi_sim::VehicleInstance> {
        self.player
            .iter()
            .chain(self.placed.iter())
            .map(|p| &p.vehicle)
            .collect()
    }

    /// The graphics device was lost (the driver reset the card: it ran out of memory, or a
    /// frame took it too long): the game starts again by itself on the situation just
    /// saved, with lighter graphics (`Settings::apply_safe_gpu`), and on Windows with the
    /// other graphics interface when the lost one was Vulkan. Twice at most in a row. False
    /// when it cannot (a LAN session, the tutorial, nothing to save): the session ends.
    pub(crate) fn restart_after_device_loss(&mut self) -> bool {
        let n = omsi_cfg::flags::SAFE_GPU.number::<u32>().unwrap_or(0);
        if n >= 2 {
            return false;
        }
        let Some(file) = self.save_last_situation() else {
            return false;
        };
        let Ok(exe) = std::env::current_exe() else {
            return false;
        };
        let mut cmd = std::process::Command::new(exe);
        cmd.arg("--root")
            .arg(&self.args.root)
            .arg("--no-menu")
            .arg("--situation")
            .arg(&file);
        // (the duty typed by itself goes on being typed: `--autostart` with a situation)
        if self.player.as_ref().is_some_and(|p| p.duty_typed) {
            cmd.arg("--autostart");
        }
        cmd.env(omsi_cfg::flags::SAFE_GPU.name, (n + 1).to_string());
        // (on Windows the other interface: DirectX 12 after Vulkan, Vulkan after DirectX 12 -
        // an AMD Radeon's DX12 driver lost the device where its Vulkan one did not, #274)
        let name = self
            .renderer
            .as_ref()
            .map(|r| r.adapter_name.clone())
            .unwrap_or_default();
        if cfg!(windows) {
            if name.contains("(Vulkan)") {
                cmd.env(omsi_cfg::flags::BACKEND.name, "dx12");
            } else if name.contains("(Dx12)") {
                cmd.env(omsi_cfg::flags::BACKEND.name, "vulkan");
            }
        }
        match cmd.spawn() {
            Ok(_) => {
                log::warn!(
                    "starting again with safer graphics on {} (the graphics device was lost)",
                    file.display()
                );
                true
            }
            Err(e) => {
                log::warn!("could not start the game again: {e}");
                false
            }
        }
    }

    /// Quick save: `Situations/quicksave.osn` next to the game, as OMSI's `quicksave`.
    pub(crate) fn quick_save(&mut self) {
        let (Some(w), Some(cam)) = (self.world.as_ref(), self.camera.as_ref()) else {
            return;
        };
        // into openOMSI's content folder, never the original installation (the menu and
        // --situation find it there as they find a mod's files)
        let dir = crate::startup::content_dir()
            .unwrap_or_else(|| self.args.root.clone())
            .join("Situations");
        let out = dir.join("quicksave.osn");
        let sit = build_situation(
            &self.args,
            w,
            &self.clock,
            self.args.weather.as_deref(),
            self.player.as_ref(),
            &self.placed,
            cam,
            self.duty.as_ref(),
            "Quicksave",
        );
        match save_or_fallback(
            &sit,
            &out,
            Path::new("Situations").join("quicksave.osn").as_path(),
        ) {
            Ok(out) => {
                crate::situation::save_script_textures(&out, &self.situation_vehicles());
                log::info!(
                    "saved situation {} ({} vehicles)",
                    out.display(),
                    sit.vehicles.len()
                );
                self.service_msg = Some(("Situation saved (quicksave)".into(), 3.0));
            }
            Err(e) => {
                log::warn!("saving {}: {e}", out.display());
                self.service_msg = Some((format!("Could not save: {e}"), 5.0));
            }
        }
    }

    /// A save of its own (#341): the situation into the next free `Saves/Slot <n>.osn` of
    /// the map's folder in the content folder - none is ever overwritten. The launcher
    /// offers them, with the last situation, to continue from.
    pub(crate) fn save_slot(&mut self) {
        let (Some(w), Some(cam)) = (self.world.as_ref(), self.camera.as_ref()) else {
            return;
        };
        let Some(dir) = crate::startup::content_dir().and_then(|base| {
            std::path::Path::new(&self.args.map.replace('\\', "/"))
                .parent()
                .map(|d| base.join(d).join(SAVES))
        }) else {
            return;
        };
        let _ = std::fs::create_dir_all(&dir);
        // (the slots of the fallback folder count too: a number is never given twice)
        let rel_dir = std::path::Path::new(&self.args.map.replace('\\', "/"))
            .parent()
            .map(|d| d.join(SAVES))
            .unwrap_or_default();
        let fallback = crate::startup::save_fallback_dir().map(|f| f.join(&rel_dir));
        let taken = |n: usize| {
            dir.join(format!("Slot {n}.osn")).exists()
                || fallback
                    .as_ref()
                    .is_some_and(|f| f.join(format!("Slot {n}.osn")).exists())
        };
        let Some(n) = (1..10_000).find(|n| !taken(*n)) else {
            return;
        };
        let out = dir.join(format!("Slot {n}.osn"));
        let bus = self.player.as_ref().map(|p| {
            let d = &p.vehicle.ty.def;
            if d.type_name.trim().is_empty() {
                d.path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default()
            } else {
                d.type_name.trim().to_string()
            }
        });
        let t = self.clock.time;
        let what = match (bus, self.duty.as_ref()) {
            (Some(b), Some(d)) => format!("{b}, line {} / {}", d.line.trim(), d.tour.trim()),
            (Some(b), None) => b,
            (None, _) => "on foot".to_string(),
        };
        let name = format!(
            "Slot {n}: {what}, {:02}:{:02}",
            (t / 3600.0) as i32 % 24,
            ((t % 3600.0) / 60.0) as i32
        );
        let sit = build_situation(
            &self.args,
            w,
            &self.clock,
            self.args.weather.as_deref(),
            self.player.as_ref(),
            &self.placed,
            cam,
            self.duty.as_ref(),
            &name,
        );
        match save_or_fallback(&sit, &out, rel_dir.join(format!("Slot {n}.osn")).as_path()) {
            Ok(out) => {
                crate::situation::save_script_textures(&out, &self.situation_vehicles());
                log::info!(
                    "saved situation {} ({} vehicles)",
                    out.display(),
                    sit.vehicles.len()
                );
                self.service_msg = Some((
                    format!("Saved as slot {n}: the launcher continues from it"),
                    4.0,
                ));
            }
            Err(e) => {
                log::warn!("saving {}: {e}", out.display());
                self.service_msg = Some((format!("Could not save: {e}"), 5.0));
            }
        }
    }

    /// OMSI's `screenshot`: the picture into the content folder's `Screenshots`, named by the
    /// time, the map and the vehicle the player is in (see `screenshot_name`).
    pub(crate) fn take_screenshot(&mut self) {
        let dir = crate::startup::content_dir()
            .unwrap_or_else(|| self.args.root.clone())
            .join("Screenshots");
        let _ = std::fs::create_dir_all(&dir);
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let map = crate::screenshot_name::folder_of(&self.args.map).unwrap_or_default();
        // in a vehicle: at the wheel, or aboard the own bus on foot; not on foot outside it
        let aboard = self.player.is_some()
            && (self.on_foot.is_none() || self.foot_bus() == Some(crate::humans::BusId::Player));
        let vehicle = if aboard {
            self.args
                .bus
                .as_deref()
                .and_then(crate::screenshot_name::folder_of)
        } else {
            None
        };
        let path = dir.join(crate::screenshot_name::build(
            secs,
            &map,
            vehicle.as_deref(),
        ));
        // (the notice with the path is shown once the picture is taken, never inside it; one
        // still showing from a shot a moment ago would be inside this one)
        if self
            .service_msg
            .as_ref()
            .is_some_and(|(m, _)| crate::screenshot_name::is_notice(m))
        {
            self.service_msg = None;
        }
        self.shot = Some((path, false));
    }

    /// On foot, the own bus is within reach: inside it, or standing by it (a hand's reach
    /// round its body; a click still has to hit one of its meshes).
    pub(crate) fn foot_reaches_bus(&self) -> bool {
        if self.foot_bus() == Some(crate::humans::BusId::Player) {
            return true;
        }
        match (self.player.as_ref(), self.camera.as_ref()) {
            // (every part of an articulated bus: a door button of the rear section is in
            // reach standing by that section, however far the front one is - #715)
            (Some(p), Some(c)) => {
                let v = &p.vehicle;
                std::iter::once((v.position, v.heading, v.ty.def.bounding_box))
                    .chain(
                        v.trailers
                            .iter()
                            .map(|t| (t.position, t.heading, t.ty.def.bounding_box)),
                    )
                    .any(|(at, heading, bb)| part_in_reach(c.position, at, heading, bb))
            }
            _ => false,
        }
    }
}
