//! Personal cockpit navigator placement, stored per vehicle outside the repository.

use crate::output::domain::vr_navigator::{
    bus_key, driver_origin, Display, Editing, Placement, Profiles,
};
use crate::output::interface::output::App;
use glam::Vec3;
use std::path::Path;

impl Profiles {
    pub fn load() -> Self {
        let Some(path) =
            crate::settings::Settings::path().map(|p| p.with_file_name("vr-navigator.json"))
        else {
            return Self::default();
        };
        match std::fs::read(&path) {
            Ok(data) => match serde_json::from_slice::<Self>(&data) {
                Ok(mut profiles) => {
                    for value in profiles.buses.values_mut() {
                        *value = value.sanitize();
                    }
                    profiles
                }
                Err(e) => {
                    log::warn!("VR navigator: cannot read {}: {e}", path.display());
                    Self::default()
                }
            },
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    log::warn!("VR navigator: {}: {e}", path.display());
                }
                Self::default()
            }
        }
    }

    pub fn get(&self, key: &str) -> Placement {
        self.buses.get(key).copied().unwrap_or_default()
    }

    fn save(&self) -> anyhow::Result<()> {
        let path = crate::settings::Settings::path()
            .ok_or_else(|| anyhow::anyhow!("no user settings directory"))?
            .with_file_name("vr-navigator.json");
        self.save_to(&path)
    }

    pub fn save_to(&self, path: &Path) -> anyhow::Result<()> {
        std::fs::create_dir_all(path.parent().unwrap())?;
        omsi_cfg::write_atomic(path, serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }
}

impl App {
    pub(crate) fn start_vr_nav_edit(&mut self) {
        if !self.vr_active() || self.player.is_none() || self.vr_nav_edit.is_some() {
            return;
        }
        self.on_left(false);
        // The editor consumes key-up events; release existing driving and script
        // inputs before entering it so none remain held when driving resumes.
        if let Some(p) = self.player.as_mut() {
            let held: Vec<_> = p.held_keys.keys().copied().collect();
            for scan in held {
                p.key(scan, 0, false);
            }
            p.axes.release_all();
            for name in self.door_key_triggers.drain().flat_map(|(_, names)| names) {
                let off = format!("{name}_off");
                if p.vehicle.ty.program.trigger(&off).is_some() {
                    p.vehicle.trigger(&off);
                }
            }
        }
        self.buttons_held = (false, false);
        if self.game_menu.is_some() {
            self.close_game_menu();
        }
        self.chooser = None;
        self.admin_list = None;
        self.list_kind = None;
        self.dropdown = None;
        self.menu_drag = None;
        self.menu_edit = None;
        self.view = "driver".into();
        self.vr_nav_edit = Some(Editing {
            moving: false,
            rotating: false,
            paused_before: self.paused,
            mouse_drive_before: self.mouse_drive,
        });
        if !self.in_lan_session() {
            self.paused = true;
        }
        self.mouse_drive = false;
        self.mouse_look = false;
        self.both_drag = None;
        self.keys.clear();
        self.hover_key = None;
        #[cfg(windows)]
        {
            self.vr_zoom_active = false;
            self.reset_vr_pointer();
        }
        if let Some(n) = self.navigator.as_mut() {
            if n.map_open() {
                n.toggle_map();
            }
        }
        if let Some(p) = self.player.as_ref() {
            let key = bus_key(&p.vehicle.ty.def.path, &self.args.root);
            self.vr_nav_profiles.buses.entry(key).or_default().enabled = true;
        }
        if let Some(window) = self.window.as_ref() {
            let _ = window.set_cursor_grab(winit::window::CursorGrabMode::Confined);
            window.set_cursor_visible(false);
        }
        self.cursor_hidden = None;
    }

    pub(crate) fn finish_vr_nav_edit(&mut self) {
        let Some(edit) = self.vr_nav_edit.take() else {
            return;
        };
        self.paused = edit.paused_before;
        self.mouse_drive = edit.mouse_drive_before;
        self.mouse_look = false;
        self.hover_key = None;
        self.keys.clear();
        self.cursor_hidden = None;
        #[cfg(windows)]
        self.reset_vr_pointer();
        if let Some(window) = self.window.as_ref() {
            let _ = window.set_cursor_grab(winit::window::CursorGrabMode::None);
            window.set_cursor_visible(true);
            if edit.mouse_drive_before {
                let _ = window.set_cursor_position(winit::dpi::PhysicalPosition::new(
                    self.cursor.0 as f64,
                    self.cursor.1 as f64,
                ));
            }
        }
        self.save_vr_nav_profiles();
    }

