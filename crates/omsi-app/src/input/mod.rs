//! Input that is not the input script: the keyboard, the mouse, the game menu's lists and the
//! object editor's keys.

mod editor_input;
mod keyboard;
mod menus;
mod mouse;

pub(crate) use menus::*;
pub(crate) use mouse::*;
