//! The content roots, highest priority first, and which root a path belongs to.

use std::path::{Path, PathBuf};

use super::listing::{content_changed, windows_components};
use super::resolve::resolve_existing;
use super::vfs;

/// Content roots, highest priority first: the game's own content folder next to its
/// binary (where mods are installed) and the OMSI 2 installation. A path asked for
/// relative to a folder of one root is looked for at the same place under the others,
/// so a mod's `Vehicles\\Foo\\foo.bus` or a replacement texture is found as if it had
/// been copied into the original installation.
static CONTENT_ROOTS: std::sync::Mutex<Vec<PathBuf>> = std::sync::Mutex::new(Vec::new());

/// Register a content root. Roots are searched in the order they were added.
pub fn add_content_root(root: PathBuf) {
    let mut r = CONTENT_ROOTS.lock().unwrap();
    if !r.iter().any(|x| x == &root) {
        r.push(root);
        drop(r);
        content_changed();
    }
}

/// Register a content root to be searched just before `before` (at the end when `before`
/// is not a root): an archive installed while the launcher runs goes in front of the
/// OMSI installation, where the game will have it on its next start.
pub fn add_content_root_before(root: PathBuf, before: &Path) {
    let mut r = CONTENT_ROOTS.lock().unwrap();
    if r.iter().any(|x| x == &root) {
        return;
    }
    match r.iter().position(|x| x == before) {
        Some(i) => r.insert(i, root),
        None => r.push(root),
    }
    drop(r);
    content_changed();
}

/// Register a content root searched before every other one (the content a LAN host sent
/// for the session: its files are the host's versions, and must win).
pub fn add_content_root_first(root: PathBuf) {
    let mut r = CONTENT_ROOTS.lock().unwrap();
    r.retain(|x| x != &root);
    r.insert(0, root);
    drop(r);
    content_changed();
}

/// Take a content root away again (a LAN session's content, when the session ends).
pub fn remove_content_root(root: &Path) {
    CONTENT_ROOTS.lock().unwrap().retain(|x| x != root);
    content_changed();
}

static SANDBOX_ROOTS: std::sync::Mutex<Vec<PathBuf>> = std::sync::Mutex::new(Vec::new());

/// Mark a content root as a sandbox: content received from another machine. It is read as
/// data (maps, vehicles, objects) like any other root, but nothing that runs code comes from
/// it - no plugins are loaded from a sandbox.
pub fn mark_sandbox(root: PathBuf) {
    let mut r = SANDBOX_ROOTS.lock().unwrap();
    if !r.contains(&root) {
        r.push(root);
    }
}

/// Does `path` lie in a sandbox root (see [`mark_sandbox`])?
pub fn is_sandbox(path: &Path) -> bool {
    SANDBOX_ROOTS
        .lock()
        .unwrap()
        .iter()
        .any(|r| path.starts_with(r))
}

/// The first content root (in the order they are searched) that has `rel`, and the file
/// or folder found there (case-insensitive, as Windows reads the folder).
pub fn find_in_roots(rel: &str) -> Option<(PathBuf, PathBuf)> {
    let comps = windows_components(rel);
    content_roots()
        .into_iter()
        .find_map(|r| resolve_existing(&r, &comps).map(|p| (r, p)))
}

pub fn content_roots() -> Vec<PathBuf> {
    CONTENT_ROOTS.lock().unwrap().clone()
}

/// Every content root's copy of the folder `dir` (which lies under one of the roots) that
/// exists, highest priority first; just `dir` when it is under no root. A mod that adds
/// files to a stock folder (a repaint's `.cti` next to the original ones) puts them into
/// the same folder under the content folder (or an archive in `<content>/Archives`), and a
/// listing must see both.
pub fn mirrored_dirs(dir: &Path) -> Vec<PathBuf> {
    let roots = content_roots();
    let Some(suffix) = owner_root(dir, &roots).and_then(|r| dir.strip_prefix(r).ok()) else {
        return vec![dir.to_path_buf()];
    };
    // a real folder: its names as they are
    let rel: Vec<String> = suffix
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    let rel: Vec<&str> = rel.iter().map(|s| s.as_str()).collect();
    let mut out: Vec<PathBuf> = Vec::new();
    for r in &roots {
        let found = if rel.is_empty() {
            Some(r.clone())
        } else {
            resolve_existing(r, &rel)
        };
        if let Some(p) = found.filter(|p| vfs::is_dir(p)) {
            if !out.contains(&p) {
                out.push(p);
            }
        }
    }
    if out.is_empty() {
        out.push(dir.to_path_buf());
    }
    out
}

/// Does `path` lie under one of the content roots? A content file may name a full path (a
/// paint scheme's picture, as its author's machine had it); only one that is in our own
/// content is followed, never one that points at another place or another computer.
pub fn is_in_content_roots(path: &Path) -> bool {
    let s = path.to_string_lossy();
    // (a network path `\\\\host\\share` or `//host/share` is never content, whatever the roots)
    if s.starts_with("\\\\") || s.starts_with("//") {
        return false;
    }
    // (`..` would leave the root after the lexical check below; no root registered: nothing
    // is inside of anything)
    if path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return false;
    }
    owner_root(path, &content_roots()).is_some()
}

/// The content root `path` belongs to: the deepest one that contains it. An archive
/// mounted from `<content>/Archives` lies inside the content folder, and a path inside the
/// archive is the archive's, not the content folder's.
pub(super) fn owner_root<'a>(path: &Path, roots: &'a [PathBuf]) -> Option<&'a PathBuf> {
    roots
        .iter()
        .filter(|r| path.starts_with(r))
        .max_by_key(|r| r.components().count())
}

/// The same relative location of `base` under `root` (None if `base` belongs to `root`
/// itself, or to no root at all).
pub(super) fn mirrored_base(base: &Path, root: &Path, roots: &[PathBuf]) -> Option<PathBuf> {
    let owner = owner_root(base, roots)?;
    if owner == root {
        return None;
    }
    base.strip_prefix(owner)
        .ok()
        .map(|suffix| root.join(suffix))
}
