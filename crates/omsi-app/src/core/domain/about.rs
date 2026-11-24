//! The project's addresses and the rule for which links the game opens: the repository and
//! its releases, the one page the launcher opens outside it, and the build's version.

/// The project on GitHub.
pub const REPO_URL: &str = "https://github.com/OmsiModsBR/depot";
/// Where the releases are listed (opened only by an explicit click).
pub const RELEASES_URL: &str = "https://github.com/OmsiModsBR/depot/releases";

/// This build's version.
pub fn current_version() -> &'static str {
    crate::core::startup::VERSION
}

/// Whether `url` is the project's repository or a page inside it: `https` only, the exact
/// host `github.com`, the exact path prefix `/OmsiModsBR/depot`, then nothing, or `/`, `?`
/// or `#`. Anything else (another repository whose name merely starts the same, a userinfo
/// trick such as `https://github.com@evil/`, another scheme) is refused.
pub fn is_depot_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix(REPO_URL) else {
        return false;
    };
    rest.is_empty() || rest.starts_with(['/', '?', '#'])
}

/// The one page outside the repository the launcher opens: Microsoft's download page of the
/// .NET 6 runtimes OmsiLaunch needs (Setup, on a click). Exactly this address, nothing else.
pub const DOTNET_DOWNLOAD_URL: &str = "https://dotnet.microsoft.com/download/dotnet/6.0";
