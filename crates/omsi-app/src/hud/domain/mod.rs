//! What the HUD is about: the objects a frame hands it and their fields, with no drawing.

mod frame;
mod info;
mod notice;

pub use frame::{ChatView, DropdownView, Frame, MenuKind, Preview};
pub use info::{InfoBar, InfoItem, InfoTone};
pub use notice::{Notice, NoticeKind, NOTICES_SHOWN};
