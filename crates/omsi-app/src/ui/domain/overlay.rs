//! What the game hands the overlay before each render ([`State`], shared through
//! [`Shared`]), and the game's side of the new interface ([`NewUi`]).

use super::menu::{Area, MenuFrame};
use glam::Vec2;
use std::sync::{Arc, Mutex};

/// What the game tells the overlay each frame.
#[derive(Default)]
pub struct State {
    pub menu: Option<MenuFrame>,
    /// A page behind one of the menu's lines, while one is open.
    pub page: Option<crate::ui::PageFrame>,
    pub(in crate::ui) alt: Option<crate::ui::AltFrame>,
    /// The "Add vehicle" window.
    pub(in crate::ui) add: Option<crate::ui::AddFrame>,
    /// The end-of-trip report.
    pub(in crate::ui) report: Option<crate::ui::ReportFrame>,
    /// "Continue" was clicked in the report.
    pub(in crate::ui) report_done: bool,
    /// Clicks and the wheel over the window showing, not yet handed to it (one a frame).
    pub(in crate::ui) win_input: std::collections::VecDeque<crate::ui::WinInput>,
    /// What clicks in that window chose, for the game to apply.
    pub(in crate::ui) add_events: Vec<omsi_ui::ingame::AddVehicleEvent>,
    /// A dropdown of that window has its list open: every click goes to the window then.
    pub(in crate::ui) add_modal: bool,
    /// The picture of the vehicle and paint chosen there, read on its thread and not yet in
    /// the GPU (none inside: no picture was drawn of them).
    pub(in crate::ui) add_picture: Option<(u64, Option<crate::ui::Picture>)>,
    /// The picture the window wants now: a picture read for one chosen before is dropped.
    pub(in crate::ui) add_picture_want: Option<u64>,
    /// The window is open (also while the Esc menu or the report hides it: its picture stays
    /// in the GPU then).
    pub(in crate::ui) add_open: bool,
    /// The live 3D view's latest frame, not yet in the GPU: which vehicle and paint it shows
    /// (`showroom::tag`), its size and RGBA.
    pub(in crate::ui) add_live: Option<(u64, u32, u32, Vec<u8>)>,
    /// The Alt bar's window open (reposition, date and time, weather...).
    pub(in crate::ui) tool: Option<crate::ui::ToolFrame>,
    /// What clicks in it asked for, for the game to apply.
    pub(in crate::ui) tool_events: Vec<omsi_ui::alt_windows::Event>,
    /// The loading screen, while the first area of the map loads.
    pub loading: Option<crate::ui::LoadingFrame>,
    /// Screenshots decoded on their thread and not yet in the GPU.
    pub slides: Vec<crate::ui::Decoded>,
    /// The bus's block of the loading card, once its file has been read.
    pub bus_card: Option<crate::ui::CardText>,
    /// The map's own picture for its block of the card, decoded and not yet in the GPU.
    pub(in crate::ui) map_picture: Option<crate::ui::Decoded>,
    /// Which loading the screenshots and the bus card being read belong to (one more each
    /// time a loading is over).
    pub loading_gen: u64,
    /// The cursor, in window pixels.
    pub mouse: Vec2,
    /// Where in the window the interface is drawn (none: the whole picture).
    pub(in crate::ui) area: Option<Area>,
}

pub type Shared = Arc<Mutex<State>>;

/// The game's side of the overlay.
#[derive(Default)]
pub struct NewUi {
    pub shared: Shared,
    pub(in crate::ui) installed: bool,
    pub slides_started: bool,
    pub bus_started: bool,
    pub(in crate::ui) map_picture_started: bool,
    pub(crate) alt: crate::ui::AltState,
    /// The "Add vehicle" window, while it is open.
    pub(crate) add: Option<crate::ui::AddWindow>,
    /// Keys the "Add vehicle" window's registration field took: their release is its too.
    pub(crate) add_taken: std::collections::HashSet<winit::keyboard::KeyCode>,
    /// The vehicle and paint whose picture the "Add vehicle" window asked for last.
    pub(in crate::ui) add_picture: Option<u64>,
    /// The manufacturer and the vehicle file chosen last in that window: it opens on them.
    pub(crate) add_last: Option<(String, String)>,
    /// That window's live 3D view, while the player has it on.
    pub(crate) add_live: Option<crate::ui::LiveView>,
    /// The page open behind a line of the Esc menu, as the HUD's frame of this frame made it.
    pub page: Option<crate::ui::PageFrame>,
    /// The headset's picture of the menu and the page (a render texture of the window's size,
    /// and its size), and the plate that dims the view behind them.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub(in crate::ui) headset: Option<(omsi_render::TextureId, u32, u32)>,
    #[cfg_attr(not(windows), allow(dead_code))]
    pub(in crate::ui) headset_dim: Option<omsi_render::TextureId>,
    /// The Alt bar's other windows.
    pub(crate) tool: crate::ui::Tools,
    /// The end-of-trip report, while it is open.
    pub(crate) report: Option<crate::ui::Report>,
    /// The trip whose report was shown last (line, tour, trip in the tour): it opens once.
    pub(crate) report_shown: Option<(String, String, usize)>,
    /// A trip at its last stop whose report waits for the doors to open there: its key, its
    /// place in the duty and where that stop is.
    pub(crate) report_watch: Option<crate::ui::Watch>,
}
