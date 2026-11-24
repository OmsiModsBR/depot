//! Opening links in the system's browser: a page of the project on a click, and the
//! .NET download page the launcher needs.

use super::super::domain::about::{is_depot_url, DOTNET_DOWNLOAD_URL, REPO_URL};

/// Open a page of the project's repository in the system's browser (called only on a click).
/// Other URLs are refused and logged.
pub fn open_url(url: &str) {
    if !is_depot_url(url) {
        log::warn!("not opening {url}: only pages of {REPO_URL} are opened");
        return;
    }
    open_in_browser(url);
}

pub fn open_dotnet_download() {
    open_in_browser(DOTNET_DOWNLOAD_URL);
}

fn open_in_browser(url: &str) {
    #[cfg(target_os = "android")]
    crate::core::android::open_url(url);
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("rundll32")
        .arg("url.dll,FileProtocolHandler")
        .arg(url)
        .spawn();
    #[cfg(all(unix, not(target_os = "macos"), not(target_os = "android")))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}
