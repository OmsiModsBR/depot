//! The new interface in the game, drawn with `omsi_ui` through the renderer's overlay hook,
//! by screen: the loading screen and the end-of-trip report so far (the Esc menu, its pages,
//! the Alt bar and its windows are still in the `new_ui*` modules).
//!
//! Each screen is in three parts: `domain/` holds its state and what it shows in a frame,
//! `interface/` what the game calls on it and what it asks of the game, and
//! `implementation/` its drawing, its clicks and keys and its side of the game's frame.

pub(crate) mod domain;
pub(crate) mod implementation;
mod interface;

pub use interface::*;
