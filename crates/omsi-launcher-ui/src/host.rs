//! What the launcher asks of the game it runs in: the parts only the game has (its
//! translations, the log of a phone's previous run, opening a web page, a phone's gallery,
//! its key names and controllers file). The game implements `Host` and hands it to the
//! launcher; the tests hand a quiet one.

use omsi_content::input::gamectrler::DeviceCfg;

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

    /// The game's version and build (`0.3.1`, `abc1234`), as Diagnostics and a report name them.
    fn version(&self) -> (&'static str, &'static str);

    /// The project's repository page (where a problem is reported).
    fn repo_url(&self) -> &'static str;

    /// Where the preview helper writes its log.
    fn helper_log(&self) -> std::path::PathBuf;

    /// An action's name as the Controls page shows it, in the language `lang` (OMSI 2's
    /// three letters) of the OMSI 2 folder `root`.
    fn action_text(&self, root: &str, lang: &str, action: &str) -> String;

    /// The actions the game knows (its language files' and the mods' event tables) and their
    /// names, as `action_text` names them.
    fn actions(&self, root: &str, lang: &str) -> Vec<(String, String)>;

    /// A key and its Shift / Ctrl / Alt as the game names them ("Ctrl+D"), for a key the
    /// language's own table does not name.
    fn key_name(&self, scan: i64, modifier: i64) -> String;

    /// The DirectInput scan code the game reads for a key pressed; None for a key it has none
    /// for.
    fn dik_code(&self, key: winit::keyboard::KeyCode) -> Option<i32>;

    /// The Depot's own game controllers file (`Inputs/gamectrler.cfg` in its content folder).
    fn controllers_file(&self) -> Option<std::path::PathBuf>;

    /// The game controllers saved to the Depot's own file.
    fn save_controllers(&self, devices: &[DeviceCfg]) -> Result<(), String>;
}
