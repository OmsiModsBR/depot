//! The frame: the window's events (winit's `ApplicationHandler` for `App`), the frame
//! rate's "1 %" low and the ending of the game from outside (SIGTERM, the console's Ctrl+C).
//!
//! In three parts: `domain/` holds the frame's types, `interface/` what the rest of the game
//! reaches of the frame, and `implementation/` the procedures.

pub(crate) mod domain;
pub(crate) mod implementation;
mod interface;

pub(crate) use interface::*;
