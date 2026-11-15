//! OMSI 2's rain reflections under "DirectX9 (D3D9Ex)" (spec 001, check V3): the puddles
//! mirror the street, the people in it too, unless the player switched them off, in the
//! game and in the launcher, and only under that API.

use omsi_launcher_lib::{settings_from_text, settings_to_text};
use openomsi_game::testing::settings::*;

#[test]
fn the_reflections_and_the_people_in_them_are_on_in_a_file_that_does_not_say() {
    for s in [Settings::default(), Settings::from_text("")] {
        assert!(s.rain_reflections && s.humans_in_rain_reflections);
    }
    let v = settings_from_text(Some("msaa=4\n"));
    assert_eq!(v["rain_reflections"], true);
    assert_eq!(v["humans_in_rain_reflections"], true);
}

#[test]
fn a_saved_choice_is_kept() {
    for key in ["rain_reflections", "humans_in_rain_reflections"] {
        for (saved, on) in [("0", false), ("1", true)] {
            let text = format!("{key}={saved}\n");
            let s = Settings::from_text(&text);
            let got = if key == "rain_reflections" {
                s.rain_reflections
            } else {
                s.humans_in_rain_reflections
            };
            assert_eq!(got, on, "{text}");
            let v = settings_from_text(Some(&text));
            assert_eq!(v[key], on);
            let written = settings_to_text(&v, Some(&text));
            assert!(written.lines().any(|l| l == text.trim_end()), "{written}");
        }
    }
}

#[test]
fn the_reflections_are_drawn_only_under_directx9_and_the_people_only_with_them() {
    // ("DirectX9 (D3D9Ex)" is Windows only: elsewhere the setting falls back to Auto)
    let d3d9 = Settings::from_text("graphics_api=d3d9\n");
    assert_eq!(d3d9.rain_reflection(), cfg!(windows).then_some(true));
    let no_people = Settings::from_text("graphics_api=d3d9\nhumans_in_rain_reflections=0\n");
    assert_eq!(no_people.rain_reflection(), cfg!(windows).then_some(false));
    let off = Settings::from_text("graphics_api=d3d9\nrain_reflections=0\n");
    assert_eq!(off.rain_reflection(), None);
    assert_eq!(
        Settings::from_text("graphics_api=vulkan\n").rain_reflection(),
        None
    );
}
