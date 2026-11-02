//! The weather a session starts with, its launcher side: a weather typed by hand (`custom:`),
//! the physical model (`natural`) and the airports of the METAR list.
//!
//! `domain` holds the typed weather, `implementation` the reading of `Weather/ICAO.txt`,
//! `interface` what the window and the game call (`input`) and what it reads from the rest of
//! the workspace (`output`).

mod domain;
mod implementation;
mod interface;

pub use interface::input::*;
