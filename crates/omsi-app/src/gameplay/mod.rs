//! The game play: for now the AI road traffic (the vehicles on the path network, the
//! traffic lights, the parked cars, how many of them the options ask for, and what the
//! traffic is told about the player), the people on foot (passengers and pedestrians) and
//! the bus service (the timetable and its AI buses, the cab's paper timetable, the lines'
//! departures, the duty's start and journey, the career and the cash desk); the player
//! follows.
//!
//! In three parts: `domain/` holds the traffic's, the people's and the timetable's types,
//! the density rules and the lines' departures, `interface/` what the rest of the game reaches of the game play, and
//! `implementation/` the procedures.

pub(crate) mod domain;
pub(crate) mod implementation;
mod interface;

pub(crate) use interface::*;
