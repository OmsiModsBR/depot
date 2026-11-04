//! The ports of the weather: `input` is what the rest of the game reaches of it, `output`
//! what the weather takes from the rest of the game.

mod input;
pub(in crate::weather) mod output;

pub(crate) use input::*;
