//! The game's side of the Controls page's game controllers tab (`omsi_launcher_ui::window::Pads`):
//! the devices as the game reads them (`crate::controllers`), kept open between frames in
//! `PagesView::pad_io`.

use super::pages::PagesView;
use omsi_content::input::gamectrler::DeviceCfg;
use omsi_launcher_lib::controllers::Connected;
use omsi_launcher_ui::window::Pads;

/// The devices the tab has taken, the window they belong to (Windows) and the game's folder.
pub(crate) struct GamePads<'a> {
    pub io: &'a mut Option<crate::controllers::Devices>,
    pub hwnd: Option<isize>,
    pub root: &'a str,
}

impl Pads for GamePads<'_> {
    fn open(&mut self, feedback: bool) {
        // (the ones taken before are given up first)
        *self.io = None;
        *self.io = Some(crate::controllers::Devices::new(self.hwnd, feedback));
    }

    fn close(&mut self) {
        *self.io = None;
    }

    fn is_open(&self) -> bool {
        self.io.is_some()
    }

    fn poll(&mut self) -> Vec<(String, usize, bool)> {
        self.io.as_mut().map(|io| io.poll()).unwrap_or_default()
    }

    fn connected(&self) -> Vec<Connected> {
        self.io
            .as_ref()
            .map(|io| io.connected())
            .unwrap_or_default()
    }

    fn buttons_only(&self, name: &str) -> bool {
        self.io.as_ref().is_some_and(|io| io.buttons_only(name))
    }

    fn calibration_pulse(&mut self, name: &str, axis: usize, force: f32) -> bool {
        self.io
            .as_mut()
            .is_some_and(|io| io.calibration_pulse(name, axis, force))
    }

    fn read_cfg(&self) -> Vec<DeviceCfg> {
        crate::controllers::read_cfg(std::path::Path::new(self.root))
    }

    fn held_gears(&self, devices: &[DeviceCfg]) -> bool {
        crate::hpattern::has_held_bindings(devices)
    }
}

impl PagesView {
    /// The force feedback test stopped, and its devices given back.
    pub fn cancel_feedback_test(&mut self) {
        let mut pads = GamePads {
            io: &mut self.pad_io,
            hwnd: None,
            root: "",
        };
        self.pads.cancel_feedback_test(&mut pads);
    }

    /// Give every controller handle up before handing the hardware to the game. The normal
    /// controller list uses non-exclusive DirectInput too, not only the force-feedback test.
    pub(super) fn release_io(&mut self) {
        self.cancel_feedback_test();
        self.pad_io = None;
    }
}
