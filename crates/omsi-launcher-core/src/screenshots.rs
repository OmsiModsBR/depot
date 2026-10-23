//! The player's screenshots, by map.
//!
//! The game writes a screenshot as `<content>/Screenshots/omsi_<seconds>__<map>__<vehicle>.png`
//! (seconds since the Unix epoch; the map and vehicle are folder names made safe for file
//! names, the vehicle only when the player is in one; the contract of #258). Older captures,
//! `omsi_<seconds>.png`, say nothing about the map: for those only, a session summary
//! decides. It is written when the map is left, with the map's folder name and the time it
//! ended (`~/.depot/sessions/<time>-<process>.json`), so such a screenshot belongs to the
//! first session that ended after it, if that session ended not more than `MAX_SPAN` seconds
//! later (a game that crashed writes no summary: its screenshots must not be given to the
//! next session on another map).
//!
//! `parse` and `safe_part` copy the game's own naming functions (`screenshot_name` in
//! `omsi-app`, #258) because this crate does not depend on the game; keep them in step.

use crate::Session;
use std::path::{Path, PathBuf};

/// The longest a session is taken to have lasted, in seconds (twelve hours).
pub const MAX_SPAN: u64 = 12 * 3600;

/// A screenshot file and when it was taken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shot {
    pub path: PathBuf,
    pub time: u64,
    /// The map in the file name (already made safe), if it has one.
    pub map: Option<String>,
}

const SEP: &str = "__";
const MAX_PART: usize = 48;

/// A folder name as the game writes it inside a file name.
pub fn safe_part(raw: &str) -> String {
    let mut out = String::new();
    let mut last_us = false;
    for c in raw.trim().chars() {
        let bad = c.is_control()
            || matches!(
                c,
                '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' | '_'
            );
        let c = if bad || c.is_whitespace() { '_' } else { c };
        if c == '_' {
            if last_us {
                continue;
            }
            last_us = true;
        } else {
            last_us = false;
        }
        out.push(c);
    }
    let out: String = out.chars().take(MAX_PART).collect();
    let out = out
        .trim_matches(|c| c == '_' || c == '.' || c == ' ')
        .to_string();
    let stem = out.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && stem.as_bytes()[3].is_ascii_digit());
    if reserved {
        format!("{out}_")
    } else {
        out
    }
}

/// The seconds and the map (if any) in a game screenshot's file name
/// (`omsi_1760000000.png`, `omsi_1760000000__Grundorf__MB_O305.png`), if it is one.
pub fn parse(name: &str) -> Option<(u64, Option<String>)> {
    let (stem, ext) = name.rsplit_once('.')?;
    if !ext.eq_ignore_ascii_case("png") && !ext.eq_ignore_ascii_case("jpg") {
        return None;
    }
    let mut parts = stem.strip_prefix("omsi_")?.split(SEP);
    let secs = parts.next()?.parse().ok()?;
    let map = parts.next().filter(|s| !s.is_empty()).map(str::to_string);
    Some((secs, map))
}

/// The game's screenshots in `dir`, oldest first.
pub fn list(dir: &Path) -> Vec<Shot> {
    let mut out: Vec<Shot> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let (time, map) = parse(&e.file_name().to_string_lossy())?;
                    Some(Shot {
                        path: e.path(),
                        time,
                        map,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by_key(|s| s.time);
    out
}

/// The map's folder name in a session summary's `map`, which is the path the game was started
/// with (`maps/Grundorf/global.cfg`); a bare folder name stays as it is.
fn folder_of(map: &str) -> &str {
    let p = map.trim_end_matches(['/', '\\']);
    let mut it = p.rsplit(['/', '\\']).filter(|s| !s.is_empty());
    let last = it.next().unwrap_or("");
    if last.to_ascii_lowercase().ends_with(".cfg") {
        it.next().unwrap_or(last)
    } else {
        last
    }
}

/// The screenshots of `map` among `shots`, oldest first. A screenshot named after a map
/// belongs to that map; one with no map in its name belongs to the session that ended
/// after it (see the module notes).
pub fn of_map(shots: &[Shot], sessions: &[Session], map: &str) -> Vec<PathBuf> {
    let mut ends: Vec<(u64, &str)> = sessions.iter().map(|s| (s.time, s.map.as_str())).collect();
    ends.sort_by_key(|e| e.0);
    let wanted = safe_part(map);
    shots
        .iter()
        .filter(|shot| match &shot.map {
            Some(m) => m.eq_ignore_ascii_case(&wanted),
            None => {
                let next = ends.iter().find(|(end, _)| *end >= shot.time);
                matches!(next, Some((end, m)) if end - shot.time <= MAX_SPAN && folder_of(m).eq_ignore_ascii_case(map))
            }
        })
        .map(|s| s.path.clone())
        .collect()
}

/// The screenshots of `map` in the content folder's `Screenshots`, oldest first (none when
/// there is no content folder or no session summary).
pub fn for_map(map: &str) -> Vec<PathBuf> {
    let Some(content) = crate::content_dir() else {
        return Vec::new();
    };
    let shots = list(&content.join("Screenshots"));
    if shots.is_empty() {
        return Vec::new();
    }
    of_map(&shots, &crate::sessions(), map)
}

/// The player's latest screenshot in the content folder's `Screenshots`, of any map.
pub fn latest() -> Option<PathBuf> {
    let content = crate::content_dir()?;
    list(&content.join("Screenshots")).pop().map(|s| s.path)
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
            map: None,
        }
    }

    #[test]
    fn only_the_games_own_file_names_are_screenshots() {
        assert_eq!(parse("omsi_1760000000.png"), Some((1760000000, None)));
        assert_eq!(parse("omsi_1760000000.PNG"), Some((1760000000, None)));
        assert_eq!(parse("omsi_1760000000.bmp"), None);
        assert_eq!(parse("holiday.png"), None);
        assert_eq!(parse("omsi_x.png"), None);
    }

    #[test]
    fn a_session_map_given_as_a_path_is_matched_by_its_folder() {
        assert_eq!(folder_of("maps/Grundorf/global.cfg"), "Grundorf");
        assert_eq!(folder_of("maps\\Grundorf\\Global.cfg"), "Grundorf");
        assert_eq!(folder_of("Grundorf"), "Grundorf");
        let sessions = [session(11, "maps/Grundorf/global.cfg")];
        assert_eq!(of_map(&[shot(10)], &sessions, "grundorf").len(), 1);
        assert!(of_map(&[shot(10)], &sessions, "Berlin").is_empty());
    }

    #[test]
    fn the_map_is_read_from_the_file_name() {
        assert_eq!(
            parse("omsi_5__Grundorf__MB_O305.png"),
            Some((5, Some("Grundorf".into())))
        );
        assert_eq!(
            parse("omsi_5__Berlin.png"),
            Some((5, Some("Berlin".into())))
        );
    }

    #[test]
    fn a_named_screenshot_needs_no_session_and_an_unnamed_one_still_uses_it() {
        let named = Shot {
            path: PathBuf::from("omsi_9__Berlin_Mitte.png"),
            time: 9,
            map: Some("Berlin_Mitte".into()),
        };
        let old = shot(10);
        let sessions = [session(11, "Berlin Mitte")];
        assert_eq!(of_map(&[named.clone()], &[], "Berlin Mitte").len(), 1);
        assert!(of_map(&[named], &sessions, "Other").is_empty());
        assert_eq!(of_map(&[old], &sessions, "Berlin Mitte").len(), 1);
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
