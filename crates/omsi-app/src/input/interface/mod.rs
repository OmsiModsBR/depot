//! The ports of the input: `input` is what the rest of the game reaches of it, `output` what
//! the input takes from the rest of the game.

mod input;
pub(in crate::input) mod output;

pub(crate) use input::*;
