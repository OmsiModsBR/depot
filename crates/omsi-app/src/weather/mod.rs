//! The weather: the session's weather and its sky (`.owt`, clouds, wet streets), the
//! weather cycle, the physical weather model, changing the weather and the clock, rain and
//! snow around the camera, tyre spray from wet roads, condensation on the bus's windows and
//! the environment sounds.
//!
//! In three parts: `domain/` holds the weather cycle's choice and the physical model,
//! `interface/` what the rest of the game reaches of the weather, and `implementation/` the
//! procedures.

pub(crate) mod domain;
pub(crate) mod implementation;
mod interface;

pub(crate) use interface::*;
