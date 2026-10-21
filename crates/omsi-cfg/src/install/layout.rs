//! openOMSI's content folder and its layout.

use std::path::{Path, PathBuf};

use crate::content::resolve_existing;

/// The top-level content folders openOMSI recognises. Most use OMSI 2's original spelling;
/// `HOFs` is an openOMSI extension for depot files shared by every vehicle. A content
/// folder is laid out with these names, so the mod installer and mounted archives can merge
/// them into the same virtual installation.
pub const CONTENT_FOLDERS: &[&str] = &[
    "Vehicles",
    "HOFs",
    "maps",
    "Sceneryobjects",
    "Splines",
    "Texture",
    "Fonts",
    "Plugins",
    "TicketPacks",
    "Drivers",
    "Weather",
    "Announcements",
    "Humans",
    "Money",
    "Scripts",
    "Trains",
    "Situations",
    "Inputs",
    "Sounds",
];

/// Marker file of an openOMSI content folder (so it is never mistaken for the OMSI 2
/// installation itself).
pub const CONTENT_MARKER: &str = ".depot-content";
/// The marker of a content folder made before the project was called openOMSI.
pub const LEGACY_CONTENT_MARKER: &str = ".omsi-rewrite-content";

/// openOMSI's content folder for a program in `dir`: that folder - unless it is the original
/// OMSI 2 folder itself (openOMSI unpacked into it), which openOMSI never writes to: then the
/// `openOMSI` folder inside it.
pub fn content_folder_of(dir: &Path) -> PathBuf {
    if resolve_existing(dir, &["Omsi.exe"]).is_some() && resolve_existing(dir, &["maps"]).is_some()
    {
        dir.join("depot")
    } else {
        dir.to_path_buf()
    }
}

/// A folder of programs - macOS's `/Applications` or `~/Applications`, where an
/// openOMSI.app is usually copied to - is no place for the content folder: the game laid
/// OMSI's folders (Vehicles, maps, Mods ...) out among the user's applications (#1043). The
/// content folder then lives in the user's data folder instead - unless content was already
/// installed there by an older version, which then stays where the player put it.
pub fn is_programs_folder(dir: &Path) -> bool {
    dir.file_name()
        .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("Applications"))
        && !holds_content(dir)
}

/// Whether `dir` holds installed OMSI content: a vehicle or scenery folder with something in
/// it, a map (a folder with its global.cfg - the game itself leaves only a `laststn.osn`
/// there), or a mod beside the `Mods` folder's own README.
fn holds_content(dir: &Path) -> bool {
    let map = resolve_existing(dir, &["maps"])
        .and_then(|d| std::fs::read_dir(d).ok())
        .is_some_and(|mut it| {
            it.any(|e| e.is_ok_and(|e| resolve_existing(&e.path(), &["global.cfg"]).is_some()))
        });
    let filled = |name: &str, skip: &[&str]| {
        resolve_existing(dir, &[name])
            .and_then(|d| std::fs::read_dir(d).ok())
            .is_some_and(|mut it| {
                it.any(|e| {
                    e.is_ok_and(|e| {
                        let n = e.file_name().to_string_lossy().to_string();
                        !n.starts_with('.') && !skip.iter().any(|s| n.eq_ignore_ascii_case(s))
                    })
                })
            })
    };
    map || ["Vehicles", "Sceneryobjects", "Splines"]
        .iter()
        .any(|n| filled(n, &[]))
        || filled("Mods", &["README.txt"])
}

