//! What a card of Settings > Modules (OMSI 2 mode) asks of the launcher.

/// What a module's card asks of the launcher (it needs the launcher's state and dialog).
#[derive(Default)]
pub struct Asks {
    /// The module's install ("Download and install…").
    pub look: Option<&'static str>,
    /// The module's removal ("Remove…").
    pub remove: Option<&'static str>,
    /// (in) Whether an install may start now: no game runs and no other install is open.
    pub idle: bool,
}
