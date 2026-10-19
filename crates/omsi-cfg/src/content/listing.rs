//! File names as Windows compares them, and the listings of folders kept between lookups.

use std::path::{Path, PathBuf};

use super::vfs;
use crate::codepage;

/// Convert an OMSI content path (backslashes, case-insensitive on Windows) to a real path
/// relative to `base`, resolving the case of every component on case-sensitive file systems.
///
/// The components of a content path the way Windows reads them: `\` and `/` both separate,
/// empty and `.` components vanish, a folder name loses one trailing dot and the last name
/// all trailing dots and spaces (Win32 path normalisation: an Ahlheim mesh asks for
/// `anz-oben.jpg.`, which Windows opens as `anz-oben.jpg`). `..` is left for the caller.
pub fn windows_components(rel: &str) -> Vec<&str> {
    let parts: Vec<&str> = rel.trim().split(['/', '\\']).collect();
    let n = parts.len();
    let mut out = Vec::with_capacity(n);
    for (i, c) in parts.into_iter().enumerate() {
        if c.is_empty() || c == "." {
            continue;
        }
        if c == ".." {
            out.push(c);
            continue;
        }
        // a path ending in a separator has an empty last part, so its last folder counts
        // as a folder here
        let c = if i + 1 == n {
            c.trim_end_matches(['.', ' '])
        } else {
            c.strip_suffix('.').unwrap_or(c)
        };
        if !c.is_empty() {
            out.push(c);
        }
    }
    out
}

/// How names compare, as on Windows: without regard to case (letters beyond ASCII too -
/// `Bahnübergang` is `BAHNÜBERGANG`).
pub(super) fn name_key(name: &str) -> String {
    if name.is_ascii() {
        name.to_ascii_lowercase()
    } else {
        // (composed: a Russian stop's announcement `Лесной.wav` unpacked on a Mac is often
        // stored with `й` as `и` + a combining breve, and was never found)
        use unicode_normalization::UnicodeNormalization;
        name.nfc().collect::<String>().to_lowercase()
    }
}

type Listing = std::sync::Arc<std::collections::HashMap<String, std::ffi::OsString>>;
static LISTINGS: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<PathBuf, Listing>>,
> = std::sync::OnceLock::new();
/// Bumped whenever content may have come or gone (a root added or taken away, a mod
/// installed while the game runs): the caches of what is where start afresh.
static CONTENT_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The content changed: directory listings are read again, and lookups that missed before
/// (see `content_generation`) are tried again.
pub fn content_changed() {
    CONTENT_GENERATION.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    if let Some(l) = LISTINGS.get() {
        l.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }
}

/// How often the content changed so far (a cache of lookups keeps its misses only as
/// long as this stays the same).
pub fn content_generation() -> u64 {
    CONTENT_GENERATION.load(std::sync::atomic::Ordering::SeqCst)
}

/// The entries of a directory by [`name_key`], listed once.
fn dir_listing(dir: &Path) -> Listing {
    let cache = LISTINGS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    if let Some(l) = cache.lock().unwrap().get(dir) {
        return l.clone();
    }
    let mut map = std::collections::HashMap::new();
    let entries = vfs::list_dir(dir).unwrap_or_default();
    for (name, _) in &entries {
        map.insert(name_key(&name.to_string_lossy()), name.clone());
    }
    // the other spellings of names beyond ASCII (see `codepage::name_variants`), behind
    // every real name
    for (name, _) in &entries {
        for v in codepage::name_variants(&name.to_string_lossy()) {
            map.entry(name_key(&v)).or_insert_with(|| name.clone());
        }
    }
    let l = std::sync::Arc::new(map);
    cache.lock().unwrap().insert(dir.to_path_buf(), l.clone());
    l
}

/// The entry of `dir` that `name` means: the same name without regard to case, or one of
/// its other spellings (a name beyond ASCII read in another code page on the way).
pub(super) fn find_in_dir(dir: &Path, name: &str) -> Option<std::ffi::OsString> {
    let listing = dir_listing(dir);
    if let Some(n) = listing.get(&name_key(name)) {
        return Some(n.clone());
    }
    codepage::name_variants(name)
        .iter()
        .find_map(|v| listing.get(&name_key(v)).cloned())
}
