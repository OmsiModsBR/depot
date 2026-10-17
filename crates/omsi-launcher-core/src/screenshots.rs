//! The player's screenshots, by map.
//!
//! The game writes a screenshot as `<content>/Screenshots/omsi_<seconds>.png` (seconds since
//! the Unix epoch) and nothing in the file says which map it shows. A session summary,
//! though, is written when the map is left, with the map's folder name and the time it
//! ended (`~/.depot/sessions/<time>-<process>.json`). A screenshot therefore belongs to the
//! first session that ended after it was taken, if that session ended not more than
//! `MAX_SPAN` seconds later (a game that crashed writes no summary: its screenshots must not
//! be given to the next session on another map).

use crate::Session;
use std::path::{Path, PathBuf};

/// The longest a session is taken to have lasted, in seconds (twelve hours).
pub const MAX_SPAN: u64 = 12 * 3600;

/// A screenshot file and when it was taken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shot {
    pub path: PathBuf,
    pub time: u64,
}

/// The seconds in a game screenshot's file name (`omsi_1760000000.png`), if it is one.
pub fn time_of(name: &str) -> Option<u64> {
    let (stem, ext) = name.rsplit_once('.')?;
    if !ext.eq_ignore_ascii_case("png") && !ext.eq_ignore_ascii_case("jpg") {
        return None;
    }
    stem.strip_prefix("omsi_")?.parse().ok()
}

/// The game's screenshots in `dir`, oldest first.
pub fn list(dir: &Path) -> Vec<Shot> {
    let mut out: Vec<Shot> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let time = time_of(&e.file_name().to_string_lossy())?;
                    Some(Shot { path: e.path(), time })
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by_key(|s| s.time);
    out
}

/// The screenshots of `map` among `shots`, oldest first, given the sessions written so far.
pub fn of_map(shots: &[Shot], sessions: &[Session], map: &str) -> Vec<PathBuf> {
    let mut ends: Vec<(u64, &str)> = sessions.iter().map(|s| (s.time, s.map.as_str())).collect();
    ends.sort_by_key(|e| e.0);
    shots
        .iter()
        .filter(|shot| {
            let next = ends.iter().find(|(end, _)| *end >= shot.time);
            matches!(next, Some((end, m)) if end - shot.time <= MAX_SPAN && m.eq_ignore_ascii_case(map))
        })
        .map(|s| s.path.clone())
        .collect()
}

/// The screenshots of `map` in the content folder's `Screenshots`, oldest first (none when
/// there is no content folder or no session summary).
pub fn for_map(map: &str) -> Vec<PathBuf> {
    let Some(content) = crate::content_dir() else { return Vec::new() };
    let shots = list(&content.join("Screenshots"));
    if shots.is_empty() {
        return Vec::new();
    }
    of_map(&shots, &crate::sessions(), map)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(time: u64, map: &str) -> Session {
        Session {
            time,
            map: map.into(),
            ..Default::default()
        }
    }

    fn shot(time: u64) -> Shot {
        Shot {
            path: PathBuf::from(format!("omsi_{time}.png")),
            time,
        }
    }

    #[test]
    fn only_the_games_own_file_names_are_screenshots() {
        assert_eq!(time_of("omsi_1760000000.png"), Some(1760000000));
        assert_eq!(time_of("omsi_1760000000.PNG"), Some(1760000000));
        assert_eq!(time_of("omsi_1760000000.bmp"), None);
        assert_eq!(time_of("holiday.png"), None);
        assert_eq!(time_of("omsi_x.png"), None);
    }

    #[test]
    fn a_screenshot_belongs_to_the_session_that_ended_after_it() {
        let sessions = [session(1_000, "A"), session(5_000, "B")];
        let shots = [shot(900), shot(1_000), shot(1_001), shot(4_000)];
        assert_eq!(of_map(&shots, &sessions, "A").len(), 2);
        assert_eq!(of_map(&shots, &sessions, "b").len(), 2);
        assert!(of_map(&shots, &sessions, "C").is_empty());
    }

    #[test]
    fn a_screenshot_with_no_session_after_it_or_too_long_before_it_belongs_to_none() {
        let sessions = [session(1_000_000, "A")];
        let shots = [shot(1_000_000 - MAX_SPAN - 1), shot(1_000_001)];
        assert!(of_map(&shots, &sessions, "A").is_empty());
        let shots = [shot(1_000_000 - MAX_SPAN)];
        assert_eq!(of_map(&shots, &sessions, "A").len(), 1);
    }

    #[test]
    fn list_reads_the_folder_oldest_first() {
        let dir = std::env::temp_dir().join(format!("omsi_shots_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for n in ["omsi_30.png", "omsi_10.png", "notes.txt", "omsi_20.jpg"] {
            std::fs::write(dir.join(n), b"x").unwrap();
        }
        let times: Vec<u64> = list(&dir).iter().map(|s| s.time).collect();
        assert_eq!(times, vec![10, 20, 30]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
