//! The settings both the launcher and the game's own menus read: the window sizes offered
//! and whether the chosen graphics API fixes the graphics to OMSI 2's.
//!
//! `domain` holds the choices offered, `implementation` what follows from the settings chosen,
//! `interface` what the windows call (`input`) and what it reads from the rest of the
//! workspace (`output`).

mod domain;
mod implementation;
mod interface;

pub use interface::input::*;
