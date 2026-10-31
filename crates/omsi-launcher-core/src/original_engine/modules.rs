//! Installing and removing the optional OmsiLaunch modules (Omsi Prefetch, openHUD), always
//! on the player's confirmation.
//!
//! The same rules as the OmsiLaunch install (see `install`): releases of the module's own
//! official repository only, the package checked against the published `.sha256`, extracted
//! into Depot's staging folder first, nothing outside the module's folder
//! (`plugins/OmsiLaunch.Modules/<id>/` in the OMSI 2 folder) is ever written, existing files
//! that are not the module's are never overwritten, links are refused, a failure half way puts
//! everything back, and what Depot owns is what its own record says, never a file inside the
//! OMSI 2 folder. A module is a separate program under its own license; Depot only places its
//! files and shows the license before the player confirms.

use super::install::{
    self, copy_into, file_hash, key, linked_part, missing_dirs, open_zip, safe_name, safe_rel,
    same_root, sha256_hex, Fetch, InstallPlan, PlannedFile, ReleaseInfo, MAX_UNPACKED,
};
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};

/// An optional module Depot knows how to install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleDef {
    /// The module's id, also its folder name.
    pub id: &'static str,
    pub name: &'static str,
    /// The official repository; nothing else is ever downloaded for this module.
    pub repo: &'static str,
    pub license: &'static str,
    /// Packages are named `<asset_prefix><version>.zip`, with a `.sha256` beside them.
    pub asset_prefix: &'static str,
}

/// The modules Depot offers. Adding one is adding a line here: its repository must be the
/// project's own.
pub const MODULES: &[ModuleDef] = &[
    ModuleDef {
        id: "omsi-prefetch",
        name: "Omsi Prefetch",
        repo: "OmsiModsBR/omsi-prefetch",
        license: "GPL-3.0-only",
        asset_prefix: "omsi-prefetch-",
    },
    ModuleDef {
        id: "openhud",
        name: "openHUD",
        repo: "OmsiModsBR/openhud",
        license: "GPL-3.0-only",
        asset_prefix: "openhud-",
    },
];

pub fn module_def(id: &str) -> Option<&'static ModuleDef> {
    MODULES.iter().find(|m| m.id == id)
}

/// Where modules live inside the OMSI 2 folder.
pub const MODULES_DIR: &str = "plugins/OmsiLaunch.Modules";

pub fn module_dir(id: &str) -> String {
    format!("{MODULES_DIR}/{id}")
}

// ---------------------------------------------------------------------------------------
// the release

pub fn latest_release(f: &dyn Fetch, def: &ModuleDef) -> Result<ReleaseInfo> {
    let body = f.get(
        &install::releases_url_of(def.repo),
        4 * 1024 * 1024,
        &mut |_, _| true,
    )?;
    install::parse_releases_with(&String::from_utf8_lossy(&body), def.asset_prefix).ok_or_else(
        || {
            anyhow!(
                "no {} release with a package and a checksum was found",
                def.name
            )
        },
    )
}

/// Downloads the package and its checksum and returns the package once the hash matches.
pub fn download(
    f: &dyn Fetch,
    def: &ModuleDef,
    rel: &ReleaseInfo,
    progress: &mut dyn FnMut(u64, Option<u64>) -> bool,
) -> Result<(Vec<u8>, String)> {
    install::download_from(f, def.repo, rel, progress)
}

// ---------------------------------------------------------------------------------------
// the record of what Depot installed

/// What Depot added for one module, kept so a removal needs nothing from the OMSI 2 folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleRecord {
    pub root: String,
    pub version: String,
    pub sha256: String,
    /// Paths relative to the OMSI 2 folder.
    pub files: Vec<String>,
    /// Folders Depot made for these files (the only ones a removal may take away).
    #[serde(default)]
    pub dirs: Vec<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Records {
    #[serde(default)]
    modules: BTreeMap<String, ModuleRecord>,
}

pub fn records_path(data: &Path) -> PathBuf {
    super::config::omsi2_dir_in(data).join("module-records.json")
}

