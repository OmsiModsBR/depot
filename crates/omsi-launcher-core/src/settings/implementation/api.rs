//! The graphics mode chosen and the renderer that draws it.

use super::super::interface::output::Value;
use omsi_cfg::GraphicsMode;

/// Whether the settings choose Vanilla on Windows, which the Direct3D 9Ex renderer draws as
/// OMSI 2 does: only the options OMSI 2 has, and no VR or triple screen.
pub fn vanilla_on_d3d9(s: &Value) -> bool {
    cfg!(windows)
        && GraphicsMode::parse(s.get("graphics").and_then(Value::as_str).unwrap_or(""))
            == GraphicsMode::Vanilla
}
