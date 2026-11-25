//! Whether the plugins' panels can have the mouse. In a game built without the `plugins`
//! feature there are no plugins, so the panels never have it.

/// The plugins' panels have the mouse: never, without plugins.
pub(crate) const PANELS_HAVE_MOUSE: bool = false;