/// Check if a directory is writable by attempting to create and remove a probe file.
pub fn is_writable(dir: &Path) -> bool {
    let probe = dir.join(".depot-write-test");
    let ok = std::fs::write(&probe, b"x").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

/// The content folder's marker, as written by versions that installed loose folders anywhere.
const MARKER_OLD: &str = "This folder holds Project Depot's own content and installed mods, laid out like the original game.\nDrop a mod folder or zip into Mods/ and the launcher sorts it into place.\n";
/// The inbox's README, as written by versions that installed loose folders anywhere.
const README_OLD: &str = "Put a mod here (a folder or a .zip) and start the launcher: it works out what the mod is\n(a bus, a map, scenery objects, splines, textures, fonts ...) and sorts it into the folders\nnext to this one. The original OMSI 2 folder is never written to.\n";

/// The marker: the launcher installs a .zip only, loose folders are copied by hand.
const MARKER_TEXT: &str = "This folder holds Project Depot's own content and installed mods, laid out like the original game.\nDrop a mod's .zip into Mods/ and the launcher installs it; loose folders are copied here by hand.\n";
/// The inbox's README: a .zip only, loose folders copied by hand.
const README_TEXT: &str = "Put a mod's .zip here and start the launcher: it works out what the mod is (a bus, a map,\nscenery objects, splines, textures, fonts ...) and installs it next to this folder, read in\nplace and never unpacked. The original OMSI 2 folder is never written to.\n\nLoose folders (not a .zip) are not installed from here. Copy them yourself: into the original\nOMSI 2 folder (the mod then works in OMSI 2 and in the Depot), or into the folders next to\nthis one (the Depot only). The Mods page has a button that opens each folder.\n";

/// Create the content folder layout at `dir` (idempotent).
pub fn ensure_content_layout(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    // (versions up to 0.2.9 made it `Sound`; OMSI 2's folder is `Sounds`)
    let (old, new) = (dir.join("Sound"), dir.join("Sounds"));
    if old.is_dir() && !new.exists() {
        let _ = std::fs::rename(&old, &new);
    } else if old.is_dir() {
        let _ = std::fs::remove_dir(&old); // only when empty
    }
    for f in CONTENT_FOLDERS {
        std::fs::create_dir_all(dir.join(f))?;
    }
    std::fs::create_dir_all(dir.join("Mods"))?;
    // (rewritten when it still holds what an older version wrote: that promised folders on a
    // computer; a file the player changed stays as it is)
    let write = |path: &Path, text: &str, older: &[&str]| -> std::io::Result<()> {
        let now = std::fs::read_to_string(path).ok();
        if now
            .as_deref()
            .is_none_or(|t| older.contains(&t) && t != text)
        {
            std::fs::write(path, text)?;
        }
        Ok(())
    };
    write(&dir.join(CONTENT_MARKER), MARKER_TEXT, &[MARKER_OLD])?;
    write(
        &dir.join("Mods").join("README.txt"),
        README_TEXT,
        &[README_OLD],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writable_check() {
        let tmp = std::env::temp_dir();
        assert!(is_writable(&tmp));
        let nonexistent = tmp.join("nonexistent_subfolder_xyz_123");
        assert!(!is_writable(&nonexistent));
    }

    #[test]
    fn applications_folder_is_no_content_folder() {
        assert!(is_programs_folder(Path::new("/Users/x/Applications")));
        assert!(!is_programs_folder(Path::new(
            "/Users/x/Games/Project Depot"
        )));
        // content an older version installed there stays in use
        let dir = std::env::temp_dir()
            .join(format!("omsi-apps-{}", std::process::id()))
            .join("Applications");
        std::fs::create_dir_all(dir.join("Mods")).unwrap();
        std::fs::create_dir_all(dir.join("Vehicles")).unwrap();
        std::fs::create_dir_all(dir.join("maps").join("Grundorf")).unwrap();
        std::fs::write(dir.join("Mods").join("README.txt"), b"x").unwrap();
        std::fs::write(dir.join("maps").join("Grundorf").join("laststn.osn"), b"x").unwrap();
        assert!(is_programs_folder(&dir));
        std::fs::create_dir_all(dir.join("Vehicles").join("MAN_SD200")).unwrap();
        assert!(!is_programs_folder(&dir));
        std::fs::remove_dir_all(dir.parent().unwrap()).unwrap();
    }

    /// The inbox's README is written once, and an older version's (which promised folder
    /// installs) is brought up to date; one the player wrote stays.
    #[test]
    fn an_older_readme_is_rewritten_a_players_kept() {
        let dir = std::env::temp_dir().join(format!("omsi-readme-{}", std::process::id()));
        let readme = dir.join("Mods").join("README.txt");
        std::fs::create_dir_all(dir.join("Mods")).unwrap();
        std::fs::write(&readme, README_OLD).unwrap();
        ensure_content_layout(&dir).unwrap();
        let first = std::fs::read_to_string(&readme).unwrap();
        std::fs::write(&readme, "my notes").unwrap();
        ensure_content_layout(&dir).unwrap();
        let kept = std::fs::read_to_string(&readme).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(first, README_TEXT);
        assert_eq!(kept, "my notes");
        assert!(!README_TEXT.contains("a folder or a .zip"));
    }
}
