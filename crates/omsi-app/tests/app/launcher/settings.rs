//! The game reads the settings file the Settings page writes (`omsi_launcher_ui::window`): what
//! the page switches comes out of the game's own parser as the page meant it.

use omsi_launcher_lib as core;
use serde_json::json;

/// Developer mode changes nothing the game's own settings parser reads (#500).
#[test]
fn developer_mode_is_off_by_default_and_the_switch_is_saved() {
    let mut s = core::settings_from_text(None);
    let off = core::settings_to_text(&s, None);
    s["developer_mode"] = json!(true);
    let on = core::settings_to_text(&s, None);
    assert!(on.contains("developer_mode=1\n"), "{on}");
    let game_off = openomsi_game::testing::settings::Settings::from_text(&off);
    let game_on = openomsi_game::testing::settings::Settings::from_text(&on);
    assert_eq!(format!("{game_off:?}"), format!("{game_on:?}"));
}

/// The mouse steering's smoothing switched off on the page is off in the game (#1092).
#[test]
fn smooth_mouse_steering_switches_off_and_is_saved() {
    let mut s = core::settings_from_text(None);
    assert!(
        openomsi_game::testing::settings::Settings::from_text(&core::settings_to_text(&s, None))
            .mouse_smooth
    );
    s["mouse_smooth"] = json!(false);
    let saved = core::settings_to_text(&s, None);
    assert!(!openomsi_game::testing::settings::Settings::from_text(&saved).mouse_smooth);
}
