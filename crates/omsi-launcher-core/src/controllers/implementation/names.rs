//! Which connected device a name in `gamectrler.cfg` means.

use super::super::domain::device::Connected;

/// OMSI stores DirectInput's product name; the system's may differ in spacing and case.
pub fn normalized_device_name(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

pub fn names_match(a: &str, b: &str) -> bool {
    // (letters of any script: a name of Cyrillic or Chinese letters only was empty here and
    // matched nothing)
    let (a, b) = (normalized_device_name(a), normalized_device_name(b));
    !a.is_empty() && (a == b || a.contains(&b) || b.contains(&a))
}

/// The connected device this name is set up as - its exact name first: an Xbox-type pad's
/// gilrs lists too, as a plain "Controller" with no axis or button at all (see
/// `mac_game_controller`), and that name is a substring of (so fuzzy-matches) this device's.
pub fn find_connected<'a>(connected: &'a [Connected], name: &str) -> Option<&'a Connected> {
    connected
        .iter()
        .find(|c| c.name == name)
        .or_else(|| connected.iter().find(|c| names_match(&c.name, name)))
}
