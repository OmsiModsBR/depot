//! The small helpers the pages share: times, sizes and names shown, a game's log read for a
//! crash, whether the launcher is laid out for fingers.

use super::super::interface::output::core;

/// Whether the game started from here (its process `pid`) is in a list of games.
pub fn game_listed(pid: Option<u32>, instances: &[core::Instance]) -> bool {
    pid.is_some_and(|p| instances.iter().any(|i| i.pid == p))
}

pub fn hhmm(seconds: f64) -> String {
    let s = seconds.max(0.0).round() as i64;
    format!("{:02}:{:02}", (s / 3600) % 24, (s % 3600) / 60)
}

pub use core::start::date_in_month;

pub fn fmt_bytes(b: u64) -> String {
    let b = b as f64;
    if b >= (1u64 << 30) as f64 {
        format!("{:.1} GB", b / (1u64 << 30) as f64)
    } else if b >= (1u64 << 20) as f64 {
        format!("{:.0} MB", b / (1u64 << 20) as f64)
    } else {
        format!("{:.0} KB", (b / 1024.0).max(1.0))
    }
}

pub fn short_map(m: &str) -> String {
    m.trim_start_matches("maps/")
        .trim_end_matches("/global.cfg")
        .to_string()
}

/// Whether `path` is a .zip file (what the Mods page installs): `core::is_zip`.
pub fn is_zip(path: &str) -> bool {
    core::is_zip(std::path::Path::new(path))
}

pub use core::start::without_bus;

/// Why `root` is not an OMSI 2 to play on, said so that the player knows what to choose.
pub fn root_problem(root: &str) -> String {
    let root = root.trim();
    let p = std::path::Path::new(root);
    let missing = omsi_cfg::missing_original_essentials(p);
    if root.is_empty() {
        "The original OMSI 2 was not found automatically: choose its folder (the one with Omsi.exe, maps and Vehicles in it) under Setup and press Save.".to_string()
    } else if !p.exists() {
        format!("{root} does not exist: choose the folder of the original OMSI 2 (with Omsi.exe, maps and Vehicles in it) under Setup.")
    } else if missing.iter().any(|m| m.contains("content folder"))
        || p.join("depot.exe").exists()
        || p.join("depot").is_file()
    {
        format!("{root} is Project Depot's own folder, not OMSI 2's: choose the folder of the original game (with Omsi.exe in it) under Setup.")
    } else if missing.is_empty() {
        String::new()
    } else {
        format!("{root} is not a complete OMSI 2 - it lacks {}. Project Depot plays on the original's stock content: choose the folder of a complete installation under Setup.", missing.iter().take(3).cloned().collect::<Vec<_>>().join(", "))
    }
}

/// Without LAN play no server sends a game away.
pub fn disconnect_of(_log: &std::path::Path) -> Option<String> {
    None
}

