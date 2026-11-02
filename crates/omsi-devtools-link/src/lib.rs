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
pub use game::{
    GameLink, Lost, MAX_EVENTS_PER_SNAPSHOT, MAX_PENDING_CONTROL, MAX_PENDING_SNAPSHOTS,
};
pub use launcher::LauncherLink;
pub use launcher::MAX_PENDING_FROM_GAME;

/// Where a link is. The game keeps running whatever it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkState {
    /// The hellos are not exchanged yet.
    Connecting,
    /// Both sides speak the same protocol version; messages flow.
    Ready,
    /// The other side speaks another protocol version, or its first message was not a hello.
    Refused,
    /// The stream ended or failed, or this end was dropped.
    Closed,
}

pub(crate) mod state {
    use crate::LinkState;
    use std::sync::{Condvar, Mutex};
    use std::time::Duration;

    /// The link's state, with a wait for the end of `Connecting`.
    pub(crate) struct Cell(Mutex<LinkState>, Condvar);

    impl Cell {
        pub(crate) fn new() -> Self {
            Cell(Mutex::new(LinkState::Connecting), Condvar::new())
        }
        pub(crate) fn get(&self) -> LinkState {
            *self.0.lock().unwrap()
        }
        fn move_from(&self, from: &[LinkState], to: LinkState) {
            let mut s = self.0.lock().unwrap();
            if from.contains(&*s) {
                *s = to;
            }
            drop(s);
            self.1.notify_all();
        }
        /// `Connecting` to `Ready` only.
        pub(crate) fn ready(&self) {
            self.move_from(&[LinkState::Connecting], LinkState::Ready);
        }
        /// `Connecting` to `Refused` only.
        pub(crate) fn refuse(&self) {
            self.move_from(&[LinkState::Connecting], LinkState::Refused);
        }
        /// Ends the link, whatever it was, except a refused one, which stays refused.
        pub(crate) fn close(&self) {
            self.move_from(
                &[LinkState::Connecting, LinkState::Ready],
                LinkState::Closed,
            );
        }
        /// Waits (in slices, so a caller can look at its own flags) until the link has left
        /// `Connecting`; returns the state then.
        pub(crate) fn wait_connected(&self, slice: Duration) -> LinkState {
            let mut s = self.0.lock().unwrap();
            while *s == LinkState::Connecting {
                let (g, t) = self.1.wait_timeout(s, slice).unwrap();
                s = g;
                if t.timed_out() {
                    break;
                }
            }
            *s
        }
    }
}
