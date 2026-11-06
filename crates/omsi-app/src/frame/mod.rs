//! The frame: the window's events (winit's `ApplicationHandler` for `App`), the frame
//! rate's "1 %" low, the ending of the game from outside (SIGTERM, the console's Ctrl+C) and
//! the setting up of a session that the window and the offscreen run share (`session`).
//!
//! In three parts: `domain/` holds the frame's types, `interface/` what the rest of the game
//! reaches of the frame, and `implementation/` the procedures.

pub(crate) mod domain;
pub(crate) mod implementation;
mod interface;

pub(crate) use interface::*;
