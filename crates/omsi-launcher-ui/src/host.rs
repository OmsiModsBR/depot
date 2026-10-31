//! What the launcher asks of the game it runs in: the parts only the game has (its
//! translations, the log of a phone's previous run, opening a web page, a phone's gallery). The game implements `Host` and hands
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

    /// A page of the project's repository opened in the browser (the game opens no other).
    fn open_url(&self, url: &str);

    /// Microsoft's download page of the .NET 6 runtimes OmsiLaunch needs opened in the browser.
    fn open_dotnet_download(&self);

    /// A phone's gallery kept out of the content folder just chosen, before its media scanner
    /// lists the textures as photos. Nothing elsewhere.
    fn hide_content_from_gallery(&self) {}

    /// The game's version and build, as a report names them: `0.3.1 (abc1234)`.
    fn build(&self) -> String;

    /// The game's version alone, as Diagnostics shows it: `0.3.1`.
    fn version(&self) -> String;
}
