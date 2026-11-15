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

/// Vanilla is drawn by Direct3D 9Ex on Windows, whatever the graphics API or VR say; the other
/// modes never are.
#[test]
fn vanilla_is_the_direct3d9ex_renderer_on_windows() {
    for spelling in ["vanilla", "classic", "omsi2"] {
        for extra in [json!({}), json!({"graphics_api": "vulkan", "vr": true})] {
            let mut s = extra;
            s["graphics"] = json!(spelling);
            assert_eq!(vanilla_on_d3d9(&s), cfg!(windows), "{s}");
        }
    }
    assert!(!vanilla_on_d3d9(&json!({"graphics": "vanilla_plus"})));
    assert!(!vanilla_on_d3d9(&json!({"graphics": "enhanced"})));
    assert!(!vanilla_on_d3d9(&json!({})));
}

/// A saved `graphics_api=d3d9` (any spelling) is read as `graphics=vanilla` with
/// `graphics_api=auto` on Windows, whatever `graphics` said; elsewhere the API is `auto` and
/// the mode stays. "DirectX9 (D3D9Ex)" is never written again (spec 001, settings-cfg).
#[test]
fn a_saved_directx9_api_is_read_as_vanilla() {
    use omsi_launcher_lib::{settings_from_text, settings_to_text};
    for api in ["d3d9", "D3D9Ex", "dx9"] {
        for graphics in ["vanilla_plus", "enhanced"] {
            let v = settings_from_text(Some(&format!("graphics={graphics}\ngraphics_api={api}\n")));
            assert_eq!(v["graphics_api"], "auto");
            let want = if cfg!(windows) { "vanilla" } else { graphics };
            assert_eq!(v["graphics"], want, "{api} {graphics}");
            let text = settings_to_text(&v, None);
            assert!(text.contains("graphics_api=auto\n"), "{text}");
            assert!(text.contains(&format!("graphics={want}\n")), "{text}");
        }
    }
    for api in ["auto", "vulkan", "dx12", "gl"] {
        let v = settings_from_text(Some(&format!("graphics=enhanced\ngraphics_api={api}\n")));
        assert_eq!((v["graphics"].as_str(), v["graphics_api"].as_str()), (Some("enhanced"), Some(api)));
    }
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
