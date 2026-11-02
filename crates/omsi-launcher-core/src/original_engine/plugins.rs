//! The OMSI 2 folder's own plug-ins in OMSI 2 mode: listed, and switched off or on by the
//! player (Settings > Plug-ins).
//!
//! OMSI 2 loads a plug-in through its descriptor, a `.opl` file directly in `plugins/` that
//! names the plug-in's DLL. Switching a plug-in off renames that descriptor to
//! `<name>.opl.disabled` in the same folder; switching it on renames it back. Nothing is
//! deleted, copied or written, the plug-in's DLL and other files are never touched, and the
//! state is visible in the folder (a player can undo it by hand). A name that would take the
//! place of an existing file is never used: nothing of the player's is overwritten.
//!
//! OmsiLaunch's own plug-in (`plugins/OmsiLaunch.*`) is the link between Depot and OMSI 2: it
//! is listed, locked on, and never switched off here.

use super::install::{linked_part, safe_rel};
use anyhow::{anyhow, bail, Result};
use std::path::Path;

/// What is added to a descriptor's name to switch its plug-in off.
pub const OFF_SUFFIX: &str = ".disabled";

const OPL: &str = ".opl";

/// One plug-in of the OMSI 2 folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plugin {
    /// The descriptor's name without `.opl` (`OmsiLaunch.Plugin`), as it is written in the
    /// folder.
    pub name: String,
    /// OMSI 2 loads it (its `.opl` is there).
    pub on: bool,
    /// OmsiLaunch's own: always on.
    pub locked: bool,
    /// Both `<name>.opl` and `<name>.opl.disabled` are there: nothing is renamed until the
    /// player keeps one of them.
    pub twin: bool,
}

/// Whether plug-in `name` is OmsiLaunch's, which links Depot to OMSI 2 and stays on.
pub fn locked(name: &str) -> bool {
    name.to_ascii_lowercase().starts_with("omsilaunch.")
}

/// The descriptor's name without `.opl` and whether it is switched on, for a file name of
/// `plugins/`; None for any other file.
fn descriptor(file: &str) -> Option<(&str, bool)> {
    let lower = file.to_ascii_lowercase();
    let off = format!("{OPL}{OFF_SUFFIX}");
    let (len, on) = if lower.ends_with(&off) {
        (off.len(), false)
    } else if lower.ends_with(OPL) {
        (OPL.len(), true)
    } else {
        return None;
    };
    let name = &file[..file.len() - len];
    (!name.is_empty()).then_some((name, on))
}

/// The descriptors directly in `plugins/`: (file name, name without `.opl`, on). Only
/// regular files: a link or a folder is not a plug-in OMSI 2 reads.
fn descriptors(root: &Path) -> Vec<(String, String, bool)> {
    let Ok(dir) = std::fs::read_dir(root.join("plugins")) else {
        return Vec::new();
    };
    dir.flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| {
            let file = e.file_name().to_str()?.to_string();
            let (name, on) = descriptor(&file)?;
            let name = name.to_string();
            Some((file, name, on))
        })
        .collect()
}

/// The plug-ins of the OMSI 2 folder at `root`, switched on or off, by name.
pub fn list(root: &Path) -> Vec<Plugin> {
    let mut out: Vec<Plugin> = Vec::new();
    for (_, name, on) in descriptors(root) {
        match out.iter_mut().find(|p| p.name.eq_ignore_ascii_case(&name)) {
            Some(p) => {
                p.twin = true;
                p.on |= on;
                if on {
                    p.name = name;
                }
            }
            None => out.push(Plugin {
                locked: locked(&name),
                name,
                on,
                twin: false,
            }),
        }
    }
    out.sort_by_key(|p| p.name.to_lowercase());
    out
}

/// Switches plug-in `name` on (`<name>.opl.disabled` back to `<name>.opl`) or off (the other
/// way). Nothing happens when it already is so. Refused: OmsiLaunch's own plug-in switched
/// off, a name that is not a plain file name, a `plugins` folder reached through a link, a
/// descriptor that is a link, and a name already taken (both files there).
pub fn set_enabled(root: &Path, name: &str, on: bool) -> Result<()> {
    if name.contains('/') || safe_rel(name).is_none() {
        bail!("{name} is not a plug-in name");
    }
    if !on && locked(name) {
        bail!("OmsiLaunch's plug-in links Depot to OMSI 2 and stays on");
    }
    if let Some(link) = linked_part(root, "plugins") {
        bail!("{link} is a link: plug-ins are not renamed through links");
    }
    let dir = root.join("plugins");
    let found = descriptors(root);
    let file_of = |want_on: bool| {
        found
            .iter()
            .find(|(_, n, o)| *o == want_on && n.eq_ignore_ascii_case(name))
            .map(|(f, _, _)| f.clone())
    };
    match (file_of(on), file_of(!on)) {
        (Some(_), Some(_)) => bail!(
            "both {name}.opl and {name}.opl{OFF_SUFFIX} are in the plugins folder: keep one of them"
        ),
        (Some(_), None) => Ok(()),
        (None, Some(from)) => {
            let to = if on {
                from[..from.len() - OFF_SUFFIX.len()].to_string()
            } else {
                format!("{from}{OFF_SUFFIX}")
            };
            rename_new(&dir.join(&from), &dir.join(&to))
        }
        (None, None) => Err(anyhow!("{name} is not in the plugins folder")),
    }
}

