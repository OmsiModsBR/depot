//! The ports of the core: `input` is what the rest of the game reaches of it, `output` what
//! the core takes from the rest of the game.

mod input;
pub(in crate::core) mod output;

pub(crate) use input::*;
