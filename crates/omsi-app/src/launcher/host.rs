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

    fn open_url(&self, url: &str) {
        crate::about::open_url(url);
    }

    fn open_dotnet_download(&self) {
        crate::about::open_dotnet_download();
    }

    fn hide_content_from_gallery(&self) {
        #[cfg(target_os = "android")]
        crate::core::android::hide_content_from_gallery();
    }

    fn version(&self) -> (&'static str, &'static str) {
        (crate::core::startup::VERSION, crate::core::startup::BUILD)
    }

    fn repo_url(&self) -> &'static str {
        crate::about::REPO_URL
    }

    fn helper_log(&self) -> std::path::PathBuf {
        crate::showroom::helper_log()
    }
}
