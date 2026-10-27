//! Input that is not the input script: the keyboard, the mouse, the game menu's lists and the
//! object editor's keys.

mod editor_input;
pub(crate) mod keyboard;
pub(crate) mod menus;
pub(crate) mod mouse;

pub(crate) use keyboard::route_char;
pub(crate) use keyboard::ROUTE_NUMBER_MAX;
pub(crate) use menus::*;
pub(crate) use mouse::*;
