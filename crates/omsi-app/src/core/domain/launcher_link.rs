//! Where another launcher program may be: the places searched for it, in order (the
//! `OMSI_LAUNCHER` setting first, then the program's own folder, then ~/.depot).

use std::path::{Path, PathBuf};

/// The candidate paths for the launcher binary, in the order they are tried: the program
/// named by the setting, the launcher next to `exe_dir` (the program's folder, also the
/// macOS bundle's), and the launcher in the data folder.
pub(crate) fn candidates(
    setting: Option<PathBuf>,
    exe_dir: Option<&Path>,
    data_dir: Option<PathBuf>,
) -> Vec<PathBuf> {
    let mut cands: Vec<PathBuf> = Vec::new();
    cands.extend(setting);
    if let Some(dir) = exe_dir {
        cands.push(dir.join("depot-launcher"));
        cands.push(dir.join("depot-launcher.exe"));
        cands.push(dir.join("depot.app/Contents/MacOS/depot-launcher"));
    }
    cands.extend(data_dir.map(|data| data.join("depot-launcher")));
    cands
}
