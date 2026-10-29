//! The ports of the graphics output: `input` is what the game calls and reads, `output` what
//! the output takes from the rest of the game.

mod input;
pub(in crate::output) mod output;

pub(crate) use input::*;