fn load_records(data: &Path) -> Records {
    std::fs::read_to_string(records_path(data))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn save_records(data: &Path, r: &Records) -> Result<()> {
    super::config::write_atomic(&records_path(data), &serde_json::to_vec_pretty(r)?)
}

/// The record of module `id` for this OMSI 2 folder, if Depot installed it there.
pub fn load_record(data: &Path, root: &Path, id: &str) -> Option<ModuleRecord> {
    load_records(data)
        .modules
        .remove(id)
        .filter(|r| same_root(Path::new(&r.root), root))
}

/// Files Depot owns for the module (what an update may replace and a removal may delete).
pub fn owned_list(data: &Path, root: &Path, id: &str) -> Vec<String> {
    let dir = module_dir(id);
    load_record(data, root, id)
        .map(|r| r.files)
        .unwrap_or_default()
        .into_iter()
        .filter(|p| safe_rel(p).is_some() && p.starts_with(&format!("{dir}/")))
        .collect()
}

// ---------------------------------------------------------------------------------------
// what an install would add

/// The files of a package as `(index in the zip, path inside the OMSI 2 folder, size)`.
///
/// A package either holds its files at the top (`module.json` first) or all of them under
/// one folder named like the module. Nothing else is accepted: no path leaves the module's
/// folder, no link, no duplicate or case-colliding name, no device name.
fn package_files(def: &ModuleDef, zip: &[u8]) -> Result<Vec<(usize, String, u64)>> {
    let mut a = open_zip(zip)?;
    let prefix = format!("{}/", def.id);
    let mut names: Vec<(usize, String, u64)> = Vec::new();
    for i in 0..a.len() {
        let f = a.by_index(i)?;
        if f.is_dir() {
            continue;
        }
        let name = f.name()?;
        if f.unix_mode()
            .map(|m| m & 0o170000 == 0o120000)
            .unwrap_or(false)
        {
            bail!("the package holds a link ({name})");
        }
        names.push((i, name.into_owned(), f.size()));
    }
    let wrapped = !names.is_empty() && names.iter().all(|(_, n, _)| n.starts_with(&prefix));
    let mut out = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut total = 0u64;
    for (i, name, size) in names {
        let inner = if wrapped {
            name[prefix.len()..].to_string()
        } else {
            name.clone()
        };
        let inner =
            safe_rel(&inner).ok_or_else(|| anyhow!("the package holds an unsafe path ({name})"))?;
        let full = safe_rel(&format!("{}/{}", module_dir(def.id), inner))
            .ok_or_else(|| anyhow!("the package holds an unsafe path ({name})"))?;
        if !seen.insert(key(&full)) {
            bail!("the package lists {inner} twice");
        }
        total = total.saturating_add(size);
        if total > MAX_UNPACKED {
            bail!("the package unpacks to too much data");
        }
        out.push((i, full, size));
    }
    if !seen.contains(&key(&format!("{}/module.json", module_dir(def.id)))) {
        bail!("the package has no module.json");
    }
    Ok(out)
}

fn check_folder(root: &Path) -> Result<()> {
    if !root.join("plugins").is_dir() {
        bail!("this is not an OMSI 2 folder (no plugins folder)");
    }
    if let Some(link) = linked_part(root, "plugins") {
        bail!("{link} is a link: modules are not installed through links");
    }
    Ok(())
}

/// Lists what the package would add, without writing anything.
pub fn plan_install(root: &Path, data: &Path, def: &ModuleDef, zip: &[u8]) -> Result<InstallPlan> {
    check_folder(root)?;
    let files = package_files(def, zip)?;
    let owned: BTreeSet<String> = owned_list(data, root, def.id)
        .iter()
        .map(|p| key(p))
        .collect();
    let (mut planned, mut replaces, mut conflicts) = (Vec::new(), Vec::new(), Vec::new());
    let mut total = 0u64;
    for (_, path, size) in files {
        if let Some(link) = linked_part(root, &path) {
            bail!("{link} is a link: modules are not installed through links");
        }
        if root.join(&path).symlink_metadata().is_ok() {
            if owned.contains(&key(&path)) {
                replaces.push(path.clone());
            } else {
                conflicts.push(path.clone());
            }
        }
        total = total.saturating_add(size);
        planned.push(PlannedFile { path, bytes: size });
    }
    Ok(InstallPlan {
        files: planned,
        total_bytes: total,
        replaces,
        conflicts,
    })
}

/// What `module.json` must say: this module, and a version. When it lists components with a
/// SHA-256 (`files` or `components`), each must be in the package and match.
fn check_manifest(def: &ModuleDef, staged: &Path) -> Result<()> {
    let text = std::fs::read_to_string(staged.join("module.json"))
        .map_err(|_| anyhow!("module.json is not readable"))?;
    let v: Value = serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|_| anyhow!("module.json is not valid"))?;
    if v.get("id").and_then(Value::as_str) != Some(def.id) {
        bail!("module.json is not the manifest of {}", def.name);
    }
    if !matches!(v.get("version").and_then(Value::as_str), Some(s) if !s.is_empty()) {
        bail!("module.json has no version");
    }
    for field in ["files", "components"] {
        let Some(list) = v.get(field).and_then(Value::as_array) else {
            continue;
        };
        for c in list {
            let (Some(p), Some(want)) = (
                c.get("path").and_then(Value::as_str),
                c.get("sha256").and_then(Value::as_str),
            ) else {
                continue;
            };
            let p = safe_rel(p).ok_or_else(|| anyhow!("module.json lists an unsafe path ({p})"))?;
            let got = file_hash(&staged.join(&p))
                .ok_or_else(|| anyhow!("the package lacks {p} listed in module.json"))?;
            if !got.eq_ignore_ascii_case(want) {
                bail!("{p} does not match module.json");
            }
        }
    }
    Ok(())
}

