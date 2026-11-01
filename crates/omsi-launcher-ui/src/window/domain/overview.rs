//! What the Diagnostics page's Overview tab shows that only the window knows.

/// The window's side of the Overview: the preview helper and the graphics adapter.
pub struct Overview<'a> {
    /// The preview helper is drawing.
    pub preview_live: bool,
    /// The graphics adapter the launcher draws with (empty: not known yet).
    pub adapter: &'a str,
}
