//! The graphics API chosen and what it fixes.

use super::super::interface::output::Value;

/// Whether the settings choose "DirectX9 (D3D9Ex)" (Windows only), which draws as OMSI 2 does:
/// Graphics is Vanilla there, and fixed. (VR takes DirectX 12 instead, as the game does.)
pub fn d3d9_fixes_vanilla(s: &Value) -> bool {
    // (the environment read last: the page asks this for every option, every frame)
    cfg!(windows)
        && s.get("graphics_api").and_then(Value::as_str) == Some("d3d9")
        && !s.get("vr").and_then(Value::as_bool).unwrap_or(false)
        && !omsi_cfg::flags::OPENXR.is_set()
}
