//! What the log files of the game and of the launcher have in common: their size cap and
//! the level of what is written.

mod cap;
mod level;

pub use cap::{truncation_marker, Capped, CappedFile, LOG_CAP_BYTES};
pub use level::{LogLevel, GAME_LOG_TARGETS};
