//! The new interface in the game, drawn with `omsi_ui` through the renderer's overlay hook,
//! by screen: the loading screen, the end-of-trip report, the Alt bar and its windows and the
//! "Add vehicle" window so far (the Esc menu and its pages are still in the `new_ui*` modules).
//!
//! Each screen is in three parts: `domain/` holds its state and what it shows in a frame,
//! `interface/` what the game calls on it and what it asks of the game, and
//! `implementation/` its drawing, its clicks and keys and its side of the game's frame.

pub(crate) mod domain;
pub(crate) mod implementation;
mod interface;

pub use interface::*;
