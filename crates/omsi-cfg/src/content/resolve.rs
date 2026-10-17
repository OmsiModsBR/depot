//! A content file's path found under the content roots.

use std::path::{Path, PathBuf};

use super::listing::{find_in_dir, name_key, windows_components};
use super::packages::{overlays_package, vehicle_package};
use super::roots::{content_roots, mirrored_base, owner_root};
use super::vfs;

/// Case-insensitive walk of the components `rel` from `base` using the cached directory
/// listings only; None as soon as a component does not exist.
pub(crate) fn resolve_existing(base: &Path, rel: &[&str]) -> Option<PathBuf> {
    let mut cur = base.to_path_buf();
    for comp in rel {
        if comp.is_empty() || *comp == "." {
            continue;
        }
        if *comp == ".." {
            cur.pop();
            continue;
        }
        let found = find_in_dir(&cur, comp).map(|n| cur.join(n))?;
        cur = found;
    }
    Some(cur)
}

/// Every content root's version of a folder given relative to a root (existing ones only),
/// highest priority first - for listing maps, vehicles, weathers across the installation
/// and the installed mods.
pub fn content_dirs(rel: &str) -> Vec<PathBuf> {
    let rel = windows_components(rel);
    content_roots()
        .iter()
        .filter_map(|r| resolve_existing(r, &rel))
        .filter(|p| vfs::is_dir(p))
        .collect()
}

/// Directory entries of `rel` (relative to a content root) merged over all roots; an
/// entry of a higher-priority root hides the same name lower down.
pub fn read_dir_merged(rel: &str) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for d in content_dirs(rel) {
        for (name, _) in vfs::list_dir(&d).unwrap_or_default() {
            if seen.insert(name_key(&name.to_string_lossy())) {
                out.push(d.join(name));
            }
        }
    }
    out
}

/// The file a content file names (`rel`, relative to the folder `base`), found the way
/// Windows finds it: see [`windows_components`] for the spelling rules, names compare without
/// regard to case, and a mod's copy under a content root of higher priority comes first.
pub fn resolve_path(base: &Path, rel: &str) -> PathBuf {
    let rel = windows_components(rel);
    let rel = rel.as_slice();
    // a drive or stream (`C:`, `x.bmp:y`) or a NUL in a name: `Path::join` takes a drive
    // component for a new root on Windows, which would leave the folder; no file has such a
    // name, so it names nothing
    if rel.iter().any(|c| c.contains([':', '\0'])) {
        return base.join("?invalid-content-name");
    }
    // a mod's copy first: the same place under a content root of higher priority than the
    // one `base` belongs to
    let roots = content_roots();
    let owner = if roots.len() > 1 {
        owner_root(base, &roots).cloned()
    } else {
        None
    };
    if let Some(owner) = &owner {
        let package = vehicle_package(base, owner, rel);
        for r in roots.iter().take_while(|r| *r != owner) {
            // another version of the same vehicle pack does not patch this one (see
            // `overlays_package`); it is only asked below, for what this one lacks
            if let Some(pkg) = &package {
                if !overlays_package(r, owner, pkg) {
                    continue;
                }
            }
            if let Some(mb) = mirrored_base(base, r, &roots) {
                if let Some(p) = resolve_existing(&mb, rel) {
                    return p;
                }
            }
        }
    }
    let mut cur = base.to_path_buf();
    for comp in rel {
        if *comp == ".." {
            cur.pop();
            continue;
        }
        let direct = cur.join(comp);
        if vfs::exists(&direct) {
            cur = direct;
            continue;
        }
        // case-insensitive lookup through a per-directory listing kept from the first
        // time (scanning the folder afresh for every miss made texture lookups crawl)
        let found = find_in_dir(&cur, comp).map(|n| cur.join(n));
        cur = found.unwrap_or(direct);
    }
    if let (Some(owner), false) = (&owner, vfs::exists(&cur)) {
        // not in `base`'s root either: another root may still have it - a lower-priority
        // one, or another version of the pack that was passed over above
        for r in roots.iter().filter(|r| *r != owner) {
            if let Some(mb) = mirrored_base(base, r, &roots) {
                if let Some(p) = resolve_existing(&mb, rel) {
                    return p;
                }
            }
        }
    }
    cur
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::is_in_content_roots;

    #[test]
    fn foreign_names_resolve_to_nothing() {
        let base = std::env::temp_dir().join("omsi-foreign-names-base");
        for name in ["C:\\x.bmp", "a\\C:evil\\x.bmp", "x.bmp:stream", "a\0b"] {
            let p = resolve_path(&base, name);
            assert!(
                p.starts_with(&base) && !vfs::exists(&p),
                "{name}: {}",
                p.display()
            );
        }
        // a network path has its empty parts dropped: it lands under `base`, not on the host
        let p = resolve_path(&base, "\\\\host\\share\\x.bmp");
        assert!(p.starts_with(&base), "{}", p.display());
        assert!(!is_in_content_roots(Path::new("\\\\host\\share\\x.bmp")));
        assert!(!is_in_content_roots(Path::new("//host/share/x.bmp")));
        assert!(!is_in_content_roots(Path::new(
            "/content/root/../private.png"
        )));
    }

    #[test]
    fn windows_paths() {
        assert_eq!(
            windows_components("texture\\anz-oben.jpg."),
            vec!["texture", "anz-oben.jpg"]
        );
        assert_eq!(
            windows_components(" ..\\..\\Model.\\a.o3d . "),
            vec!["..", "..", "Model", "a.o3d"]
        );
        assert_eq!(
            windows_components("\\Splines//x\\.\\y.sli"),
            vec!["Splines", "x", "y.sli"]
        );
        assert_eq!(windows_components("Folder..\\x"), vec!["Folder.", "x"]);
        assert_eq!(windows_components("Folder.\\"), vec!["Folder"]);
        assert_eq!(
            windows_components("str_gehweg02.bmp "),
            vec!["str_gehweg02.bmp"]
        );
        assert_eq!(windows_components(""), Vec::<&str>::new());
        let dir = std::env::temp_dir().join(format!("omsi-cfg-paths-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("Texture").join("Bahn\u{fc}bergang")).unwrap();
        std::fs::write(
            dir.join("Texture")
                .join("Bahn\u{fc}bergang")
                .join("anz-oben.jpg"),
            b"x",
        )
        .unwrap();
        let p = resolve_path(
            &dir.join("model"),
            "..\\TEXTURE.\\BAHN\u{dc}BERGANG/Anz-Oben.JPG. ",
        );
        assert!(vfs::is_file(&p), "{}", p.display());
        std::fs::remove_dir_all(&dir).ok();
    }
}
