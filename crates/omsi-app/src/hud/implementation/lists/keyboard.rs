//! The keyboard's settings window: its actions, its pages and capturing a key.

use super::rows::{opens, row};
use super::store::remember_setting;
use crate::core::app::App;
use crate::hud::domain::lists::Page;

pub(crate) fn keyboard_actions(app: &App) -> Vec<String> {
    let cfg = keyboard_cfg(app);
    cfg.vehicles
        .into_iter()
        .chain(cfg.game)
        .map(|b| b.action)
        .collect()
}

pub(in crate::hud) fn keyboard_cfg(app: &App) -> omsi_content::KeyboardCfg {
    omsi_content::KeyboardCfg::load(&crate::core::startup::keyboard_cfg(&app.args.root))
        .unwrap_or_default()
        .with_game_defaults()
        .with_vr_defaults()
}

pub(in crate::hud) fn write_keyboard_cfg(
    _app: &App,
    cfg: &omsi_content::KeyboardCfg,
) -> Result<(), String> {
    let dir = crate::core::startup::content_dir()
        .ok_or_else(|| "No writable Project Depot content folder was found".to_string())?
        .join("Inputs");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("keyboard.cfg");
    let tmp = path.with_extension("cfg.tmp");
    cfg.save(&tmp).map_err(|e| e.to_string())?;
    omsi_content::KeyboardCfg::load(&tmp).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    Ok(())
}

impl App {
    fn install_keyboard_cfg(&mut self, cfg: omsi_content::KeyboardCfg) {
        let runtime = cfg.with_game_defaults().with_vr_defaults();
        self.game_keys = runtime.game.clone();
        if let Some(p) = self.player.as_mut() {
            p.bindings = runtime.vehicles;
        }
        self.own_keys = crate::core::startup::own_keys(&self.args.root);
        self.own_shift =
            crate::core::startup::own_bindings(&self.args.root, omsi_content::input::KEY_SHIFT);
        // A key changed by the player must win over the ready-made W/A/S/D or arrow
        // presets just as a key changed on the launcher's Controls page does.
        self.args.drive_keys = "omsi".into();
        self.settings.drive_keys = "omsi".into();
        remember_setting("drive_keys", "omsi");
    }

    pub(crate) fn begin_key_capture(&mut self, game: bool, index: usize) {
        self.key_capture = Some((game, index));
        self.service_msg = Some((
            "Press a key for this action (Delete clears it, Esc cancels)".into(),
            5.0,
        ));
        self.refresh_list();
    }

    pub(crate) fn cancel_key_capture(&mut self) {
        if self.key_capture.take().is_some() {
            self.service_msg = Some(("Key change cancelled".into(), 2.0));
            self.refresh_list();
        }
    }

    pub(crate) fn apply_key_capture(&mut self, scan: Option<i32>, chord: i32) {
        let Some((game, index)) = self.key_capture else {
            return;
        };
        let mut cfg = keyboard_cfg(self);
        let list = if game {
            &mut cfg.game
        } else {
            &mut cfg.vehicles
        };
        let Some(binding) = list.get_mut(index) else {
            self.key_capture = None;
            self.service_msg = Some(("That key entry no longer exists".into(), 3.0));
            self.refresh_list();
            return;
        };
        let hold = binding.modifier & omsi_content::input::KEY_HOLD;
        binding.scan_code = scan.unwrap_or(0);
        binding.modifier = if scan.is_some() { hold | chord } else { hold };
        let action = binding.action.clone();
        match write_keyboard_cfg(self, &cfg) {
            Ok(()) => {
                self.key_capture = None;
                self.install_keyboard_cfg(cfg);
                let key = scan
                    .map(|s| crate::keys::key_name(s as i64, (hold | chord) as i64))
                    .unwrap_or_else(|| "(unbound)".into());
                self.service_msg = Some((
                    format!(
                        "{}: {key}",
                        crate::describe::names(&self.args.root, &self.settings.language)
                            .control(&action)
                    ),
                    3.0,
                ));
                self.refresh_list();
            }
            Err(e) => self.service_msg = Some((format!("Key binding was not saved: {e}"), 5.0)),
        }
    }

    pub(crate) fn add_key_event(&mut self, action: &str) {
        let action = action.trim();
        if action.is_empty() {
            return;
        }
        let mut cfg = keyboard_cfg(self);
        cfg.vehicles.push(omsi_content::KeyBinding {
            action: action.to_string(),
            scan_code: 0,
            modifier: 0,
        });
        let index = cfg.vehicles.len() - 1;
        match write_keyboard_cfg(self, &cfg) {
            Ok(()) => {
                self.install_keyboard_cfg(cfg);
                self.key_capture = Some((false, index));
                self.service_msg = Some((format!("KY_{action} added. Press its key now."), 5.0));
            }
            Err(e) => self.service_msg = Some((format!("Event was not added: {e}"), 5.0)),
        }
    }
}

pub(in crate::hud) fn keyboard_pages(app: &App) -> Vec<Page> {
    let cfg = keyboard_cfg(app);
    let names = crate::describe::names(&app.args.root, &app.settings.language);
    let mut vehicle = vec![opens(
        "Add a vehicle event…",
        "Choose a control supplied by the bus or its mods",
        "key_events",
    )];
    let mut game = Vec::new();
    for (is_game, bindings, rows) in [
        (false, &cfg.vehicles, &mut vehicle),
        (true, &cfg.game, &mut game),
    ] {
        let mut bindings: Vec<_> = bindings.iter().enumerate().collect();
        bindings.sort_by_key(|(_, b)| names.control(&b.action).to_lowercase());
        for (i, b) in bindings {
            let key = if app.key_capture == Some((is_game, i)) {
                "press a key…".into()
            } else {
                crate::keys::key_name(b.scan_code as i64, b.modifier as i64)
            };
            rows.push((
                row(
                    &names.control(&b.action),
                    'a',
                    &key,
                    "Enter to change; Delete clears; Esc cancels",
                    None,
                ),
                format!("keybind {} {i}", if is_game { "g" } else { "v" }),
            ));
        }
    }
    vec![("Driving and bus", vehicle), ("Game and camera", game)]
}
