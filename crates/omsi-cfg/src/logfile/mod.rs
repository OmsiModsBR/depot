//! What the log files of the game and of the launcher have in common: their size cap, the
//! level of what is written, as a whole and channel by channel, and the private names kept
//! out of them.

mod cap;
mod channels;
mod level;
mod redact;

pub use cap::{truncation_marker, Capped, CappedFile, LOG_CAP_BYTES};
pub use channels::{channel_names, ChannelLevels, LOG_CHANNELS};
pub use level::LogLevel;
pub use redact::{Redacting, Redactor, COMPUTER_NAME, USER_FOLDER, USER_NAME};
