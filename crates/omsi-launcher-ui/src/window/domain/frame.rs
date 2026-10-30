//! What a page is drawn with: the widgets and the launcher's state.

use super::super::interface::output::{State, Ui};

/// A page's frame: the window's widgets and the launcher's state, borrowed for one draw.
pub struct Frame<'a> {
    pub ui: &'a mut Ui,
    pub state: &'a mut State,
}
