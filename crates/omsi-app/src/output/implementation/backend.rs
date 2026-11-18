//! The one place the graphics backend is chosen from the settings.

use super::super::domain::backend::Backend;
use crate::settings::{GraphicsMode, Settings};

impl Backend {
    /// The backend `settings` choose: the mode decides. Vanilla is drawn by the Direct3D 9Ex
    /// renderer on Windows; Vanilla+ and Enhanced, and Vanilla elsewhere, by wgpu.
    pub(crate) fn of(settings: &Settings) -> Backend {
        if cfg!(windows) && settings.graphics == GraphicsMode::Vanilla {
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