/// Installs the package into the module's folder; the bytes must be the ones `sha256` names.
pub fn install_module(
    root: &Path,
    data: &Path,
    def: &ModuleDef,
    zip: &[u8],
    sha256: &str,
    version: &str,
) -> Result<ModuleRecord> {
    if sha256_hex(zip) != sha256.to_ascii_uppercase() {
        bail!("the package does not match its SHA-256");
    }
    let plan = plan_install(root, data, def, zip)?;
    if !plan.conflicts.is_empty() {
        bail!(
            "these files already exist in the OMSI 2 folder and are not {}'s: {}",
            def.name,
            plan.conflicts.join(", ")
        );
    }
    if plan.files.is_empty() {
        bail!("the package is empty");
    }
    let files = package_files(def, zip)?;
    let dir = module_dir(def.id);
    let stage = super::config::omsi2_dir_in(data)
        .join("staging")
        .join(format!(
            "module-{}-{}-{}",
            def.id,
            safe_name(version),
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&stage);
    let staged = stage.join("new");
    let backup = stage.join("old");
    std::fs::create_dir_all(&staged)?;
    let result = (|| -> Result<ModuleRecord> {
        // extract with a real size count (the headers could lie)
        let mut a = open_zip(zip)?;
        let mut written = 0u64;
        for (i, full, _) in &files {
            let mut f = a.by_index(*i)?;
            let inner = &full[dir.len() + 1..];
            let dst = staged.join(inner);
            std::fs::create_dir_all(dst.parent().unwrap())?;
            let mut out = std::fs::File::create(&dst)?;
            written += std::io::copy(&mut (&mut f).take(MAX_UNPACKED + 1), &mut out)?;
            if written > MAX_UNPACKED {
                bail!("the package unpacks to too much data");
            }
        }
        check_manifest(def, &staged)?;
        // files of the release being replaced that the new one no longer has
        let keep: BTreeSet<String> = plan.files.iter().map(|f| key(&f.path)).collect();
        let obsolete: Vec<String> = owned_list(data, root, def.id)
            .into_iter()
            .filter(|p| !keep.contains(&key(p)) && root.join(p).is_file())
            .collect();
        for p in &obsolete {
            if let Some(link) = linked_part(root, p) {
                bail!("{link} is a link: nothing is removed through links");
            }
        }
        let prev_dirs = load_record(data, root, def.id)
            .map(|r| r.dirs)
            .unwrap_or_default();
        let mut created: Vec<String> = Vec::new();
        let mut replaced: Vec<String> = Vec::new();
        let mut dropped: Vec<String> = Vec::new();
        let mut made_dirs: Vec<String> = Vec::new();
        let prev_records = std::fs::read(records_path(data)).ok();
        let moved = (|| -> Result<ModuleRecord> {
            for f in &plan.files {
                let target = root.join(&f.path);
                if target.exists() {
                    let b = backup.join(&f.path);
                    std::fs::create_dir_all(b.parent().unwrap())?;
                    std::fs::copy(&target, &b)?;
                    replaced.push(f.path.clone());
                } else {
                    created.push(f.path.clone());
                    for d in missing_dirs(root, &f.path) {
                        if !made_dirs.contains(&d) {
                            made_dirs.push(d);
                        }
                    }
                }
                copy_into(&staged.join(&f.path[dir.len() + 1..]), &target)?;
            }
            for p in &obsolete {
                if let Some(link) = linked_part(root, p) {
                    bail!("{link} is a link: nothing is removed through links");
                }
                let target = root.join(p);
                let b = backup.join(p);
                std::fs::create_dir_all(b.parent().unwrap())?;
                std::fs::copy(&target, &b)?;
                std::fs::remove_file(&target)
                    .with_context(|| format!("removing {}", target.display()))?;
                dropped.push(p.clone());
            }
            // the record is part of the transaction: a failure here restores the old install
            let mut dirs = prev_dirs.clone();
            for d in &made_dirs {
                if !dirs.contains(d) {
                    dirs.push(d.clone());
                }
            }
            let rec = ModuleRecord {
                root: root.to_string_lossy().into_owned(),
                version: version.to_string(),
                sha256: sha256.to_ascii_uppercase(),
                files: plan.files.iter().map(|f| f.path.clone()).collect(),
                dirs,
            };
            let mut all = load_records(data);
            all.modules.insert(def.id.to_string(), rec.clone());
            save_records(data, &all)?;
            Ok(rec)
        })();
        match moved {
            Ok(rec) => Ok(rec),
            Err(e) => {
                // the old files first, then what was new, then the folders made for it
                for p in replaced.iter().chain(&dropped) {
                    let _ = std::fs::copy(backup.join(p), root.join(p));
                }
                for p in &created {
                    let _ = std::fs::remove_file(root.join(p));
                }
                for d in made_dirs.iter().rev() {
                    let _ = std::fs::remove_dir(root.join(d));
                }
                match prev_records {
                    Some(b) => {
                        let _ = super::config::write_atomic(&records_path(data), &b);
                    }
                    None => {
                        let _ = std::fs::remove_file(records_path(data));
                    }
                }
                Err(e)
            }
        }
    })();
    let _ = std::fs::remove_dir_all(&stage);
    result
}

// ---------------------------------------------------------------------------------------
// removal

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModuleRemovePlan {
    pub files: Vec<String>,
}

pub fn plan_remove(root: &Path, data: &Path, id: &str) -> ModuleRemovePlan {
    let mut files: Vec<String> = owned_list(data, root, id)
        .into_iter()
        .filter(|p| root.join(p).exists())
        .collect();
    files.sort();
    ModuleRemovePlan { files }
}

/// Removes the files Depot installed for the module, all or none. Whatever else is in the
/// module's folder (the data it learned, a file the player added) stays, and so does the
/// folder then.
pub fn remove_module(root: &Path, data: &Path, id: &str) -> Result<ModuleRemovePlan> {
    let plan = plan_remove(root, data, id);
    if let Some(link) = plan.files.iter().find_map(|p| linked_part(root, p)) {
        bail!("{link} is a link: nothing is removed through links");
    }
    let stage = super::config::omsi2_dir_in(data)
        .join("staging")
        .join(format!(
            "remove-module-{}-{}",
            safe_name(id),
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&stage);
    let mut gone: Vec<&String> = Vec::new();
    for p in &plan.files {
        let t = root.join(p);
        let step = (|| -> Result<()> {
            let b = stage.join(p);
            std::fs::create_dir_all(b.parent().unwrap())?;
            std::fs::copy(&t, &b)?;
            std::fs::remove_file(&t).with_context(|| format!("removing {}", t.display()))
        })();
        if let Err(e) = step {
            for q in &gone {
                let _ = std::fs::copy(stage.join(q), root.join(q));
            }
            let _ = std::fs::remove_dir_all(&stage);
            return Err(e);
        }
        gone.push(p);
    }
    // the record goes while the copies still exist: if it cannot be written, the files return
    let dirs = load_record(data, root, id)
        .map(|r| r.dirs)
        .unwrap_or_default();
    // (an entry of another OMSI 2 folder stays)
    if load_record(data, root, id).is_some() {
        let mut all = load_records(data);
        all.modules.remove(id);
        if let Err(e) = save_records(data, &all) {
            for q in &gone {
                let _ = std::fs::copy(stage.join(q), root.join(q));
            }
            let _ = std::fs::remove_dir_all(&stage);
            return Err(e);
        }
    }
    let _ = std::fs::remove_dir_all(&stage);
    // only folders Depot made, and only when empty, go
    let mut dirs: Vec<String> = dirs.into_iter().filter(|d| safe_rel(d).is_some()).collect();
    dirs.sort_by_key(|d| std::cmp::Reverse(d.matches('/').count()));
    for d in dirs {
        if linked_part(root, &d).is_none() {
            let _ = std::fs::remove_dir(root.join(&d));
        }
    }
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::io::Write;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("depot-oe-mod-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// An OMSI 2 folder in miniature.
    fn omsi(name: &str) -> (PathBuf, PathBuf) {
        let root = tmp(name);
        std::fs::create_dir_all(root.join("plugins")).unwrap();
        (root, tmp(&format!("{name}-data")))
    }

    fn def() -> &'static ModuleDef {
        module_def("omsi-prefetch").unwrap()
    }

    fn manifest(extra: &str) -> String {
        format!(r#"{{"id":"omsi-prefetch","version":"1.0.0-beta.1"{extra}}}"#)
    }

    fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let o = zip::write::SimpleFileOptions::default();
        for (n, b) in entries {
            w.start_file(*n, o).unwrap();
            w.write_all(b).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    /// A good package: module.json, a program and a nested data file.
    fn package() -> Vec<u8> {
        let m = manifest("");
        zip_of(&[
            ("module.json", m.as_bytes()),
            ("bin/prefetch.dll", b"dll"),
            ("docs/NOTICE.txt", b"gpl"),
        ])
    }

    fn put(root: &Path, zip: &[u8], data: &Path, version: &str) -> Result<ModuleRecord> {
        install_module(root, data, def(), zip, &sha256_hex(zip), version)
    }

    /// A release feed and its files served from memory (the seam the real client plugs into).
    struct Feed(HashMap<String, Vec<u8>>);

    impl Fetch for Feed {
        fn get(
            &self,
            url: &str,
            max: u64,
            progress: &mut dyn FnMut(u64, Option<u64>) -> bool,
        ) -> Result<Vec<u8>> {
            let b = self.0.get(url).ok_or_else(|| anyhow!("404 {url}"))?;
            if b.len() as u64 > max {
                bail!("the download is larger than allowed");
            }
            if !progress(b.len() as u64, Some(b.len() as u64)) {
                bail!("cancelled");
            }
            Ok(b.clone())
        }
    }

    const BASE: &str = "https://github.com/OmsiModsBR/omsi-prefetch/releases/download/v1";

    fn feed(zip: &[u8], sha_line: &str) -> Feed {
        let mut m = HashMap::new();
        let json = format!(
            r#"[{{"tag_name":"v1","html_url":"https://github.com/OmsiModsBR/omsi-prefetch/releases/tag/v1","assets":[
              {{"name":"omsi-prefetch-1.0.0.zip","browser_download_url":"{BASE}/omsi-prefetch-1.0.0.zip"}},
              {{"name":"omsi-prefetch-1.0.0.zip.sha256","browser_download_url":"{BASE}/omsi-prefetch-1.0.0.zip.sha256"}}]}}]"#
        );
        m.insert(install::releases_url_of(def().repo), json.into_bytes());
        m.insert(format!("{BASE}/omsi-prefetch-1.0.0.zip"), zip.to_vec());
        m.insert(
            format!("{BASE}/omsi-prefetch-1.0.0.zip.sha256"),
            sha_line.as_bytes().to_vec(),
        );
        Feed(m)
    }

    #[test]
    fn the_release_comes_from_the_modules_own_repository() {
        let z = package();
        let f = feed(&z, &format!("{}  omsi-prefetch-1.0.0.zip", sha256_hex(&z)));
        let rel = latest_release(&f, def()).unwrap();
        assert_eq!(rel.version, "1.0.0");
        let (bytes, sha) = download(&f, def(), &rel, &mut |_, _| true).unwrap();
        assert_eq!(bytes, z);
        assert_eq!(sha, sha256_hex(&z));
        // a release of another repository (even OmsiLaunch's) is not this module's
        let mut other = rel.clone();
        other.zip_url = other.zip_url.replace("omsi-prefetch/", "OmsiLaunch/");
        assert!(download(&f, def(), &other, &mut |_, _| true).is_err());
        // an unknown module has no definition
        assert!(module_def("something-else").is_none());
    }

    #[test]
    fn a_wrong_checksum_or_a_cancelled_download_installs_nothing() {
        let z = package();
        let f = feed(&z, &format!("{}  x", "0".repeat(64)));
        let rel = latest_release(&f, def()).unwrap();
        assert!(download(&f, def(), &rel, &mut |_, _| true).is_err());
        let f = feed(&z, "not a hash");
        assert!(download(&f, def(), &rel, &mut |_, _| true).is_err());
        let f = feed(&z, &format!("{}  x", sha256_hex(&z)));
        assert!(download(&f, def(), &rel, &mut |_, _| false).is_err());
        // a package that is not the one its hash names never reaches the folder
        let (root, data) = omsi("badhash");
        assert!(install_module(&root, &data, def(), &z, &"0".repeat(64), "1").is_err());
        assert!(!root.join(module_dir("omsi-prefetch")).exists());
        assert!(!records_path(&data).exists());
    }

    #[test]
    fn install_and_remove_leave_the_folder_as_it_was() {
        let (root, data) = omsi("roundtrip");
        std::fs::write(root.join("plugins/other.opl"), b"x").unwrap();
        let z = package();
        let plan = plan_install(&root, &data, def(), &z).unwrap();
        assert!(plan.ok());
        assert_eq!(plan.files.len(), 3);
        assert!(plan.files.iter().all(|f| f
            .path
            .starts_with("plugins/OmsiLaunch.Modules/omsi-prefetch/")));
        let rec = put(&root, &z, &data, "1.0.0").unwrap();
        let dir = root.join("plugins/OmsiLaunch.Modules/omsi-prefetch");
        assert_eq!(std::fs::read(dir.join("bin/prefetch.dll")).unwrap(), b"dll");
        assert!(dir.join("module.json").is_file());
        assert_eq!(rec.files.len(), 3);
        assert_eq!(load_record(&data, &root, "omsi-prefetch").unwrap(), rec);
        // nothing of the staging area is left
        assert!(!super::super::config::omsi2_dir_in(&data)
            .join("staging")
            .join(format!("module-omsi-prefetch-1.0.0-{}", std::process::id()))
            .exists());
        let gone = remove_module(&root, &data, "omsi-prefetch").unwrap();
        assert_eq!(gone.files.len(), 3);
        assert!(!root.join("plugins/OmsiLaunch.Modules").exists());
        assert!(root.join("plugins/other.opl").is_file());
        assert!(load_record(&data, &root, "omsi-prefetch").is_none());
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&data);
    }

    #[test]
    fn a_package_wrapped_in_the_modules_folder_is_accepted() {
        let (root, data) = omsi("wrapped");
        let m = manifest("");
        let z = zip_of(&[
            ("omsi-prefetch/module.json", m.as_bytes()),
            ("omsi-prefetch/a.dll", b"a"),
        ]);
        put(&root, &z, &data, "1").unwrap();
        assert!(root
            .join("plugins/OmsiLaunch.Modules/omsi-prefetch/a.dll")
            .is_file());
    }

    #[test]
    fn unsafe_packages_are_refused() {
        let (root, data) = omsi("unsafe");
        let m = manifest("");
        for bad in [
            "../evil.dll",
            "/abs.dll",
            "C:/x.dll",
            "a\\b.dll",
            "bin/../../x.dll",
            "NUL",
            "aux.dll",
            "COM0",
            "trailing.",
        ] {
            let z = zip_of(&[("module.json", m.as_bytes()), (bad, b"x")]);
            assert!(plan_install(&root, &data, def(), &z).is_err(), "{bad}");
        }
        // the same file twice, or twice by case
        let z = zip_of(&[
            ("module.json", m.as_bytes()),
            ("a.dll", b"1"),
            ("A.DLL", b"2"),
        ]);
        assert!(plan_install(&root, &data, def(), &z).is_err());
        // no module.json
        let z = zip_of(&[("a.dll", b"1")]);
        assert!(plan_install(&root, &data, def(), &z).is_err());
        // nothing was written anywhere
        assert!(!root.join("plugins/OmsiLaunch.Modules").exists());
        assert!(!root.join("evil.dll").exists());
        // not an OMSI 2 folder
        let empty = tmp("noplugins");
        assert!(plan_install(&empty, &data, def(), &package()).is_err());
    }

    #[test]
    fn module_json_must_be_this_modules_and_its_hashes_must_match() {
        let (root, data) = omsi("manifest");
        let wrong = zip_of(&[("module.json", br#"{"id":"openhud","version":"1"}"#)]);
        assert!(put(&root, &wrong, &data, "1").is_err());
        let nover = zip_of(&[("module.json", br#"{"id":"omsi-prefetch"}"#)]);
        assert!(put(&root, &nover, &data, "1").is_err());
        let junk = zip_of(&[("module.json", b"not json")]);
        assert!(put(&root, &junk, &data, "1").is_err());
        let bad_hash = manifest(&format!(
            r#","files":[{{"path":"a.dll","sha256":"{}"}}]"#,
            "0".repeat(64)
        ));
        let z = zip_of(&[("module.json", bad_hash.as_bytes()), ("a.dll", b"a")]);
        assert!(put(&root, &z, &data, "1").is_err());
        let missing = manifest(&format!(
            r#","components":[{{"path":"gone.dll","sha256":"{}"}}]"#,
            sha256_hex(b"x")
        ));
        let z = zip_of(&[("module.json", missing.as_bytes())]);
        assert!(put(&root, &z, &data, "1").is_err());
        let good = manifest(&format!(
            r#","files":[{{"path":"a.dll","sha256":"{}"}}]"#,
            sha256_hex(b"a").to_ascii_lowercase()
        ));
        let z = zip_of(&[("module.json", good.as_bytes()), ("a.dll", b"a")]);
        put(&root, &z, &data, "1").unwrap();
        assert!(!root
            .join("plugins/OmsiLaunch.Modules/omsi-prefetch")
            .join("x")
            .exists());
    }

    #[test]
    fn a_file_that_is_not_ours_is_never_overwritten() {
        let (root, data) = omsi("conflict");
        let dir = root.join("plugins/OmsiLaunch.Modules/omsi-prefetch");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("module.json"), b"someone else's").unwrap();
        let z = package();
        let plan = plan_install(&root, &data, def(), &z).unwrap();
        assert!(!plan.ok());
        assert_eq!(plan.conflicts.len(), 1);
        assert!(put(&root, &z, &data, "1").is_err());
        assert_eq!(
            std::fs::read(dir.join("module.json")).unwrap(),
            b"someone else's"
        );
        assert!(!dir.join("bin").exists());
        // a record for another OMSI 2 folder does not make it ours either
        let other = tmp("conflict-other");
        let mut all = Records::default();
        all.modules.insert(
            "omsi-prefetch".into(),
            ModuleRecord {
                root: other.to_string_lossy().into_owned(),
                version: "1".into(),
                sha256: "0".into(),
                files: vec![format!("{MODULES_DIR}/omsi-prefetch/module.json")],
                dirs: vec![],
            },
        );
        save_records(&data, &all).unwrap();
        assert!(!plan_install(&root, &data, def(), &z).unwrap().ok());
        assert!(remove_module(&root, &data, "omsi-prefetch")
            .unwrap()
            .files
            .is_empty());
        assert_eq!(
            std::fs::read(dir.join("module.json")).unwrap(),
            b"someone else's"
        );
        // the other folder's record is still there
        assert!(load_record(&data, &other, "omsi-prefetch").is_some());
    }

    #[test]
    fn an_update_replaces_what_is_ours_and_drops_what_the_new_release_lacks() {
        let (root, data) = omsi("update");
        put(&root, &package(), &data, "1.0.0").unwrap();
        let dir = root.join("plugins/OmsiLaunch.Modules/omsi-prefetch");
        // data the module learned and a file of the player's stay through update and removal
        std::fs::create_dir_all(dir.join("learned")).unwrap();
        std::fs::write(dir.join("learned/map.bin"), b"data").unwrap();
        let m = manifest("");
        let z2 = zip_of(&[("module.json", m.as_bytes()), ("bin/prefetch.dll", b"v2")]);
        let plan = plan_install(&root, &data, def(), &z2).unwrap();
        assert!(plan.ok());
        assert_eq!(plan.replaces.len(), 2);
        put(&root, &z2, &data, "2.0.0").unwrap();
        assert_eq!(std::fs::read(dir.join("bin/prefetch.dll")).unwrap(), b"v2");
        assert!(!dir.join("docs/NOTICE.txt").exists());
        assert_eq!(std::fs::read(dir.join("learned/map.bin")).unwrap(), b"data");
        assert_eq!(
            load_record(&data, &root, "omsi-prefetch").unwrap().version,
            "2.0.0"
        );
        remove_module(&root, &data, "omsi-prefetch").unwrap();
        assert_eq!(std::fs::read(dir.join("learned/map.bin")).unwrap(), b"data");
        assert!(!dir.join("module.json").exists());
        assert!(load_record(&data, &root, "omsi-prefetch").is_none());
    }

    #[test]
    fn a_failure_half_way_puts_everything_back() {
        let (root, data) = omsi("rollback");
        let dir = root.join("plugins/OmsiLaunch.Modules/omsi-prefetch");
        std::fs::create_dir_all(&dir).unwrap();
        // `sub` is a file, so `sub/x.txt` cannot be written after module.json went in
        std::fs::write(dir.join("sub"), b"in the way").unwrap();
        let m = manifest("");
        let z = zip_of(&[("module.json", m.as_bytes()), ("sub/x.txt", b"x")]);
        let before = std::fs::read(records_path(&data)).ok();
        assert!(put(&root, &z, &data, "1").is_err());
        assert!(!dir.join("module.json").exists());
        assert_eq!(std::fs::read(dir.join("sub")).unwrap(), b"in the way");
        assert_eq!(std::fs::read(records_path(&data)).ok(), before);
        assert!(load_record(&data, &root, "omsi-prefetch").is_none());
    }

    #[test]
    fn a_removal_whose_record_cannot_be_updated_keeps_the_module() {
        let (root, data) = omsi("recfail");
        put(&root, &package(), &data, "1").unwrap();
        // a folder where the atomic write puts its temporary file makes the next save fail
        let rp = records_path(&data);
        let blocker = rp.with_file_name(format!(
            ".{}.{}.tmp",
            rp.file_name().unwrap().to_string_lossy(),
            std::process::id()
        ));
        std::fs::create_dir_all(&blocker).unwrap();
        assert!(remove_module(&root, &data, "omsi-prefetch").is_err());
        let dir = root.join("plugins/OmsiLaunch.Modules/omsi-prefetch");
        assert_eq!(std::fs::read(dir.join("bin/prefetch.dll")).unwrap(), b"dll");
        assert!(dir.join("module.json").is_file());
        assert!(dir.join("docs/NOTICE.txt").is_file());
        assert!(load_record(&data, &root, "omsi-prefetch").is_some());
        // and once the record can be written again the removal goes through
        std::fs::remove_dir_all(&blocker).unwrap();
        assert_eq!(
            remove_module(&root, &data, "omsi-prefetch")
                .unwrap()
                .files
                .len(),
            3
        );
    }

    #[test]
    fn modules_do_not_touch_each_other() {
        let (root, data) = omsi("two");
        put(&root, &package(), &data, "1").unwrap();
        let hud = module_def("openhud").unwrap();
        let m = r#"{"id":"openhud","version":"0.1.0"}"#;
        let z = zip_of(&[("module.json", m.as_bytes()), ("h.dll", b"h")]);
        install_module(&root, &data, hud, &z, &sha256_hex(&z), "0.1.0").unwrap();
        remove_module(&root, &data, "omsi-prefetch").unwrap();
        assert!(root
            .join("plugins/OmsiLaunch.Modules/openhud/h.dll")
            .is_file());
        // the shared folder stays while the other module is in it
        assert!(root.join("plugins/OmsiLaunch.Modules").is_dir());
        remove_module(&root, &data, "openhud").unwrap();
        assert!(load_record(&data, &root, "openhud").is_none());
    }

    #[test]
    #[cfg(unix)]
    fn nothing_is_installed_or_removed_through_a_link() {
        let (root, data) = omsi("linked");
        let outside = tmp("linked-out");
        std::fs::create_dir_all(root.join("plugins/OmsiLaunch.Modules")).unwrap();
        std::os::unix::fs::symlink(
            &outside,
            root.join("plugins/OmsiLaunch.Modules/omsi-prefetch"),
        )
        .unwrap();
        assert!(put(&root, &package(), &data, "1").is_err());
        assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 0);
        // a record that points through a link removes nothing
        let rec = ModuleRecord {
            root: root.to_string_lossy().into_owned(),
            version: "1".into(),
            sha256: "0".into(),
            files: vec![format!("{MODULES_DIR}/omsi-prefetch/keep.txt")],
            dirs: vec![],
        };
        std::fs::write(outside.join("keep.txt"), b"k").unwrap();
        let mut all = Records::default();
        all.modules.insert("omsi-prefetch".into(), rec);
        save_records(&data, &all).unwrap();
        assert!(remove_module(&root, &data, "omsi-prefetch").is_err());
        assert!(outside.join("keep.txt").exists());
        // `plugins` itself a link
        let (root2, data2) = omsi("linked2");
        let real = tmp("linked2-real");
        std::fs::remove_dir_all(root2.join("plugins")).unwrap();
        std::os::unix::fs::symlink(&real, root2.join("plugins")).unwrap();
        assert!(put(&root2, &package(), &data2, "1").is_err());
        assert_eq!(std::fs::read_dir(&real).unwrap().count(), 0);
    }

    #[test]
    fn a_record_with_unsafe_paths_owns_nothing() {
        let (root, data) = omsi("evilrec");
        std::fs::write(root.join("Omsi.exe"), b"game").unwrap();
        let rec = ModuleRecord {
            root: root.to_string_lossy().into_owned(),
            version: "1".into(),
            sha256: "0".into(),
            files: vec![
                "Omsi.exe".into(),
                "plugins/OmsiLaunch.Modules/omsi-prefetch/../../../Omsi.exe".into(),
                "plugins/OmsiLaunch.Modules/openhud/x.dll".into(),
            ],
            dirs: vec!["..".into()],
        };
        let mut all = Records::default();
        all.modules.insert("omsi-prefetch".into(), rec);
        save_records(&data, &all).unwrap();
        assert!(owned_list(&data, &root, "omsi-prefetch").is_empty());
        remove_module(&root, &data, "omsi-prefetch").unwrap();
        assert_eq!(std::fs::read(root.join("Omsi.exe")).unwrap(), b"game");
    }
}
