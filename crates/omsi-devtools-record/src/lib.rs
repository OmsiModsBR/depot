//! The run record of the developer tools (#500).
//!
//! While developer mode watches a game, the launcher can keep what it receives as a file to
//! open later: one JSON object a line (`.jsonl`), one file per session next to `game.log`.
//! This crate turns the messages of `omsi-devtools-proto` into those lines, stops a file at
//! its cap with a last line that says so, and keeps the last five sessions.
//!
//! It only writes what it is handed: it opens no connection, and nothing calls it unless the
//! launcher runs with developer mode on. The lines:
//!
//! ```text
//! {"t":"run","protocol":1,"build":"0.3.0","started":"2026-11-22T10:15:00Z"}
//! {"t":"snap","frame":5120,"sim_s":85.3,"vars":{"throttle_in":0.8},"phys":{...}}
//! {"t":"ev","frame":5118,"kind":"macro","name":"M_door_open"}
//! {"t":"cmd","frame":5121,"cmd":{"set":"throttle_in","value":1.0}}
//! {"t":"truncated"}
//! ```
//!
//! A record with a `cmd` line is of a run the tool changed: such a run is no 1:1 comparison
//! run.

mod line;
mod sessions;
mod writer;

pub use line::TRUNCATED_LINE;
pub use sessions::{prune, records_in, start_in, RECORD_EXTENSION, RECORD_PREFIX, SESSIONS_KEPT};
pub use writer::{RunRecord, RECORD_CAP_BYTES};
