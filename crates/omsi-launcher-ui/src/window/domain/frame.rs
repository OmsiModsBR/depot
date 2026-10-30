//! What a page is drawn with: the widgets, the launcher's state and the window's.

use super::super::interface::output::{State, Ui};

/// A page's frame: the window's widgets and the launcher's state, borrowed for one draw, and
/// what the window knows that a page shows.
pub struct Frame<'a> {
    pub ui: &'a mut Ui,
    pub state: &'a mut State,
    /// A file is dragged over the window (the Mods page lights its drop zone).
    pub drop_hover: bool,
}
