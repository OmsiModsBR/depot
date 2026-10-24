//! File names of screenshots: `omsi_<secs>__<map>__<vehicle>.png`.
//!
//! The map and the vehicle are folder names of the content, made safe for Windows and Linux
//! file systems. Older captures (`omsi_<secs>.png`) stay readable: [`parse`] gives them no
//! map and no vehicle. A deliberate improvement; the pictures used to carry only the time.

/// Longest a map or vehicle part gets, in characters.
const MAX_PART: usize = 48;
const SEP: &str = "__";

/// A folder name made safe inside a file name: no path separators, reserved characters or
/// controls, no runs of `__` (the separator), no leading/trailing dots or spaces, at most
/// [`MAX_PART`] characters, and never a Windows device name. Empty when nothing is left.
pub(crate) fn sanitize_part(raw: &str) -> String {
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

/// The name of a capture taken at `secs` (Unix time) on `map`, in `vehicle` if the player
/// is in one. Parts that are empty after sanitising are left out.
pub(crate) fn build(secs: u64, map: &str, vehicle: Option<&str>) -> String {
    let mut name = format!("omsi_{secs}");
    for part in [Some(map), vehicle].into_iter().flatten() {
        let p = sanitize_part(part);
        if !p.is_empty() {
            name.push_str(SEP);
            name.push_str(&p);
        }
    }
    name.push_str(".png");
    name
}

/// What a capture's file name says: (secs, map, vehicle). `None` when it is not one of ours.
/// Old names give a map of `None`; a name with one part gives only the map.
pub(crate) fn parse(file_name: &str) -> Option<(u64, Option<String>, Option<String>)> {
    let stem = file_name
        .strip_suffix(".png")
        .or_else(|| file_name.strip_suffix(".PNG"))?;
    let rest = stem.strip_prefix("omsi_")?;
    let mut parts = rest.split(SEP);
    let secs = parts.next()?.parse().ok()?;
    let map = parts.next().filter(|s| !s.is_empty()).map(str::to_string);
    let vehicle = parts.next().filter(|s| !s.is_empty()).map(str::to_string);
    Some((secs, map, vehicle))
}

/// The last folder of a content path (`maps/Grundorf/Global.cfg` -> `Grundorf`).
pub(crate) fn folder_of(path: &str) -> Option<String> {
    let p = path.replace('\\', "/");
    let mut it = p.rsplit('/').filter(|s| !s.is_empty());
    let file = it.next()?;
    Some(it.next().unwrap_or(file).to_string())
}

/// The on-screen notice for a saved screenshot. It is raised only after the capture, so the
/// path never ends up inside the picture itself.
pub(crate) fn notice(path: &std::path::Path) -> (String, f32) {
    (format!("Screenshot: {}", path.display()), 4.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_notice_names_the_file_and_lasts_a_few_seconds() {
        let (text, secs) = notice(std::path::Path::new("Screenshots/a.png"));
        assert_eq!(text, "Screenshot: Screenshots/a.png");
        assert!(secs > 0.0);
    }

    #[test]
    fn map_and_vehicle_follow_the_time() {
        assert_eq!(
            build(1760000000, "Grundorf", Some("MB O305")),
            "omsi_1760000000__Grundorf__MB_O305.png"
        );
    }

    #[test]
    fn on_foot_or_free_camera_names_only_the_map() {
        assert_eq!(build(5, "Berlin", None), "omsi_5__Berlin.png");
    }

    #[test]
    fn an_unusable_part_is_left_out() {
        assert_eq!(build(5, "???", Some("Bus")), "omsi_5__Bus.png");
        assert_eq!(build(5, "", None), "omsi_5.png");
    }

    #[test]
    fn unsafe_characters_never_reach_the_file_name() {
        let n = build(1, "a/b\\c:d*e?\"f<g>h|i", Some("../../x\u{7}y"));
        assert!(
            !n.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|', '\u{7}']),
            "{n}"
        );
        assert_eq!(sanitize_part("..hidden. "), "hidden");
        assert_eq!(sanitize_part("a   b__c"), "a_b_c");
    }

    #[test]
    fn windows_device_names_are_defused() {
        assert_eq!(sanitize_part("CON"), "CON_");
        assert_eq!(sanitize_part("lpt1"), "lpt1_");
        assert_eq!(sanitize_part("Console"), "Console");
    }

    #[test]
    fn long_parts_are_cut_by_characters_not_bytes() {
        let long = "é".repeat(200);
        assert_eq!(sanitize_part(&long).chars().count(), MAX_PART);
        assert!(build(1, &long, Some(&long)).len() < 255);
    }

    #[test]
    fn what_is_built_is_read_back() {
        let n = build(42, "Grundorf", Some("MB O305"));
        assert_eq!(
            parse(&n),
            Some((42, Some("Grundorf".into()), Some("MB_O305".into())))
        );
        assert_eq!(
            parse("omsi_42__Grundorf.png"),
            Some((42, Some("Grundorf".into()), None))
        );
    }

    #[test]
    fn old_captures_are_still_read() {
        assert_eq!(parse("omsi_1760000000.png"), Some((1760000000, None, None)));
        assert_eq!(parse("holiday.png"), None);
        assert_eq!(parse("omsi_x.png"), None);
    }

    #[test]
    fn folder_names_come_from_content_paths() {
        assert_eq!(
            folder_of("maps/Grundorf/Global.cfg").as_deref(),
            Some("Grundorf")
        );
        assert_eq!(
            folder_of("Vehicles\\MB_O305\\MB_O305.bus").as_deref(),
            Some("MB_O305")
        );
        assert_eq!(folder_of("single.cfg").as_deref(), Some("single.cfg"));
    }
}