/// What a game's log says when the game ended on an error: the error (a panic, "no graphics
/// adapter", a fatal message) and the last lines of the log. None for a game that ended as
/// it should.
pub fn crash_of(log: &std::path::Path) -> Option<(String, String)> {
    let bytes = std::fs::read(log).ok()?;
    let text = String::from_utf8_lossy(&bytes[bytes.len().saturating_sub(64 * 1024)..]).to_string();
    let all: Vec<&str> = text.lines().collect();
    // (the run itself only: an error the launcher logged before the game started - a
    // preview's picture left out - titled the report of a game that died much later, and
    // the phone's "died compiling a shader" hint never showed, #381, #331)
    let lines: Vec<&str> = match all.iter().rposition(|l| l.contains("starting the game:")) {
        Some(k) => all[k..].to_vec(),
        None => all.clone(),
    };
    // (an error the game got over - "the game goes on", a part of the picture left out -
    // is no crash)
    let recovered = |l: &str| {
        l.contains("the game goes on")
            || l.contains("left out")
            || l.contains("could not be recorded")
    };
    // (a lost graphics device ends the game in order - it saves the run - but it is a crash
    // for the player all the same: the driver gave up)
    // (one the game got over by starting again with safer graphics is no crash)
    if lines
        .iter()
        .any(|l| l.contains("starting again with safer graphics"))
    {
        return None;
    }
    let lost = lines
        .iter()
        .rposition(|l| l.contains("the graphics device was lost"));
    if lost.is_none() && lines.iter().any(|l| l.contains("game ends")) {
        return None;
    }
    let at = lost.or_else(|| {
        lines.iter().rposition(|l| {
            l.contains("the game stopped on an error") || (l.contains(" ERROR ") && !recovered(l))
        })
    })?;
    let first = lines[at]
        .split_once("] ")
        .map(|x| x.1)
        .unwrap_or(lines[at])
        .trim();
    // (a panic's message is on the following lines)
    let mut what = first.to_string();
    // (and stops at the next record: a lost device's line had "game ends" and the tile
    // loading after it in the report's title, #1187)
    for l in lines.iter().skip(at + 1).take(6) {
        if l.trim().is_empty() || l.trim_start().starts_with("0:") || l.starts_with('[') {
            break;
        }
        what.push(' ');
        what.push_str(l.trim());
    }
    // the computer, its graphics card and the command line (the map) from the start of the
    // run, before the end of the log and a line `…`: a report of the end alone never said
    // what it happened on (#1187). Its GitHub link keeps them and shortens only the end.
    let whole = String::from_utf8_lossy(&bytes);
    let machine: Vec<&str> = MACHINE_LINES
        .iter()
        .filter_map(|k| whole.lines().rfind(|l| l.contains(k)))
        .collect();
    let end = all[all.len().saturating_sub(150)..].join("\n");
    let tail = if machine.is_empty() {
        end
    } else {
        format!("{}\n{CRASH_TAIL_GAP}\n{end}", machine.join("\n"))
    };
    Some((what.chars().take(600).collect(), tail))
}

/// The log lines that tell what a game ran on (see `crash_of`).
const MACHINE_LINES: [&str; 4] = [
    "] system: ",
    "] graphics adapter: ",
    "] opening graphics device: ",
    "] command line: ",
];

/// The line between the machine and the end of the log in a crash's tail.
pub const CRASH_TAIL_GAP: &str = "…";

/// Whether the launcher is laid out for fingers.
pub fn mobile() -> bool {
    cfg!(target_os = "android") || omsi_cfg::flags::MOBILE.is_set()
}

/// `2024-06-14` as `14/06/2024`.
pub fn date_shown(iso: &str) -> String {
    let (y, m, d) = omsi_ui::widgets::parse_date(iso);
    format!("{d:02}/{m:02}/{y:04}")
}

/// A tour's days as the core puts them ("Mon-Fri & holidays, school days"), in the
/// player's language, piece by piece.
pub fn days_text(d: &str) -> String {
    let (head, school) = match (
        d.strip_suffix(", school holidays"),
        d.strip_suffix(", school days"),
    ) {
        (Some(h), _) => (h, Some("school holidays")),
        (_, Some(h)) => (h, Some("school days")),
        _ => (d, None),
    };
    let (week, holidays) = match head.strip_suffix(" & holidays") {
        Some(w) => (w, true),
        None if head == "holidays" => ("", true),
        None => (head, false),
    };
    let mut out = week
        .split(", ")
        .filter(|x| !x.is_empty())
        .map(|x| omsi_ui::tr(x).into_owned())
        .collect::<Vec<_>>()
        .join(", ");
    if holidays {
        out = if out.is_empty() {
            omsi_ui::tr("holidays").into_owned()
        } else {
            format!("{out} & {}", omsi_ui::tr("holidays"))
        };
    }
    if let Some(x) = school {
        out = format!("{out}, {}", omsi_ui::tr(x));
    }
    out
}
