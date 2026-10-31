//! The core of the game: `App`, the window's game, and what makes it (the command line,
//! the start-up before a window, `make_app`), with the process's threads, log, memory and
//! platform, and Android's entry point.
//!
//! In three parts: `domain/` holds `App`'s state and the command line, `interface/` what the
//! rest of the game reaches of the core, and `implementation/` the procedures.

pub(crate) mod domain;
pub(crate) mod implementation;
mod interface;

pub(crate) use interface::*;
