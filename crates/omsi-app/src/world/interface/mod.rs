//! The ports of the world: `input` is what the rest of the game reaches of it, `output` what
//! the world takes from the rest of the game.

mod input;
pub(in crate::world) mod output;

pub(crate) use input::*;