    /// The headset's axes in vehicle space, so a drag follows the direction the
    /// driver is looking while the resulting placement stays attached to the bus.
    fn vr_nav_edit_geometry(&self) -> Option<(Vec3, Vec3, Vec3, Vec3)> {
        let p = self.player.as_ref()?;
        let def = &p.vehicle.ty.def;
        let driver = def
            .cameras_driver
            .get(def.camera_std)
            .or(def.cameras_driver.first())?;
        let eye = *self.camera.as_ref()?;
        #[cfg(windows)]
        let eye = self
            .vr
            .as_ref()
            .and_then(|vr| vr.navigator_edit_camera())
            .unwrap_or(eye);
        let inverse = p.vehicle.body_rotation().inverse();
        Some((
            inverse.transform_vector3(eye.right()),
            inverse.transform_vector3(eye.up()),
            inverse.transform_point3((eye.position - p.vehicle.position).as_vec3()),
            driver_origin(driver),
        ))
    }

    pub(crate) fn vr_nav_drag(&mut self, dx: f32, dy: f32) {
        let Some(edit) = self.vr_nav_edit.as_ref() else {
            return;
        };
        if !edit.moving && !edit.rotating {
            return;
        }
        let (moving, rotating) = (edit.moving, edit.rotating);
        let Some((right, up, eye, driver)) = self.vr_nav_edit_geometry() else {
            return;
        };
        let key = bus_key(
            &self.player.as_ref().unwrap().vehicle.ty.def.path,
            &self.args.root,
        );
        let p = self.vr_nav_profiles.buses.entry(key).or_default();
        if moving {
            let distance = (driver + Vec3::from(p.offset) - eye)
                .length()
                .clamp(0.25, 2.5);
            p.offset =
                (Vec3::from(p.offset) + (right * dx - up * dy) * distance * 0.0015).to_array();
        }
        if rotating {
            use winit::keyboard::KeyCode;
            if self.keys.contains(&KeyCode::ShiftLeft) || self.keys.contains(&KeyCode::ShiftRight) {
                p.roll += dx * 0.25;
            } else {
                p.yaw -= dx * 0.25;
                p.tilt -= dy * 0.25;
            }
        }
        *p = p.sanitize();
    }

    pub(crate) fn vr_nav_scroll(&mut self, amount: f32) {
        if !amount.is_finite() {
            return;
        }
        let Some((_, _, eye, driver)) = self.vr_nav_edit_geometry() else {
            return;
        };
        let key = bus_key(
            &self.player.as_ref().unwrap().vehicle.ty.def.path,
            &self.args.root,
        );
        let p = self.vr_nav_profiles.buses.entry(key).or_default();
        use winit::keyboard::KeyCode;
        let resize =
            self.keys.contains(&KeyCode::ControlLeft) || self.keys.contains(&KeyCode::ControlRight);
        p.scroll(amount, resize, eye, driver);
    }

    fn save_vr_nav_profiles(&mut self) {
        if let Err(e) = self.vr_nav_profiles.save() {
            log::warn!("VR navigator: saving placement: {e}");
            self.service_msg = Some((
                format!("{}: {e}", omsi_ui::tr("Could not save navigator position")),
                5.0,
            ));
        }
    }
    pub(crate) fn vr_nav_profile(&self) -> Placement {
        self.player
            .as_ref()
            .map(|p| {
                self.vr_nav_profiles
                    .get(&bus_key(&p.vehicle.ty.def.path, &self.args.root))
            })
            .unwrap_or_default()
    }

    pub(crate) fn vr_nav_display(&self) -> Option<Display> {
        if !self.vr_active() || self.view != "driver" {
            return None;
        }
        let p = self.player.as_ref()?;
        let def = &p.vehicle.ty.def;
        let eye = def
            .cameras_driver
            .get(def.camera_std)
            .or(def.cameras_driver.first())?;
        let placement = self.vr_nav_profile();
        Some(Display {
            placement,
            local_center: driver_origin(eye) + Vec3::from(placement.offset),
        })
    }

    pub(crate) fn vr_nav_adjust(&mut self, field: &str, direction: f32) {
        self.update_vr_nav_profile(|p| p.adjust(field, direction));
    }

    pub(crate) fn vr_nav_set(&mut self, field: &str, value: f32) {
        self.update_vr_nav_profile(|p| p.set_value(field, value));
    }

    fn update_vr_nav_profile(&mut self, update: impl FnOnce(&mut Placement)) {
        if !self.vr_active() {
            return;
        }
        let Some(player) = self.player.as_ref() else {
            return;
        };
        let key = bus_key(&player.vehicle.ty.def.path, &self.args.root);
        update(self.vr_nav_profiles.buses.entry(key).or_default());
        if self.vr_nav_edit.is_none() {
            self.save_vr_nav_profiles();
        }
    }
}
