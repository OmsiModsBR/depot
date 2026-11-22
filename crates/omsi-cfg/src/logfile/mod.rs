//! What the log files of the game and of the launcher have in common: their size cap.

mod cap;

pub use cap::{is_last_word, truncation_marker, Capped, CappedFile, LAST_WORDS, LOG_CAP_BYTES};
