//! The launcher's timetable editor, its data side: the departures as typed ("h:mm"), the
//! numbers and repeats of the tours, and where a changed line is written and how the map's own
//! timetable is put back.
//!
//! `domain` holds the times and the tours, `implementation` the saving and the reset,
//! `interface` what the window calls (`input`) and what it reads from the rest of the crate
//! (`output`).

mod domain;
mod implementation;
mod interface;

pub use interface::input::*;
