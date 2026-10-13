//! Another launcher program (`OMSI_LAUNCHER=<program>`): the game's own window is the
//! launcher now (`launcher`), this only starts a different one when asked to.

use anyhow::{anyhow, Result};
use crate::startup::root_memo;
use std::path::PathBuf;

/// The launcher binary: next to this program, or the launcher's own build folder in the
/// source tree, or in ~/.openomsi.
pub(crate) fn find_launcher() -> Option<PathBuf> {
    let mut cands: Vec<PathBuf> = Vec::new();
    if let Some(p) = omsi_cfg::flags::LAUNCHER.os() {
        cands.push(PathBuf::from(p));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            cands.push(dir.join("depot-launcher"));
            cands.push(dir.join("depot-launcher.exe"));
            cands.push(dir.join("depot.app/Contents/MacOS/depot-launcher"));
        }
    }
    if let Some(memo) = root_memo() {
        if let Some(home) = memo.parent() {
            cands.push(home.join(".depot/depot-launcher"));
        }
    }
    cands.into_iter().find(|p| p.is_file())
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
