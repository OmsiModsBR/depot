//! The game play: for now the AI road traffic (the vehicles on the path network, the
//! traffic lights, the parked cars, how many of them the options ask for, and what the
//! traffic is told about the player) and the people on foot (passengers and pedestrians);
//! the bus service and the player follow.
//!
//! In three parts: `domain/` holds the traffic's and the people's types and the density
//! rules, `interface/` what the rest of the game reaches of the game play, and
//! `implementation/` the procedures.

pub(crate) mod domain;
pub(crate) mod implementation;
mod interface;

pub(crate) use interface::*;
