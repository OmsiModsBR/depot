//! What the background work sends back to the window.

use super::super::interface::output::core;
use super::{original::*, squeeze::*};

/// Results of background work.
pub enum Msg {
    Content(
        Result<
            (
                Vec<core::MapInfo>,
                Vec<core::VehicleInfo>,
                Vec<core::WeatherInfo>,
            ),
            String,
        >,
    ),
    /// The first reading of the content: the maps and weathers (quick to read), before the
    /// buses.
    ContentEarly {
        maps: Vec<core::MapInfo>,
        weathers: Vec<core::WeatherInfo>,
    },
    /// The first reading of the content: the buses of the next few folders, how many
    /// folders are read and how many there are.
    VehiclesRead {
        batch: Vec<core::VehicleInfo>,
        done: usize,
        total: usize,
    },
    Lines {
        map: String,
        date: String,
        lines: Result<Vec<core::LineInfo>, String>,
    },
    Poll(Result<core::Poll, String>),
    Profile(Result<core::Profile, String>),
    Profiles(Vec<String>),
    Ibis {
        key: String,
        info: Result<core::IbisInfo, String>,
    },
    Args(Result<Vec<String>, String>),
    Launched(Result<core::Launched, String>),
    Stopped {
        pid: u32,
        result: Result<bool, String>,
    },
    LogTail {
        pid: u32,
        lines: Vec<String>,
    },
    Mods(Result<core::ModsStatus, String>),
    /// What the content takes on disk and how much of it there is (Setup), for the reading
    /// of that number.
    Usage(u64, core::disk::Usage),
    /// The folders of the Free up space card, read (the reading asked last counts).
    Squeeze(u64, std::sync::Arc<Vec<core::compress::Folder>>),
    /// A compression or an undo ended.
    SqueezeDone(Squeezed),
    ModInfo(Result<core::install::SourceInfo, String>),
    Installed(Result<core::install::Progress, String>),
    /// A job of the original OMSI 2 mode (`original.rs`).
    Original(Event),
    /// A background job stopped on an error of its own (a panic): whatever it was loading
    /// is not coming.
    Crashed(String),
}
