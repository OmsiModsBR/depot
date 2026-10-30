//! The one place the graphics backend is chosen from the settings.

use super::super::domain::backend::Backend;
use crate::settings::Settings;

impl Backend {
    /// The backend `settings` choose: "DirectX9 (D3D9Ex)" for `graphics_api = d3d9` or
    /// OMSI_BACKEND=d3d9, on Windows only and not with VR (OpenXR takes DirectX 12); wgpu
    /// otherwise.
    pub(crate) fn of(settings: &Settings) -> Backend {
        let wanted = omsi_cfg::flags::BACKEND
            .text()
            .unwrap_or_else(|| settings.graphics_api.clone());
        if cfg!(windows) && is_d3d9(&wanted) && !settings.vr_requested() {
            Backend::D3d9
        } else {
            Backend::Wgpu
        }
    }

    /// The backend the settings on disk choose (for what is set up before the game has its
    /// own settings, the graphics instance and the window).
    pub(crate) fn wanted() -> Backend {
        if !cfg!(windows) {
            return Backend::Wgpu;
        }
        Backend::of(&Settings::load())
    }
}

/// Whether a `graphics_api` or OMSI_BACKEND value names "DirectX9 (D3D9Ex)".
pub(crate) fn is_d3d9(name: &str) -> bool {
    matches!(
        name.trim().to_ascii_lowercase().as_str(),
        "d3d9" | "d3d9ex" | "dx9"
    )
}
