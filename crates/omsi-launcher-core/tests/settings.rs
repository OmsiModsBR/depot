//! The settings both the launcher and the game read (`omsi_launcher_lib::settings`).

use omsi_launcher_lib::settings::*;
use serde_json::json;

/// The window sizes start with the automatic one, and every other is "<width>x<height>".
#[test]
fn the_window_sizes_are_automatic_or_a_width_by_a_height() {
    assert_eq!(RESOLUTIONS[0].0, "auto");
    for (key, _) in &RESOLUTIONS[1..] {
        let (w, h) = key.split_once('x').unwrap();
        assert!(
            w.parse::<u32>().is_ok() && h.parse::<u32>().is_ok(),
            "{key}"
        );
    }
}

/// DirectX9 fixes the graphics only when it is chosen, without VR, and only on Windows.
#[test]
fn directx9_fixes_the_graphics_only_without_vr() {
    let d3d9 = json!({"graphics_api": "d3d9", "vr": false});
    assert_eq!(d3d9_fixes_vanilla(&d3d9), cfg!(windows));
    assert!(!d3d9_fixes_vanilla(
        &json!({"graphics_api": "d3d9", "vr": true})
    ));
    assert!(!d3d9_fixes_vanilla(&json!({"graphics_api": "auto"})));
    assert!(!d3d9_fixes_vanilla(&json!({})));
}

/// The `physics` key (spec 002, `contracts/settings-cfg.md`): a file without it is OMSI
/// Classic, every spelling of the contract reads as its mode, and what is saved is one of the
/// two canonical spellings.
#[test]
fn the_physics_setting_reads_and_is_saved_as_the_contract_says() {
    use omsi_launcher_lib::{settings_from_text, settings_to_text};
    assert_eq!(settings_from_text(None)["physics"], "classic");
    assert_eq!(settings_from_text(Some("msaa=4\n"))["physics"], "classic");
    for (saved, want) in [
        ("classic", "classic"),
        ("omsi", "classic"),
        ("omsi2", "classic"),
        ("OMSI_Classic", "classic"),
        ("depot", "depot"),
        ("Project-Depot", "depot"),
        ("nonsense", "classic"),
        ("", "classic"),
    ] {
        let v = settings_from_text(Some(&format!("physics={saved}\n")));
        assert_eq!(v["physics"], want, "physics={saved}");
    }
    for mode in ["classic", "depot"] {
        let v = json!({ "physics": mode });
        let text = settings_to_text(&v, None);
        assert!(
            text.lines().any(|l| l == format!("physics={mode}")),
            "{text}"
        );
        assert_eq!(settings_from_text(Some(&text))["physics"], mode);
    }
    // a value that is not a mode is not written as it is
    let text = settings_to_text(&json!({ "physics": "nonsense" }), None);
    assert!(text.lines().any(|l| l == "physics=classic"), "{text}");
}
