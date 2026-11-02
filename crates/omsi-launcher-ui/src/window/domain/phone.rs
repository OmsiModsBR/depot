//! What the phone's launcher keeps between frames (`PhoneView`): the tab shown, the sheet open
//! over it and the page of More; and what a frame of it leaves the game to draw (`PhoneFrame`).

use super::super::interface::output::Page;
use omsi_ui::Rect;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Tab {
    #[default]
    Play,
    Online,
    Mods,
    More,
}

/// A choice made on a sheet of its own.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sheet {
    Map,
    Bus,
    Livery,
    /// HOF, fleet number and registration plate.
    Vehicle,
    Duty,
    Tour,
    Time,
    /// Spawn and service options that the desktop Drive page exposes.
    Start,
    /// The selected duty's roadbook and IBIS hint.
    Roadbook,
}

#[derive(Default)]
pub struct PhoneView {
    pub tab: Tab,
    pub sheet: Option<Sheet>,
    /// A page of More that is open.
    pub page: Option<Page>,
    pub filter: String,
}

/// A frame of the phone's launcher as `window::phone` drew it: the game draws the desktop page
/// it shows (`page`), then `window::phone_foot` and `window::phone_toast`.
pub struct PhoneFrame {
    /// The screen above the tab bar.
    pub body: Rect,
    /// A sheet fills the screen: nothing more is drawn this frame.
    pub sheet: bool,
    /// A desktop page to draw, full width, and where (clipped to the screen until
    /// `phone_foot`).
    pub page: Option<(Page, Rect)>,
    /// That page opened from More: its bar has a way back.
    pub(crate) back: bool,
    /// A page of More chosen: the window goes to it.
    pub open: Option<Page>,
}
