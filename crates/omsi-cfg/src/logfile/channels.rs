//! The channels of the log: the parts of the program whose lines can be given a level of
//! their own (`--log-channels physics=debug,render=warn`).

use super::level::LogLevel;

/// The channels and the log targets (crates, and modules inside a crate) each one covers.
/// A line belongs to the channel with the longest target that starts its own, so a module
/// listed here is taken out of its crate's channel. `app` holds what no other channel does.
pub const LOG_CHANNELS: &[(&str, &[&str])] = &[
    ("script", &["omsi_script"]),
    (
        "content",
        &[
            "omsi_cfg",
            "omsi_content",
            "omsi_geometry",
            "omsi_html",
            "omsi_map",
            "omsi_model",
            "omsi_o3d",
            "omsi_scenery",
            "omsi_texture",
            "omsi_timetable",
            "omsi_vehicle",
            "openomsi_game::world",
        ],
    ),
    ("audio", &["omsi_audio", "openomsi_game::radio"]),
    (
        "physics",
        &[
            "omsi_sim::collision",
            "omsi_sim::physics",
            "omsi_sim::vehicle",
            "omsi_trolley",
        ],
    ),
    (
        "ai",
        &[
            "omsi_sim::ai",
            "omsi_sim::people",
            "omsi_sim::traffic",
            "openomsi_game::gameplay::implementation::humans",
            "openomsi_game::gameplay::implementation::traffic",
            "openomsi_game::gameplay::implementation::traffic_link",
        ],
    ),
    (
        "network",
        &["omsi_devtools_link", "omsi_net", "openomsi_game::steam"],
    ),
    (
        "render",
        &["omsi_render", "omsi_render_vanilla", "openomsi_game::output"],
    ),
    ("input", &["omsi_sim::input", "openomsi_game::input"]),
    (
        "app",
        &[
            "omsi_integrity",
            "omsi_launcher_lib",
            "omsi_launcher_ui",
            "omsi_plugin",
            "omsi_sim",
            "omsi_ui",
            "openomsi_game",
        ],
    ),
];

/// The levels asked for channel by channel, and what of the request was not understood.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ChannelLevels {
    /// A channel of [`LOG_CHANNELS`] and its level; a channel named twice keeps the last.
    pub levels: Vec<(&'static str, LogLevel)>,
    /// The entries left out, each with why (for a warning in the log).
    pub rejected: Vec<String>,
}

impl ChannelLevels {
    /// What `spec` asks for: `channel=level` entries separated by commas (spaces around
    /// them and letter case ignored, empty entries passed over). An entry whose channel or
    /// level is not known is left out and reported; the others still count.
    pub fn parse(spec: &str) -> Self {
        let mut out = Self::default();
        for entry in spec.split(',').map(str::trim).filter(|e| !e.is_empty()) {
            let Some((name, level)) = entry.split_once('=') else {
                out.rejected
                    .push(format!("\"{entry}\" is not channel=level"));
                continue;
            };
            let name = name.trim().to_ascii_lowercase();
            let Some((channel, _)) = LOG_CHANNELS.iter().find(|(c, _)| *c == name) else {
                out.rejected.push(format!(
                    "\"{entry}\": no channel \"{name}\" (the channels are {})",
                    channel_names()
                ));
                continue;
            };
            let Some(level) = LogLevel::parse(level) else {
                out.rejected.push(format!(
                    "\"{entry}\": \"{}\" is none of error, warn, info, debug",
                    level.trim()
                ));
                continue;
            };
            out.levels.retain(|(c, _)| c != channel);
            out.levels.push((channel, level));
        }
        out
    }

    /// The level of `channel` when it was given one.
    pub fn level_of(&self, channel: &str) -> Option<LogLevel> {
        self.levels
            .iter()
            .find(|(c, _)| *c == channel)
            .map(|(_, l)| *l)
    }

    /// The logger's filter for the level `global` with these channels at their own: every
    /// target of every channel at its channel's level, or at `global` when the channel was
    /// given none; everything else (the graphics libraries) at `global`, but never past
    /// `info`.
    pub fn filter(&self, global: LogLevel) -> String {
        let rest = match global {
            LogLevel::Debug => LogLevel::Info,
            level => level,
        };
        let mut filter = String::from(rest.name());
        for (channel, targets) in LOG_CHANNELS {
            let level = self.level_of(channel).unwrap_or(global);
            for target in *targets {
                filter.push_str(&format!(",{target}={}", level.name()));
            }
        }
        filter
    }
}

/// The channels' names, as a sentence lists them.
pub fn channel_names() -> String {
    LOG_CHANNELS
        .iter()
        .map(|(c, _)| *c)
        .collect::<Vec<_>>()
        .join(", ")
}
