//! What the launcher knows and has chosen, and the work it has running in the background
//! (reading the content lists and timetables, polling the running games and installs).
//!
//! Everything slow runs on a thread of its own and comes back as a [`Msg`]; the window
//! drains them at the start of each frame, so it never waits.
//!
//! `domain` holds the state, its messages, the Free up space card and the original OMSI 2
//! mode, `implementation` the work on them, `interface` what the window calls (`input`) and
//! what it reads from the rest of the workspace (`output`, with the game's `Host`).

pub(crate) mod domain;
pub(crate) mod implementation;
mod interface;

pub use interface::input::*;
