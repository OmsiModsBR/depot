//! This build's version and the project's release page.
//!
//! In-app updating is disabled (see docs/removed/UPDATER_DISABLED.md): nothing here checks
//! for, downloads or installs a release. The only network-adjacent thing left is
//! [`open_url`], which hands a link to the system's browser when the player clicks it, and
//! only for a URL inside the project's own repository.

/// The project on GitHub.
pub const REPO_URL: &str = "https://github.com/OmsiModsBR/depot";
/// Where the releases are listed (opened only by an explicit click).
pub const RELEASES_URL: &str = "https://github.com/OmsiModsBR/depot/releases";

/// This build's version.
pub fn current_version() -> &'static str {
    crate::startup::VERSION
}

/// Whether `url` is the project's repository or a page inside it: `https` only, the exact
/// host `github.com`, the exact path prefix `/OmsiModsBR/depot`, then nothing, or `/`, `?`
/// or `#`. Anything else (another repository whose name merely starts the same, a userinfo
/// trick such as `https://github.com@evil/`, another scheme) is refused.
pub fn is_depot_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix(REPO_URL) else { return false };
    rest.is_empty() || rest.starts_with(['/', '?', '#'])
}

/// Open a page of the project's repository in the system's browser (called only on a click).
/// Other URLs are refused and logged.
pub fn open_url(url: &str) {
    if !is_depot_url(url) {
        log::warn!("not opening {url}: only pages of {REPO_URL} are opened");
        return;
    }
    #[cfg(target_os = "android")]
    crate::android::open_url(url);
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("rundll32").arg("url.dll,FileProtocolHandler").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos"), not(target_os = "android")))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_projects_own_pages_are_opened() {
        assert!(is_depot_url(REPO_URL));
        assert!(is_depot_url(RELEASES_URL));
        assert!(is_depot_url("https://github.com/OmsiModsBR/depot/issues/new?title=a&body=b"));
        assert!(is_depot_url("https://github.com/OmsiModsBR/depot#readme"));
        assert!(!is_depot_url("http://github.com/OmsiModsBR/depot"));
        assert!(!is_depot_url("https://github.com/OmsiModsBR/depot-evil"));
        assert!(!is_depot_url("https://github.com/OmsiModsBR/depotx/releases"));
        assert!(!is_depot_url("https://github.com.evil.example/OmsiModsBR/depot"));
        assert!(!is_depot_url("https://github.com@evil.example/OmsiModsBR/depot"));
        assert!(!is_depot_url("file:///etc/passwd"));
        assert!(!is_depot_url("javascript:alert(1)"));
        assert!(!is_depot_url(""));
    }

    #[test]
    fn the_release_link_is_inside_the_repository() {
        assert!(RELEASES_URL.starts_with(REPO_URL));
        assert!(!current_version().is_empty());
    }
}
