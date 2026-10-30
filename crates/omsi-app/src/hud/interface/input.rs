//! What the game hands the HUD each frame and calls on it, and what it reads back (where
//! the menu's lines and controls were drawn, for the mouse).

pub use super::super::domain::{
    ChatView, DropdownView, Frame, Hits, InfoBar, InfoCell, InfoItem, InfoTone, MenuKind, Preview,
};
pub(crate) use super::super::implementation::legacy::shift_overlays;
pub use super::super::implementation::legacy::{backdrop, fps_level, size_factor, Hud, Ui};
