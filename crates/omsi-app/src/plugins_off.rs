//! The game built without the `plugins` feature: no plugin ever has the mouse.

impl crate::App {
    /// Whether the plugins' panels have the mouse: never without plugins.
    #[inline]
    pub(crate) fn plugin_focus(&self) -> bool {
        false
    }

    /// Esc while the panels have the mouse: never without plugins.
    #[inline]
    pub(crate) fn release_plugin_focus(&mut self) -> bool {
        false
    }

    /// The left button while the panels have the mouse: never without plugins.
    #[inline]
    pub(crate) fn plugin_click(&mut self, _pressed: bool) {}
}
