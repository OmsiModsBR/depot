//! What the launcher asks of the game it runs in: the parts only the game has (its
//! translations, the log of a phone's previous run). The game implements `Host` and hands
//! it to the launcher; the tests hand a quiet one.

/// The game's side of the launcher.
pub trait Host {
    /// The interface language set to `code` (OMSI 2's three letters, as the settings keep it).
    fn set_language(&self, code: &str);

    /// When the app's previous run closed while a drive was going (a phone runs the game in
    /// the launcher's process): what it said last and the end of its log, for the crash
    /// dialog. None elsewhere.
    fn previous_run_crash(&self) -> Option<(String, String)> {
        None
    }
}
