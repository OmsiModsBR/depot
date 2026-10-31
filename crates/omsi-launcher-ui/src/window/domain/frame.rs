//! What a page is drawn with: the widgets, the launcher's state, the window's and the game.

use super::super::interface::output::{Host, State, Ui};
use super::root::RootCheck;

/// A page's frame: the window's widgets and the launcher's state, borrowed for one draw, and
/// what the window knows that a page shows.
pub struct Frame<'a> {
    pub ui: &'a mut Ui,
    pub state: &'a mut State,
    /// A file is dragged over the window (the Mods page lights its drop zone).
    pub drop_hover: bool,
    /// What only the game does (opening a web page, a phone's gallery).
    pub host: &'a dyn Host,
    /// Whether the OMSI 2 folder chosen has what the game needs.
    pub root: &'a mut RootCheck,
}
