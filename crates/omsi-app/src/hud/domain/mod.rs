//! What the HUD is about: the objects a frame hands it and their fields, with no drawing.

pub(crate) mod describe;
pub(crate) mod menu;
pub(crate) mod tr_fill;
mod frame;
mod hits;
mod info;
pub(crate) mod lists;
pub(crate) mod nav;

pub use frame::{ChatView, DropdownView, Frame, MenuKind, Preview};
pub use hits::{Hits, InfoTip};
pub use info::{InfoBar, InfoCell, InfoData, InfoItem, InfoMode, InfoTone};
