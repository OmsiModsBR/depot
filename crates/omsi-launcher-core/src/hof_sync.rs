//! Keeps the depot files (`.hof`) of the game root in every playable vehicle.
//!
//! OMSI 2 players copy a map's `.hof` into the folder of each bus that should offer it. Here
//! the launcher does that on its own: every `.hof` in the root folders called `hof` or
//! `hofs` (any letter case) of the content roots is put beside every playable vehicle.
//!
//! The OMSI 2 folder is never written to. A vehicle that lives there gets a folder of the
//! same path in the Depot's own content folder (the overlay) holding only the synced files;
//! the game merges both folders of a vehicle when it loads (`omsi_cfg::mirrored_dirs`), and
//! the overlay's file is the one that takes effect. "Newer" is the files' modified time: a
//! file restored from an archive with an old date, or a changed clock, flips the outcome. Nothing is deleted, a file that is the
//! same already (in the overlay or in the vehicle's own folder) is left alone, and running
//! the sync again changes nothing.

use std::path::{Path, PathBuf};

/// Where the files of one playable vehicle go.
#[derive(Clone, Debug)]
pub struct VehicleDir {
    /// The folder the synced files are written to (in the content folder, never in OMSI 2's).
    pub overlay: PathBuf,
    /// The vehicle's other real folders (the OMSI 2 one, a mod's), read to see what it has.
    pub existing: Vec<PathBuf>,
}

/// What a sync did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    /// Source files found.
    pub sources: usize,
    /// Vehicle folders looked at (one per folder, not per `.bus`).
    pub vehicles: usize,
    /// Files written where the vehicle had none.
    pub created: usize,
    /// Overlay files replaced by a newer source.
    pub updated: usize,
    /// Files the vehicle had already, byte for byte.
    pub identical: usize,
    /// Overlay files that differ but are not older than the source: left as they are.
    pub kept: usize,
    /// Files that could not be written (read-only, no space ...).
    pub failed: usize,
    /// The first few failures, with the reason.
    pub failures: Vec<String>,
    /// Files left alone because the bus's own file in another folder differs and is newer than
    /// the source (a bus's own, newer depot file is never overridden by the root's).
    pub own_newer: usize,
    /// Of those, the bus has a newer own file but an older Depot copy from an earlier run is
    /// still in the overlay and, read first, still takes effect: the sync deletes nothing, so
    /// it only counts them (remove that copy by hand to let the bus's file win).
    pub stale_overlay: usize,
    /// The overlay folders where a file was written (created or replaced).
    pub changed: Vec<PathBuf>,
    /// The vehicle folders (by name, as the vehicle list has them) of the vehicles whose
    /// overlay changed: only these are read again. Filled by the launcher, not by `sync`.
    pub folders: Vec<String>,
}

impl Summary {
    pub fn line(&self) -> String {
        format!(
            "hof sync: {} depot file(s) for {} vehicle folder(s): {} created, {} updated, {} already there, {} kept, {} left to a newer bus file ({} of them with an older Depot copy still in the overlay), {} failed",
            self.sources, self.vehicles, self.created, self.updated, self.identical, self.kept, self.own_newer, self.stale_overlay, self.failed
        ) + &self.failures.iter().map(|f| format!("; failed: {f}")).collect::<String>()
    }
}

fn is_hof(p: &Path) -> bool {
    p.extension().is_some_and(|e| e.eq_ignore_ascii_case("hof"))
}

/// The `.hof` files directly inside the folders called `hof` or `hofs` (any case) of each
/// root; the first root's file wins over a later one of the same name.
pub fn source_files(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for root in roots {
        let Ok(rd) = std::fs::read_dir(root) else {
            continue;
        };
        let mut dirs: Vec<PathBuf> = rd
            .flatten()
            .filter(|e| {
                let n = e.file_name().to_string_lossy().to_ascii_lowercase();
                (n == "hof" || n == "hofs") && e.path().is_dir()
            })
            .map(|e| e.path())
            .collect();
        dirs.sort();
        for d in dirs {
            let Ok(rd) = std::fs::read_dir(&d) else {
                continue;
            };
            let mut files: Vec<PathBuf> = rd
                .flatten()
                .map(|e| e.path())
                .filter(|p| is_hof(p) && p.is_file())
                .collect();
            files.sort();
            for f in files {
                let key = f
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_ascii_lowercase();
                if seen.insert(key) {
                    out.push(f);
                }
            }
        }
    }
    out
}

/// `rel` (slash separated) below `base`, matching existing folder names in any letter case
/// and staying inside `base`: unlike `omsi_cfg::resolve_path`, a folder that is missing here
/// is never looked for in another content root, so the result is always under `base`.
pub fn local_dir(base: &Path, rel: &str) -> PathBuf {
    let mut cur = base.to_path_buf();
    for comp in rel
        .split(['/', '\\'])
        .filter(|c| !c.is_empty() && *c != "." && *c != "..")
    {
        // (the spelling on disk: a case-insensitive file system answers `exists` for any)
        let found = std::fs::read_dir(&cur).ok().and_then(|rd| {
            rd.flatten()
                .map(|e| e.file_name())
                .find(|n| n.to_string_lossy().eq_ignore_ascii_case(comp))
        });
        cur = cur.join(found.unwrap_or_else(|| comp.into()));
    }
    cur
}

