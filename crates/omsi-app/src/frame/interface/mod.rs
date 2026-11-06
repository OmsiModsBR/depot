//! The ports of the frame: `input` is what the rest of the game reaches of it, `output` what
//! the frame takes from the rest of the game.

mod input;
pub(in crate::frame) mod output;

pub(crate) use input::*;
