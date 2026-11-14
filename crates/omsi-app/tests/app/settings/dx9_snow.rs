//! The falling snow of "DirectX9 (D3D9Ex)" (spec 001, check V13): on unless the player
//! switched it off, in the game and in the launcher, and only under that API.

use omsi_launcher_lib::{settings_from_text, settings_to_text};
use openomsi_game::testing::settings::*;

#[test]
fn the_snow_is_on_in_a_file_that_does_not_say() {
    assert!(Settings::default().dx9_snow);
    assert!(Settings::from_text("").dx9_snow);
    assert!(Settings::from_text("graphics_api=d3d9\n").dx9_snow);
    assert_eq!(settings_from_text(None)["dx9_snow"], true);
    assert_eq!(settings_from_text(Some("msaa=4\n"))["dx9_snow"], true);
}

#[test]
fn a_saved_choice_is_kept() {
    for (saved, on) in [("0", false), ("1", true)] {
        let text = format!("dx9_snow={saved}\n");
        assert_eq!(Settings::from_text(&text).dx9_snow, on);
        let v = settings_from_text(Some(&text));
        assert_eq!(v["dx9_snow"], on);
        let written = settings_to_text(&v, Some(&text));
        assert!(written.lines().any(|l| l == text.trim_end()), "{written}");
    }
}

#[test]
fn a_launcher_save_of_a_file_that_does_not_say_leaves_the_snow_on() {
    let written = settings_to_text(&settings_from_text(Some("msaa=4\n")), Some("msaa=4\n"));
    assert!(Settings::from_text(&written).dx9_snow, "{written}");
}

#[test]
fn the_snow_falls_only_under_directx9() {
    // ("DirectX9 (D3D9Ex)" is Windows only: elsewhere the setting falls back to Auto)
    let d3d9 = Settings::from_text("graphics_api=d3d9\n");
    assert_eq!(d3d9.dx9_snowfall(), cfg!(windows));
    assert!(!Settings::from_text("graphics_api=d3d9\ndx9_snow=0\n").dx9_snowfall());
    assert!(!Settings::from_text("graphics_api=vulkan\n").dx9_snowfall());
}
