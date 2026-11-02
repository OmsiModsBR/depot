//! The link between the game and the launcher's developer tools (#500).
//!
//! It carries the messages of `omsi-devtools-proto` over any pair of byte streams: a reader
//! and a writer. Which stream that is (a named pipe on Windows, a Unix socket elsewhere,
//! named by the launcher for one run) is decided by the caller, so this crate opens no
//! socket, depends on no operating-system API and has no effect unless someone hands it a
//! stream. The game's end never blocks the frame: snapshots go into a small queue that drops
//! the oldest when the launcher falls behind, and commands are read without waiting.

mod error;
mod game;
mod io;
mod launcher;

pub use error::LinkError;
pub use game::{GameLink, MAX_PENDING_CONTROL, MAX_PENDING_SNAPSHOTS};
pub use launcher::LauncherLink;

/// Where a link is. The game keeps running whatever it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkState {
    /// The hellos are not exchanged yet.
    Connecting,
    /// Both sides speak the same protocol version; messages flow.
    Ready,
    /// The other side speaks another protocol version, or did not start with a hello.
    Refused,
    /// The stream ended or failed.
    Closed,
}

pub(crate) mod state {
    use crate::LinkState;
    use std::sync::atomic::{AtomicU8, Ordering};

    pub(crate) struct Cell(AtomicU8);

    impl Cell {
        pub(crate) fn new() -> Self {
            Cell(AtomicU8::new(0))
        }
        pub(crate) fn get(&self) -> LinkState {
            match self.0.load(Ordering::Acquire) {
                0 => LinkState::Connecting,
                1 => LinkState::Ready,
                2 => LinkState::Refused,
                _ => LinkState::Closed,
            }
        }
        /// Moves out of `Connecting` only; a refused or closed link stays so.
        pub(crate) fn ready(&self) {
            let _ = self
                .0
                .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire);
        }
        pub(crate) fn refuse(&self) {
            let _ = self
                .0
                .compare_exchange(0, 2, Ordering::AcqRel, Ordering::Acquire);
        }
        pub(crate) fn close(&self) {
            self.0.store(3, Ordering::Release);
        }
    }
}
