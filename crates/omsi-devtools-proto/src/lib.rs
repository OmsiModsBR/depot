//! Wire protocol of the developer tools.
//!
//! The game and the launcher exchange two kinds of message over a local socket that the
//! launcher owns: *snapshots* (game to launcher, read-only copies of simulation state) and
//! *commands* (launcher to game, applied by the game at the start of a frame). This crate
//! holds only the types, the handshake and the framing, so both sides depend on it and
//! neither depends on `omsi-app`. It opens no socket and has no effect on the simulation.

mod frame;
mod message;
mod validate;

pub use frame::{decode, encode, FrameError, MAX_FRAME_LEN};
pub use message::{
    Command, Event, EventKind, Hello, PhysicsSnapshot, Snapshot, ToGame, ToLauncher, VarValue,
    WheelSnapshot, PROTOCOL_VERSION,
};
pub use validate::{
    check_hello, validate_command, CommandError, HelloError, MAX_SUBSCRIPTION, MAX_TEXT,
};
