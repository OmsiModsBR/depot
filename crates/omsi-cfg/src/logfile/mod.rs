//! What the log files of the game and of the launcher have in common: their size cap, the
//! level of what is written, and the private names kept out of them.

mod cap;
mod level;
mod redact;

pub use cap::{truncation_marker, Capped, CappedFile, LOG_CAP_BYTES};
pub use level::{LogLevel, GAME_LOG_TARGETS};
pub use redact::{Redacting, Redactor, COMPUTER_NAME, USER_FOLDER, USER_NAME};
