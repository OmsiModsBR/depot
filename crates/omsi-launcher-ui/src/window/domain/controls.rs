//! What the Controls page keeps between frames besides its tabs' own (`KeyboardView`,
//! `PadsView`): the tab shown, and on a phone the keyboard list's filters and the "Add event…"
//! browser.

#[derive(Default)]
pub struct ControlsView {
    /// The tab: 0 the keyboard, 1 the game controllers.
    pub tab: usize,
    /// A phone's keyboard list: the filter of each section.
    pub(crate) filter: [String; 2],
    /// A phone's keyboard list: the OMSI-style "Add event…" browser is open for a section.
    pub(crate) events: [bool; 2],
}