/// Renames `from` to `to` in the same folder, never over an existing file.
fn rename_new(from: &Path, to: &Path) -> Result<()> {
    let meta = from.symlink_metadata()?;
    if !meta.is_file() {
        bail!("{} is not a plain file", from.display());
    }
    if to.symlink_metadata().is_ok() {
        bail!("{} already exists", to.display());
    }
    std::fs::rename(from, to).map_err(|e| anyhow!("renaming {}: {e}", from.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn omsi(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("depot-oe-plug-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("plugins")).unwrap();
        d
    }

    fn put(root: &Path, file: &str, body: &str) {
        std::fs::write(root.join("plugins").join(file), body).unwrap();
    }

    fn files(root: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(root.join("plugins"))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    #[test]
    fn the_descriptors_are_the_plugins() {
        let root = omsi("list");
        put(
            &root,
            "OmsiLaunch.Plugin.opl",
            "[dll]\nOmsiLaunch.PluginNE.dll\n",
        );
        put(&root, "OmsiLaunch.PluginNE.dll", "dll");
        put(&root, "Ticket.opl", "[dll]\nTicket.dll\n");
        put(&root, "Ticket.dll", "dll");
        put(&root, "Radio.OPL.disabled", "[dll]\nRadio.dll\n");
        put(&root, "notes.txt", "x");
        put(&root, ".opl", "x");
        std::fs::create_dir_all(root.join("plugins").join("Folder.opl")).unwrap();
        std::fs::create_dir_all(root.join("plugins").join("OmsiLaunch.Modules")).unwrap();
        let l = list(&root);
        let names: Vec<(&str, bool, bool)> = l
            .iter()
            .map(|p| (p.name.as_str(), p.on, p.locked))
            .collect();
        assert_eq!(
            names,
            [
                ("OmsiLaunch.Plugin", true, true),
                ("Radio", false, false),
                ("Ticket", true, false)
            ]
        );
        assert!(l.iter().all(|p| !p.twin));
        assert!(list(&root.join("nothing")).is_empty());
    }

    #[test]
    fn off_and_on_again_rename_only_the_descriptor() {
        let root = omsi("toggle");
        put(&root, "Ticket.opl", "[dll]\nTicket.dll\n");
        put(&root, "Ticket.dll", "dll");
        set_enabled(&root, "Ticket", false).unwrap();
        assert_eq!(files(&root), ["Ticket.dll", "Ticket.opl.disabled"]);
        assert!(!list(&root)[0].on);
        // already off: nothing changes
        set_enabled(&root, "ticket", false).unwrap();
        assert_eq!(files(&root), ["Ticket.dll", "Ticket.opl.disabled"]);
        set_enabled(&root, "Ticket", true).unwrap();
        assert_eq!(files(&root), ["Ticket.dll", "Ticket.opl"]);
        assert_eq!(
            std::fs::read_to_string(root.join("plugins/Ticket.opl")).unwrap(),
            "[dll]\nTicket.dll\n"
        );
        // the name's case as written in the folder is kept
        put(&root, "Radio.OPL.disabled", "r");
        set_enabled(&root, "radio", true).unwrap();
        assert!(files(&root).contains(&"Radio.OPL".to_string()));
    }

    #[test]
    fn omsilaunch_stays_on() {
        let root = omsi("locked");
        put(&root, "OmsiLaunch.Plugin.opl", "x");
        assert!(set_enabled(&root, "OmsiLaunch.Plugin", false).is_err());
        assert!(set_enabled(&root, "omsilaunch.plugin", false).is_err());
        assert_eq!(files(&root), ["OmsiLaunch.Plugin.opl"]);
        // switched off by hand: it may be switched back on
        std::fs::rename(
            root.join("plugins/OmsiLaunch.Plugin.opl"),
            root.join("plugins/OmsiLaunch.Plugin.opl.disabled"),
        )
        .unwrap();
        set_enabled(&root, "OmsiLaunch.Plugin", true).unwrap();
        assert_eq!(files(&root), ["OmsiLaunch.Plugin.opl"]);
    }

    #[test]
    fn nothing_is_overwritten() {
        let root = omsi("twin");
        put(&root, "Ticket.opl", "new");
        put(&root, "Ticket.opl.disabled", "old");
        let l = list(&root);
        assert_eq!(l.len(), 1);
        assert!(l[0].on && l[0].twin);
        assert!(set_enabled(&root, "Ticket", false).is_err());
        assert!(set_enabled(&root, "Ticket", true).is_err());
        assert_eq!(
            std::fs::read_to_string(root.join("plugins/Ticket.opl.disabled")).unwrap(),
            "old"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("plugins/Ticket.opl")).unwrap(),
            "new"
        );
        assert!(set_enabled(&root, "Missing", true).is_err());
    }

    #[test]
    fn names_and_links_are_refused() {
        let root = omsi("names");
        put(&root, "Ticket.opl", "x");
        for bad in ["../Ticket", "a/Ticket", "a\\Ticket", "C:Ticket", "", "NUL"] {
            assert!(set_enabled(&root, bad, false).is_err(), "{bad}");
        }
        assert_eq!(files(&root), ["Ticket.opl"]);
        #[cfg(unix)]
        {
            // a descriptor that is a link is not a plug-in here
            let target = root.join("elsewhere.opl");
            std::fs::write(&target, "x").unwrap();
            std::os::unix::fs::symlink(&target, root.join("plugins/Linked.opl")).unwrap();
            assert!(list(&root).iter().all(|p| p.name != "Linked"));
            assert!(set_enabled(&root, "Linked", false).is_err());
            assert!(target.exists() && root.join("plugins/Linked.opl").exists());
            // a plugins folder that is a link is not renamed in
            let root2 = omsi("names-link");
            let real = omsi("names-real");
            put(&real, "Ticket.opl", "x");
            std::fs::remove_dir_all(root2.join("plugins")).unwrap();
            std::os::unix::fs::symlink(real.join("plugins"), root2.join("plugins")).unwrap();
            assert!(set_enabled(&root2, "Ticket", false).is_err());
            assert!(real.join("plugins/Ticket.opl").exists());
        }
    }
}
