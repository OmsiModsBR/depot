//! File names of screenshots: `omsi_<secs>__<map>__<vehicle>.png`.
//!
//! The map and the vehicle are folder names of the content, made safe for Windows and Linux
//! file systems. Older captures (`omsi_<secs>.png`) stay readable: [`parse`] gives them no
//! map and no vehicle. A deliberate improvement; the pictures used to carry only the time.

/// Longest a map or vehicle part gets, in characters.
pub const MAX_PART: usize = 48;
const SEP: &str = "__";

/// A folder name made safe inside a file name: no path separators, reserved characters or
/// controls, no runs of `__` (the separator), no leading/trailing dots or spaces, at most
/// [`MAX_PART`] characters, and never a Windows device name. Empty when nothing is left.
pub fn sanitize_part(raw: &str) -> String {
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
pub fn build(secs: u64, map: &str, vehicle: Option<&str>) -> String {
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
pub fn parse(file_name: &str) -> Option<(u64, Option<String>, Option<String>)> {
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
pub fn folder_of(path: &str) -> Option<String> {
    let p = path.replace('\\', "/");
    let mut it = p.rsplit('/').filter(|s| !s.is_empty());
    let file = it.next()?;
    Some(it.next().unwrap_or(file).to_string())
}

/// The on-screen notice for a saved screenshot. It is raised only after the capture, so the
/// path never ends up inside the picture itself.
pub fn notice(path: &std::path::Path) -> (String, f32) {
    (format!("Screenshot: {}", path.display()), 4.0)
}

/// Whether an on-screen message is a screenshot notice (saved, or failed to save).
pub fn is_notice(msg: &str) -> bool {
    msg.starts_with("Screenshot: ") || msg.starts_with("Screenshot could not be saved")
}
