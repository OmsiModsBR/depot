//! The level of the log: how much of what the program says is written.

/// The game's own crates as the log names them: what `debug` is switched on for. The
/// graphics libraries are left at `info`, their debug lines being theirs to read.
pub const GAME_LOG_TARGETS: &[&str] = &[
    "openomsi_game",
    "omsi_audio",
    "omsi_cfg",
    "omsi_content",
    "omsi_geometry",
    "omsi_html",
    "omsi_map",
    "omsi_model",
    "omsi_net",
    "omsi_o3d",
    "omsi_plugin",
    "omsi_render",
    "omsi_render_d3d9",
    "omsi_scenery",
    "omsi_script",
    "omsi_sim",
    "omsi_texture",
    "omsi_timetable",
    "omsi_ui",
    "omsi_vehicle",
];

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

    /// The logger's filter for this level: `error`, `warn` and `info` for every part of the
    /// program; `debug` for the game's own crates over `info` for the rest.
    pub fn filter(self) -> String {
        match self {
            Self::Debug => {
                let mut filter = String::from("info");
                for target in GAME_LOG_TARGETS {
                    filter.push_str(&format!(",{target}=debug"));
                }
                filter
            }
            level => level.name().to_string(),
        }
    }
}
