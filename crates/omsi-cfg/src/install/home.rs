//! The user's data folder.

/// Move the data of a version from before the rename (`~/.omsi-rewrite`,
/// `~/.omsi-rewrite-root`) to its new place (`~/.depot`, `~/.depot-root`), once.
/// (First thing at the start of every program: an unusable `HOME` is dropped here, see
/// [`drop_unusable_home`].)
pub fn migrate_legacy_data_dir() {
    drop_unusable_home();
    let Some(home) = super::paths::home() else {
        return;
    };
    for (old, new) in [
        (".omsi-rewrite", ".depot"),
        (".omsi-rewrite-root", ".depot-root"),
    ] {
        let (old, new) = (home.join(old), home.join(new));
        if old.exists() && !new.exists() {
            let _ = std::fs::rename(&old, &new);
        }
    }
}

/// On Windows a `HOME` variable some other program set for itself (a Unix-style path, a
/// network drive that is not connected) is no folder to keep openOMSI's data in: the
/// launcher's settings went nowhere, and the OMSI folder chosen under Setup was forgotten
/// as soon as it was saved - the lists stayed empty. Such a `HOME` is dropped for this
/// program (and the game it starts), which then uses `USERPROFILE` as without one.
pub fn drop_unusable_home() {
    if !cfg!(windows) {
        return;
    }
    let Some(h) = std::env::var_os("HOME") else {
        return;
    };
    let p = std::path::PathBuf::from(&h);
    let usable = p.is_absolute() && p.is_dir() && std::fs::create_dir_all(p.join(".depot")).is_ok();
    if !usable && std::env::var_os("USERPROFILE").is_some() {
        std::env::remove_var("HOME");
    }
}
