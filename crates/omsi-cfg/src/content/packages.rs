//! Vehicle packs: which copy of a pack patches another, and which pack a missing file needs.

use std::path::{Path, PathBuf};

use super::listing::name_key;
use super::resolve::resolve_existing;
use super::roots::{content_roots, owner_root};
use super::vfs;

/// The vehicle pack (`Vehicles/<folder>`, relative to its root) that `rel` from `base` lands
/// in, when it does: the files of one vehicle refer to each other relative to its folder.
pub(super) fn vehicle_package(base: &Path, owner: &Path, rel: &[&str]) -> Option<String> {
    let suffix = base.strip_prefix(owner).ok()?;
    let mut parts: Vec<String> = suffix
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(n) => Some(n.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();
    for comp in rel {
        if *comp == ".." {
            parts.pop();
        } else {
            parts.push(comp.to_string());
        }
    }
    if parts.len() >= 3 && parts[0].eq_ignore_ascii_case("Vehicles") {
        Some(format!("Vehicles/{}", parts[1]))
    } else {
        None
    }
}

/// The vehicle definitions (`.bus`, `.ovh`, `.zug`) lying directly in a pack folder.
fn package_definitions(root: &Path, package: &str) -> std::collections::HashSet<String> {
    let comps: Vec<&str> = package.split('/').collect();
    let Some(dir) = resolve_existing(root, &comps) else {
        return Default::default();
    };
    vfs::list_dir(&dir)
        .unwrap_or_default()
        .into_iter()
        .filter(|(_, is_dir)| !is_dir)
        .map(|(n, _)| name_key(&n.to_string_lossy()))
        .filter(|n| n.ends_with(".bus") || n.ends_with(".ovh") || n.ends_with(".zug"))
        .collect()
}

/// Whether the copy of vehicle pack `package` under the higher-priority root `over` patches
/// the one under `owner` (the root the vehicle was loaded from), i.e. its files replace
/// the owner's the way files copied over an installation would.
///
/// That holds for a patch (no vehicle definitions of its own, or only ones the owner's pack
/// has as well). A pack with vehicles the owner's copy does not know is another *version*
/// of the pack - a map archive bundles its own Citaro Facelift next to the one installed
/// in the content folder, with scripts, varlists and constfiles that belong together - and
/// taking single files from it broke the bus it did not come from (the Ahlheim Citaro's
/// VDV and IBIS scripts ran on the installed pack's varlists, missing half their
/// variables, and every display stayed dark). Such a copy is only asked for files the
/// owner's pack does not have.
pub(super) fn overlays_package(over: &Path, owner: &Path, package: &str) -> bool {
    type Key = (PathBuf, PathBuf, String);
    static CACHE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<Key, bool>>> =
        std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let key = (
        over.to_path_buf(),
        owner.to_path_buf(),
        package.to_ascii_lowercase(),
    );
    if let Some(v) = cache.lock().unwrap().get(&key) {
        return *v;
    }
    // (a root without the pack at all is only where the lookup starts: `--bus` is resolved
    // from the installation's root)
    let comps: Vec<&str> = package.split('/').collect();
    let owner_has = resolve_existing(owner, &comps)
        .map(|d| vfs::is_dir(&d))
        .unwrap_or(false);
    let theirs = package_definitions(over, package);
    let ok =
        !owner_has || theirs.is_empty() || theirs.is_subset(&package_definitions(owner, package));
    cache.lock().unwrap().insert(key, ok);
    ok
}

/// The vehicle pack a missing file belongs to, when that whole pack is installed in no
/// content root: a mod that borrows parts from another (the Ahlheim Citaro's Faremaster
/// ticket machine and dashboard come from `Vehicles/Urbino_II`) is missing them because the
/// other pack is not installed, not because of a broken path, and the player can fix that.
pub fn missing_vehicle_pack(path: &Path) -> Option<String> {
    let roots = content_roots();
    let owner = owner_root(path, &roots)?.clone();
    let suffix = path.strip_prefix(&owner).ok()?;
    let mut comps = suffix.components().filter_map(|c| match c {
        std::path::Component::Normal(n) => Some(n.to_string_lossy().into_owned()),
        _ => None,
    });
    let first = comps.next()?;
    let pack = comps.next()?;
    if !first.eq_ignore_ascii_case("Vehicles") || comps.next().is_none() {
        return None;
    }
    let installed = roots.iter().any(|r| {
        resolve_existing(r, &["Vehicles", pack.as_str()])
            .map(|d| vfs::is_dir(&d))
            .unwrap_or(false)
    });
    (!installed).then_some(pack)
}
