//! The game built without the `plugins` feature: the plugins' panels never have the mouse.

use super::super::domain::plugins_off::PANELS_HAVE_MOUSE;

impl crate::core::app::App {
    /// Whether the plugins' panels have the mouse: never without plugins.
    #[inline]
    pub(crate) fn plugin_focus(&self) -> bool {
        PANELS_HAVE_MOUSE
    }

    /// Esc while the panels have the mouse: never without plugins.
    #[inline]
    pub(crate) fn release_plugin_focus(&mut self) -> bool {
        PANELS_HAVE_MOUSE
    }

    /// The left button while the panels have the mouse: never without plugins.
    #[inline]
    pub(crate) fn plugin_click(&mut self, _pressed: bool) {}
}