/// The file of `dir` called `name` (any letter case).
fn find_in(dir: &Path, name: &str) -> Option<PathBuf> {
    let direct = dir.join(name);
    if direct.is_file() {
        return Some(direct);
    }
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            p.is_file()
                && p.file_name()
                    .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(name))
        })
}

fn same_bytes(a: &Path, b: &Path) -> bool {
    let (Ok(ma), Ok(mb)) = (std::fs::metadata(a), std::fs::metadata(b)) else {
        return false;
    };
    ma.len() == mb.len()
        && match (std::fs::read(a), std::fs::read(b)) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
}

fn newer(src: &Path, dest: &Path) -> bool {
    let m = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    match (m(src), m(dest)) {
        (Some(s), Some(d)) => s > d,
        _ => false,
    }
}

/// Whether `dir` holds a vehicle (a `.bus` or `.ovh` file): a folder of a bus is somebody's
/// files, not ours, and no file in it is ever replaced.
fn holds_vehicle(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|rd| {
        rd.flatten().any(|e| {
            let p = e.path();
            p.extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("bus") || x.eq_ignore_ascii_case("ovh"))
        })
    })
}

/// Puts every file of `sources` into every vehicle folder of `vehicles` that does not have it.
///
/// Nothing is deleted or lost: a file the vehicle has already (byte for byte) is skipped; a
/// differing file of the same name in the overlay is replaced only when the source is newer,
/// never in a folder that holds a vehicle (a mod's own files), and the file it replaces is
/// kept beside it as `<name>.hof.bak` (the first one replaced only; later replacements do
/// not refresh it). A bus's own differing file in its other folders is
/// not touched, and when it is newer than the source nothing is put beside it (the bus's
/// newer file wins); when it is older, the overlay copy is put beside it and, the game
/// reading the overlay first, takes effect.
pub fn sync(sources: &[PathBuf], vehicles: &[VehicleDir]) -> Summary {
    let mut sum = Summary {
        sources: sources.len(),
        ..Default::default()
    };
    let mut done = std::collections::HashSet::new();
    for v in vehicles {
        if !done.insert(v.overlay.to_string_lossy().to_ascii_lowercase()) {
            continue;
        }
        sum.vehicles += 1;
        let own = holds_vehicle(&v.overlay);
        for src in sources {
            let name = src.file_name().unwrap_or_default().to_string_lossy();
            let in_overlay = find_in(&v.overlay, &name);
            let others: Vec<PathBuf> = v
                .existing
                .iter()
                .filter_map(|d| find_in(d, &name))
                .collect();
            if in_overlay.as_deref().is_some_and(|p| same_bytes(src, p))
                || others.iter().any(|p| same_bytes(src, p))
            {
                sum.identical += 1;
                continue;
            }
            // (the bus's own file, when it is newer than the root's, is the one that stays)
            if others.iter().any(|p| newer(p, src)) {
                sum.own_newer += 1;
                if in_overlay.is_some() {
                    sum.stale_overlay += 1;
                }
                continue;
            }
            let replacing = in_overlay.is_some();
            if let Some(p) = &in_overlay {
                if own || !newer(src, p) {
                    sum.kept += 1;
                    continue;
                }
            }
            let dest = in_overlay
                .clone()
                .unwrap_or_else(|| v.overlay.join(name.as_ref()));
            let r = std::fs::create_dir_all(&v.overlay).and_then(|_| {
                if replacing {
                    let bak = PathBuf::from(format!("{}.bak", dest.display()));
                    if !bak.exists() {
                        std::fs::copy(&dest, &bak)?;
                    }
                }
                // (a temporary name first: a game started meanwhile never reads half a file)
                let tmp = PathBuf::from(format!("{}.tmp", dest.display()));
                std::fs::copy(src, &tmp)
                    .and_then(|_| std::fs::rename(&tmp, &dest))
                    .inspect_err(|_| {
                        let _ = std::fs::remove_file(&tmp);
                    })
            });
            if r.is_ok() && !sum.changed.contains(&v.overlay) {
                sum.changed.push(v.overlay.clone());
            }
            match (r, replacing) {
                (Ok(()), false) => sum.created += 1,
                (Ok(()), true) => sum.updated += 1,
                (Err(e), _) => {
                    sum.failed += 1;
                    if sum.failures.len() < 3 {
                        sum.failures.push(format!("{}: {e}", dest.display()));
                    }
                }
            }
        }
    }
    sum
}

/// The vehicle folders (by name) whose overlay folder got a file: the only ones read again.
pub fn affected_folders(changed: &[PathBuf], overlay_folder: &[(PathBuf, String)]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (overlay, folder) in overlay_folder {
        if changed.contains(overlay) && !out.contains(folder) {
            out.push(folder.clone());
        }
    }
    out
}
