//! The next start the launcher prepares (the Drive and Maps pages): the choices remembered
//! between launches (`Choice`), the season and the weather that fits it, the depot file used
//! when none is picked, the duty handed to the game and the trip it begins with, the starred
//! buses and the airport of the real weather.
//!
//! `domain` holds the choices and the seasons, `implementation` how a start is put together
//! and stored, `interface` what the window calls (`input`) and what it reads from the rest of
//! the crate (`output`).

mod domain;
mod implementation;
mod interface;

pub use interface::input::*;
