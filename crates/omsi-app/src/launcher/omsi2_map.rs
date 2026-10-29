//! What the Settings and Controls pages show while the original OMSI 2 is the engine: only
//! what OMSI 2 has. Anything else is the Depot's own and stays out of sight until the Depot
//! engine is chosen again (nothing is deleted, the Depot's files are not touched).

use omsi_launcher_lib::original_engine::catalog;

/// Launcher settings that mean the same in both modes: how the launcher itself looks and
/// speaks. (The whole Interface section counts too.)
const LAUNCHER_OWN: &[&str] = &[
    "language",
    "ui_scale",
    "ui_scale_window",
    "ui_opacity",
    "tooltips",
    "launcher_rest",
];

/// Depot settings with a setting of the same meaning and the same kind of value in the
/// catalog that do not carry a `depot` link there.
const TWINS: &[(&str, &str)] = &[
    ("red_steer_spd", "controls.reducedSteeringSpeed"),
    ("auto_clutch", "simulation.automaticClutch"),
    ("collision_pedestrians", "simulation.collisionPedestrians"),
];

/// The OmsiLaunch key of the Depot setting `key`, when OMSI 2 has the setting.
pub fn catalog_key(key: &str) -> Option<&'static str> {
    catalog::depot_map()
        .into_iter()
        .find(|(d, _)| *d == key)
        .map(|(_, k)| k)
        .or_else(|| TWINS.iter().find(|(d, _)| *d == key).map(|(_, k)| *k))
}

/// Whether the setting `key` (of section `interface`, or not) is shown in OMSI 2 mode.
pub fn setting_in_omsi2(key: &str, interface: bool) -> bool {
    interface || LAUNCHER_OWN.contains(&key) || catalog_key(key).is_some()
}

/// Whether the key action `action` exists in OMSI 2. Only the actions the Depot added are
/// named here; a bus's own triggers and everything else stay.
pub fn action_in_omsi2(action: &str) -> bool {
    let a = action.to_ascii_lowercase();
    !(a.starts_with("vr_")
        || a.starts_with("chat_")
        || a == "voice_radio"
        || a == "doors_all"
        || a == "view_toggle_interior"
        || matches!(a.as_str(), "door_1" | "door_2" | "door_3" | "door_4"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_twin_is_a_catalog_key() {
        for (d, k) in TWINS {
            assert!(catalog::entry(k).is_some(), "{d} -> {k}");
        }
    }

    #[test]
    fn the_catalog_links_are_shown() {
        for (d, _) in catalog::depot_map() {
            assert!(setting_in_omsi2(d, false), "{d}");
        }
        assert!(!setting_in_omsi2("vr", false));
        assert!(setting_in_omsi2("ui_accent", true));
        assert!(setting_in_omsi2("language", false));
    }

    #[test]
    fn depot_only_actions_are_left_out() {
        assert!(!action_in_omsi2("vr_recenter"));
        assert!(!action_in_omsi2("chat_open"));
        assert!(!action_in_omsi2("voice_radio"));
        assert!(!action_in_omsi2("doors_all"));
        assert!(action_in_omsi2("throttle"));
        assert!(action_in_omsi2("kw_something_toggle"));
    }
}
