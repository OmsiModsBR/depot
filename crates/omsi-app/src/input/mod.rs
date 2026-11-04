//! The game's input: the keyboard, the mouse, the game menu's lists and the object editor's
//! keys; game controllers (wheels, pedals, joysticks, gamepads) through DirectInput, evdev and
//! macOS's HID and GameController, with their configuration page; the touch controls; head
//! tracking; the input script; and what a key, a menu or a page asks the game to do (`actions`).
//!
//! In three parts: `domain/` holds the key codes and the H-pattern gates, `interface/` what the
//! rest of the game reaches of the input, and `implementation/` the procedures.

pub(crate) mod domain;
pub(crate) mod implementation;
mod interface;

pub(crate) use interface::*;
