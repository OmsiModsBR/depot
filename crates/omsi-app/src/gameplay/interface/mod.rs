//! The ports of the game play: `input` is what the rest of the game reaches of it, `output`
//! what the game play takes from the rest of the game.

mod input;
pub(in crate::gameplay) mod output;

pub(crate) use input::*;
