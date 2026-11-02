//! The game controllers as the launcher's Controls page and the game share them: a device
//! connected now (`Connected`), how a name in `gamectrler.cfg` is matched to one, and the
//! force feedback direction test (`Calibration`).
//!
//! `domain` holds the connected device, `implementation` the name matching and the test,
//! `interface` what the window and the game call (`input`) and what it reads from the rest of
//! the workspace (`output`).

mod domain;
mod implementation;
mod interface;

pub use interface::input::*;
