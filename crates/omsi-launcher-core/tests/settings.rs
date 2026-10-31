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
