//! What the launcher knows and has chosen.

use super::super::interface::output::core;
use super::{msg::*, original::*, squeeze::*};
use std::sync::mpsc::{Receiver, Sender};
use std::time::Instant;

/// The duty as it is remembered between launches (`omsi_launcher_lib::start`).
pub use core::start::Choice;

pub struct State {
    pub config: core::Config,
    pub maps: Vec<core::MapInfo>,
    pub vehicles: Vec<core::VehicleInfo>,
    pub weathers: Vec<core::WeatherInfo>,
    pub lines: Vec<core::LineInfo>,
    pub lines_for: (String, String),
    pub loading_content: bool,
    /// The lists are filled while they are read (the first reading: nothing to show yet).
    pub content_first: bool,
    /// The content changed while it was being read: read it again when that is done.
    pub reload_content: bool,
    pub loading_lines: bool,
    pub choice: Choice,
    pub choice_dirty: f32,
    /// Map, whether it has a `laststn.osn`, when that was looked up.
    pub last_sit: Option<(String, Vec<core::SavedSituation>, std::time::Instant)>,
    /// Which of them "Continue" starts (0: the newest, the last situation when there is one).
    pub save_pick: usize,
    pub profiles: Vec<String>,
    pub profile: Option<core::Profile>,
    pub settings: serde_json::Value,
    pub settings_dirty: f32,
    /// `settings.cfg` as last read or written here: a game changes it too (its Options
    /// in the pause menu), and the launcher's copy from before must not be written back
    /// over that.
    pub(crate) settings_file: Option<String>,
    pub keybindings: serde_json::Value,
    pub keybindings_error: String,
    pub instances: Vec<core::Instance>,
    pub queued_launch: Option<core::Duty>,
    /// Start was pressed: the graphics device stays given up until the list of games has the
    /// game started (its process, once it is known), 15 s at most.
    pub launch_hold: Option<std::time::Instant>,
    pub(crate) launched_pid: Option<u32>,
    /// The last game started from this launcher (its process), until the launcher has put
    /// itself away for it: a game that already ran when the launcher opened is not one.
    pub started_here: Option<u32>,
    /// A game started from here ended on an error: what it said, and the end of its log
    /// (see `crash_of`), for the dialog that asks to report it.
    pub crash: Option<(String, String)>,
    /// A game started from here was sent away by its server (kicked, banned) or turned away
    /// at the door: the server's message, for the "Disconnected from the server" dialog.
    pub disconnected: Option<String>,
    pub jobs: Vec<core::install::Progress>,
    pub mods: Option<core::ModsStatus>,
    pub mods_asked: bool,
    /// What the content takes on disk (Setup), read when the page opens; the reading asked
    /// last (an earlier one that ends after it is not taken).
    pub usage: Option<core::disk::Usage>,
    pub usage_asked: bool,
    pub(crate) usage_gen: u64,
    /// The Free up space card's folders, choices and the job running.
    pub squeeze: Squeeze,
    /// The original OMSI 2 mode: what is installed and the install flow.
    pub original: Original,
    /// Counts the readings of the content that brought other files than before.
    pub content_gen: u64,
    pub mod_info: Option<Result<core::install::SourceInfo, String>>,
    pub mod_path: String,
    pub ibis: Option<(String, Result<core::IbisInfo, String>)>,
    pub cmdline: String,
    pub join: (bool, String),
    pub logs: std::collections::HashMap<u32, Vec<String>>,
    pub open_logs: std::collections::HashSet<u32>,
    pub stopping: std::collections::HashSet<u32>,
    /// Content that appeared while the launcher was open (marked NEW for a while).
    pub fresh: std::collections::HashMap<String, Instant>,
    pub status: (String, bool, Instant),
    pub(crate) stamp: Option<String>,
    pub(crate) poll_t: f32,
    pub(crate) polling: bool,
    /// Multiplayer: the server the Drive page is joined to now (its address).
    pub joined_server: Option<String>,
    pub(crate) tx: Sender<Msg>,
    pub(crate) rx: Receiver<Msg>,
}
