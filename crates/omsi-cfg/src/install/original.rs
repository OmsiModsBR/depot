//! What makes a folder an installation of the original OMSI 2.

use std::path::{Path, PathBuf};

use super::layout::{CONTENT_MARKER, LEGACY_CONTENT_MARKER};
use crate::content::resolve_existing;

/// What an installation of the original OMSI 2 must have for openOMSI to run on it: the
/// game itself and the stock content every player gets with it (both stock maps, the stock
/// buses, people, fonts, weather, inputs). Any copy of OMSI 2 has them, whatever version or
/// shop it came from; a folder that lacks them (openOMSI's own content folder, a mod pack,
/// a half-copied installation) is not a base to play on - everybody plays on the same
/// original content.
pub const ORIGINAL_ESSENTIALS: &[&str] = &[
    "Omsi.exe",
    "maps/Grundorf/global.cfg",
    "maps/Berlin-Spandau/global.cfg",
    "Vehicles/MAN_SD200",
    "Vehicles/MAN_SD202",
    "Vehicles/MAN_NL_NG",
    "Sceneryobjects",
    "Splines",
    "Texture",
    "Fonts",
    "Humans",
    "Weather",
    "Inputs",
    "envir.cfg",
];

/// The key assignment of the OMSI installation at `root`: its `Inputs/keyboard.cfg`, or
/// where it has none (it is the player's own, and not every copy comes with one) the
/// standard `keyboard_reset.cfg`, which OMSI loads then too.
pub fn original_keyboard_cfg(root: &Path) -> PathBuf {
    resolve_existing(root, &["Inputs", "keyboard.cfg"])
        .or_else(|| resolve_existing(root, &["Inputs", "keyboard_reset.cfg"]))
        .unwrap_or_else(|| root.join("Inputs").join("keyboard.cfg"))
}

/// The file `name` of the `Inputs` folder of the OMSI installation at `root`, whatever the case
/// of its letters (`keyboard_reset.cfg`, `gamectrler.cfg`, a language's `.kyb`).
pub fn original_inputs_file(root: &Path, name: &str) -> Option<PathBuf> {
    resolve_existing(root, &["Inputs", name])
}

/// The essentials (see [`ORIGINAL_ESSENTIALS`]) that `root` lacks; empty for a complete
/// original installation. openOMSI's content folder never counts as one.
pub fn missing_original_essentials(root: &Path) -> Vec<String> {
    // (a folder with the game's executable in it is the game's, even marked: openOMSI unpacked into the OMSI
    // folder made it its content folder once - see `content_folder_of` - and every start after
    // that said the game was not there)
    let marked = root.join(CONTENT_MARKER).exists() || root.join(LEGACY_CONTENT_MARKER).exists();
    if marked && resolve_existing(root, &["Omsi.exe"]).is_none() {
        return vec![format!(
            "{} (this is the Project Depot content folder, not the original game)",
            root.display()
        )];
    }
    ORIGINAL_ESSENTIALS
        .iter()
        .filter(|rel| {
            let comps: Vec<&str> = rel.split('/').collect();
            // case-insensitive, as Windows reads the folder
            resolve_existing(root, &comps).is_none()
        })
        .map(|s| s.to_string())
        .collect()
}
