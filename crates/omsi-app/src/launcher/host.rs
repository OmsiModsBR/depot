//! The game's side of the launcher (`omsi_launcher_ui::Host`): what only the game has.

/// The game, as the launcher's state sees it.
pub(crate) struct GameHost;

impl omsi_launcher_ui::Host for GameHost {
    fn set_language(&self, code: &str) {
        crate::ui_language(code);
    }

    fn previous_run_crash(&self) -> Option<(String, String)> {
        #[cfg(target_os = "android")]
        {
            crate::core::android::previous_run_crash()
        }
        #[cfg(not(target_os = "android"))]
        {
            None
        }
    }
}
