//! Input that is not the input script: the keyboard, the mouse, the game menu's lists and the
//! object editor's keys.

use super::*;

mod editor_input;
mod keyboard;
mod menus;
mod mouse;

// (the look tests in mouse.rs also check two of the keyboard's helpers)
#[cfg(test)]
pub(crate) use keyboard::*;
pub(crate) use menus::*;
pub(crate) use mouse::*;
