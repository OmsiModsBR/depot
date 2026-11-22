//! The level of the log: how much of what the program says is written.

use super::channels::ChannelLevels;

/// One of the four levels of the log, the quietest first. A level writes its own lines and
/// those of every level before it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LogLevel {
    /// Errors only.
    Error,
    /// Errors and warnings.
    Warn,
    /// What the program does, its warnings and errors: the usual log.
    #[default]
    Info,
    /// Also every file the game loads and every failure, from the game's own crates.
    Debug,
}

impl LogLevel {
    /// The four levels, the quietest first.
    pub const ALL: [LogLevel; 4] = [
        LogLevel::Error,
        LogLevel::Warn,
        LogLevel::Info,
        LogLevel::Debug,
    ];

    /// The level `text` names (any letter case, spaces around it ignored); None for
    /// anything else.
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "error" => Some(Self::Error),
            "warn" | "warning" => Some(Self::Warn),
            "info" => Some(Self::Info),
            "debug" => Some(Self::Debug),
            _ => None,
        }
    }

    /// The level's name as the command line and the settings file write it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
        }
    }

    /// The logger's filter for this level with no channel at a level of its own: `error`,
    /// `warn` and `info` for every part of the program; `debug` for the game's own crates
    /// over `info` for the rest (the graphics libraries, their debug lines being theirs to
    /// read).
    pub fn filter(self) -> String {
        match self {
            Self::Debug => ChannelLevels::default().filter(self),
            level => level.name().to_string(),
        }
    }
}
