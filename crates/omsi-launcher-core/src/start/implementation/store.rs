//! Where a start's choices are kept: the duty of the next start (`launcher-duty.json`) and the
//! starred buses (`favourite-buses.txt`), both in the launcher's data folder.

use super::super::domain::choice::Choice;
use super::super::interface::output::data_dir;
use std::collections::BTreeSet;

fn choice_path() -> std::path::PathBuf {
    data_dir().join("launcher-duty.json")
}

impl Choice {
    /// The duty remembered from the last launch (the default when there is none), as a new
    /// launcher starts with it (see `Choice::fresh`).
    pub fn load(mobile: bool) -> Choice {
        let c: Choice = std::fs::read_to_string(choice_path())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        Self::fresh(c, mobile)
    }

    pub fn save(&self) {
        if let Ok(t) = serde_json::to_string_pretty(self) {
            let _ = omsi_cfg::write_atomic(choice_path(), t);
        }
    }
}

fn favourites_file() -> std::path::PathBuf {
    data_dir().join("favourite-buses.txt")
}

/// How a bus file is kept in the starred buses: lower case, with '/'.
pub fn favourite_key(file: &str) -> String {
    file.replace('\\', "/").to_lowercase()
}

/// The starred buses of `text` (one bus file a line).
pub fn parse_favourites(text: &str) -> BTreeSet<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(favourite_key)
        .collect()
}

/// The starred buses (#524), from the launcher's data folder.
pub fn read_favourites() -> BTreeSet<String> {
    std::fs::read_to_string(favourites_file())
        .map(|t| parse_favourites(&t))
        .unwrap_or_default()
}

/// The starred buses written back, one a line.
pub fn write_favourites(f: &BTreeSet<String>) -> std::io::Result<()> {
    let text: String = f.iter().map(|l| format!("{l}\n")).collect();
    std::fs::write(favourites_file(), text)
}
