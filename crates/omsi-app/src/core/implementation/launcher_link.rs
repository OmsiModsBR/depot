//! Starting another launcher program: find it among the candidates on disk and start it.

use super::super::domain::launcher_link::candidates;
use anyhow::{anyhow, Result};
use std::path::PathBuf;

/// The launcher binary, the first candidate that is a file.
pub(crate) fn find_launcher() -> Option<PathBuf> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.to_path_buf()));
    candidates(
        omsi_cfg::flags::LAUNCHER.os().map(PathBuf::from),
        exe_dir.as_deref(),
        omsi_cfg::paths::data_dir(),
    )
    .into_iter()
    .find(|p| p.is_file())
}

/// Start the launcher and return once it is running. `Ok(false)` when there is none.
pub(crate) fn open_launcher() -> Result<bool> {
    let Some(bin) = find_launcher() else {
        return Ok(false);
    };
    log::info!("starting the launcher {}", bin.display());
    let exe = std::env::current_exe().unwrap_or_default();
    std::process::Command::new(&bin)
        .env("OPENOMSI_BIN", &exe)
        .spawn()
        .map_err(|e| anyhow!("starting {}: {e}", bin.display()))?;
    Ok(true)
}
