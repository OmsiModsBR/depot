//! What the log files of the game and of the launcher have in common: their size cap.

mod cap;

pub use cap::{truncation_marker, Capped, CappedFile, LOG_CAP_BYTES};
