//! Where a picture goes: the window, a triple screen's panel, a headset's eye, or a texture
//! of the scene (a mirror, a screen).

use crate::scene::TextureId;
use glam::Mat4;

/// The destination of one picture (`Renderer::render_inner`).
#[derive(Clone, Copy)]
pub(crate) enum Output {
    /// The window's picture, with the HUD and the overlays.
    Window,
    /// One panel of a triple screen, with its own projection; `later` for every panel after
    /// the first, which take the first one's shadows.
    TriplePanel { projection: Mat4, later: bool },
    /// One eye of the headset; the second eye takes the first one's shadows.
    XrEye { projection: Mat4, second: bool },
    /// A texture of the scene (a mirror, a screen), leaving out `exclude`, the texture itself.
    Texture { exclude: TextureId },
}

impl Output {
    /// What the frame's stages read of it: the HUD and overlays are drawn (the window's
    /// pictures), the texture left out, the projection, and whether the shadows are the
    /// previous view's.
    pub(crate) fn flags(self) -> (bool, Option<TextureId>, Option<Mat4>, bool) {
        match self {
            Output::Window => (true, None, None, false),
            Output::TriplePanel { projection, later } => (true, None, Some(projection), later),
            Output::XrEye { projection, second } => (false, None, Some(projection), second),
            Output::Texture { exclude } => (false, Some(exclude), None, false),
        }
    }

    /// Whether this is the window's own finished picture (not a triple screen's panel, an eye
    /// or a texture): the only one whose frame calls the overlay hook (a triple screen calls
    /// it once, after its panels are in the window).
    pub(crate) fn is_window(self) -> bool {
        matches!(self, Output::Window)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_window_picture_gets_the_overlay_hook() {
        let panel = Output::TriplePanel {
            projection: Mat4::IDENTITY,
            later: false,
        };
        assert!(Output::Window.is_window());
        assert!(!panel.is_window());
        // (the panels draw the HUD and overlays too, which is why `flags` alone is not enough)
        assert!(panel.flags().0);
        assert!(!Output::XrEye {
            projection: Mat4::IDENTITY,
            second: false
        }
        .is_window());
        assert!(!Output::Texture {
            exclude: TextureId::default()
        }
        .is_window());
    }
}
