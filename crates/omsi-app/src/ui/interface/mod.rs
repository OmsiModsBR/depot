//! The ports of the new interface: `input` is what the game calls and reads, `output` what
//! the screens take from the rest of the game.

mod input;
pub(in crate::ui) mod output;

pub use input::*;
