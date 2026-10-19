//! The user's data folder (`~/.depot`) and the files the game and the launcher keep in it.
//!
//! The home folder is `HOME`, or `USERPROFILE` where there is none (Windows); an unusable
//! `HOME` is dropped at start-up (see [`crate::drop_unusable_home`]). Every function gives
//! `None` when neither is set.

use std::path::PathBuf;

/// The user's home folder: `HOME`, else `USERPROFILE`.
pub fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// The data folder of the game and the launcher (`~/.depot`).
pub fn data_dir() -> Option<PathBuf> {
    home().map(|h| h.join(".depot"))
}

/// The file that remembers the last OMSI 2 installation that worked (`~/.depot-root`).
pub fn root_memo() -> Option<PathBuf> {
    home().map(|h| h.join(".depot-root"))
}

/// The settings shared by the game and the launcher (`~/.depot/settings.cfg`).
pub fn settings() -> Option<PathBuf> {
    data_dir().map(|d| d.join("settings.cfg"))
}

/// The key assignment kept when no content folder takes it (`~/.depot/Inputs`).
pub fn inputs() -> Option<PathBuf> {
    data_dir().map(|d| d.join("Inputs"))
}

/// The mirror positions per bus (`~/.depot/mirrors.cfg`).
pub fn mirrors() -> Option<PathBuf> {
    data_dir().map(|d| d.join("mirrors.cfg"))
}

/// The driver's seat per bus (`~/.depot/seats.cfg`).
pub fn seats() -> Option<PathBuf> {
    data_dir().map(|d| d.join("seats.cfg"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_lie_in_the_data_dir() {
        let Some(home) = home() else {
            return;
        };
        let data = home.join(".depot");
        assert_eq!(data_dir(), Some(data.clone()));
        assert_eq!(root_memo(), Some(home.join(".depot-root")));
        assert_eq!(settings(), Some(data.join("settings.cfg")));
        assert_eq!(inputs(), Some(data.join("Inputs")));
        assert_eq!(mirrors(), Some(data.join("mirrors.cfg")));
        assert_eq!(seats(), Some(data.join("seats.cfg")));
    }
}
