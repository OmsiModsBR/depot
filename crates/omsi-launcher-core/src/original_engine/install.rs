//! Downloading, verifying, installing and removing OmsiLaunch, always on the player's
//! confirmation (the dialog shows `InstallPlan`).
//!
//! The package is extracted into Depot's own staging folder first and checked against its
//! own `release-manifest.json`; only then are its files moved into the OMSI 2 folder. A file
//! that already exists there and is not part of an earlier OmsiLaunch is never overwritten
//! (the install is refused and says which), and a failure half way puts everything back.

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

/// The official repository; nothing else is ever downloaded from.
pub const REPO: &str = "OmsiModsBR/OmsiLaunch";
pub const LICENSE: &str = "LGPL-3.0-only";

const MAX_PACKAGE: u64 = 300 * 1024 * 1024;
const MAX_ENTRIES: usize = 6000;
const MAX_UNPACKED: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReleaseInfo {
    pub version: String,
    pub zip_name: String,
    pub zip_url: String,
    pub sha_url: String,
    pub page_url: String,
}

/// The newest published release of the list the API gives (drafts skipped; a beta counts,
/// the project only has betas so far) that carries the package and its `.sha256`.
pub fn parse_releases(json: &str) -> Option<ReleaseInfo> {
    let v: Value = serde_json::from_str(json).ok()?;
    let list = match &v {
        Value::Array(a) => a.clone(),
        o @ Value::Object(_) => vec![o.clone()],
        _ => return None,
    };
    for r in list {
        if r.get("draft").and_then(Value::as_bool).unwrap_or(false) {
            continue;
        }
        let tag = r
            .get("tag_name")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let assets = r.get("assets").and_then(Value::as_array)?.clone();
        let find = |pred: &dyn Fn(&str) -> bool| {
            assets.iter().find_map(|a| {
                let n = a.get("name")?.as_str()?;
                let url = a.get("browser_download_url")?.as_str()?;
                pred(n).then(|| (n.to_string(), url.to_string()))
            })
        };
        let zip = find(&|n| {
            n.starts_with("OmsiLaunch-") && n.ends_with(".zip") && !n.contains("current")
        });
        let Some((zip_name, zip_url)) = zip else {
            continue;
        };
        let want = format!("{zip_name}.sha256");
        let Some((_, sha_url)) = find(&|n| n == want) else {
            continue;
        };
        let version = zip_name
            .trim_start_matches("OmsiLaunch-")
            .trim_end_matches(".zip")
            .to_string();
        let version = if version.is_empty() {
            tag.trim_start_matches('v').to_string()
        } else {
            version
        };
        return Some(ReleaseInfo {
            version,
            zip_name,
            zip_url,
            sha_url,
            page_url: r
                .get("html_url")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        });
    }
    None
}

/// The hash in a `.sha256` file (`<HEX>  <name>`), checked to be 64 hex digits.
pub fn parse_sha256_file(text: &str) -> Option<String> {
    let first = text
        .trim_start_matches('\u{feff}')
        .split_whitespace()
        .next()?;
    (first.len() == 64 && first.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| first.to_ascii_uppercase())
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:X}", Sha256::digest(bytes))
}

/// Where the bytes of a URL come from. `progress(done, total)` returns false to cancel.
pub trait Fetch {
    fn get(
        &self,
        url: &str,
        max: u64,
        progress: &mut dyn FnMut(u64, Option<u64>) -> bool,
    ) -> Result<Vec<u8>>;
}

/// Hosts a release download may come from or pass through (HTTPS only).
pub fn host_allowed(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if host.contains('@') || host.contains(':') {
        return false;
    }
    matches!(
        host.to_ascii_lowercase().as_str(),
        "api.github.com"
            | "github.com"
            | "objects.githubusercontent.com"
            | "release-assets.githubusercontent.com"
    )
}

/// The URL of a file attached to a release of the official repository (the hosts a redirect
/// may pass through are checked apart, hop by hop).
pub fn official_asset(url: &str) -> bool {
    host_allowed(url) && url.starts_with(&format!("https://github.com/{REPO}/releases/download/"))
}

pub fn releases_url() -> String {
    format!("https://api.github.com/repos/{REPO}/releases?per_page=10")
}

pub fn latest_release(f: &dyn Fetch) -> Result<ReleaseInfo> {
    let body = f.get(&releases_url(), 4 * 1024 * 1024, &mut |_, _| true)?;
    parse_releases(&String::from_utf8_lossy(&body))
        .ok_or_else(|| anyhow!("no OmsiLaunch release with a package and a checksum was found"))
}

/// Downloads the package and its checksum and returns the package once the hash matches.
pub fn download(
    f: &dyn Fetch,
    rel: &ReleaseInfo,
    progress: &mut dyn FnMut(u64, Option<u64>) -> bool,
) -> Result<(Vec<u8>, String)> {
    if !official_asset(&rel.zip_url) || !official_asset(&rel.sha_url) {
        bail!("the release points outside the official download hosts");
    }
    let sha_text = f.get(&rel.sha_url, 4096, &mut |_, _| true)?;
    let want = parse_sha256_file(&String::from_utf8_lossy(&sha_text))
        .ok_or_else(|| anyhow!("the checksum file is not a SHA-256"))?;
    let zip = f.get(&rel.zip_url, MAX_PACKAGE, progress)?;
    let got = sha256_hex(&zip);
    if got != want {
        bail!("the downloaded package does not match its SHA-256 (expected {want}, got {got})");
    }
    Ok((zip, want))
}

// ---------------------------------------------------------------------------------------
// what an install would add

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlannedFile {
    pub path: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstallPlan {
    pub files: Vec<PlannedFile>,
    pub total_bytes: u64,
    /// Files of an earlier OmsiLaunch this install replaces.
    pub replaces: Vec<String>,
    /// Existing files that are not OmsiLaunch's: the install is refused while there are any.
    pub conflicts: Vec<String>,
}

impl InstallPlan {
    pub fn ok(&self) -> bool {
        self.conflicts.is_empty() && !self.files.is_empty()
    }
}

/// `plugins/OmsiLaunch.dll` style, no way out of the folder: the one form of a path inside
/// a package or manifest that is accepted.
pub fn safe_rel(p: &str) -> Option<String> {
    if p.is_empty() || p.contains('\\') || p.contains(':') || p.starts_with('/') {
        return None;
    }
    let ok = Path::new(p)
        .components()
        .all(|c| matches!(c, Component::Normal(_)));
    (ok && p.split('/').all(windows_name_ok)).then(|| p.to_string())
}

/// A file or folder name Windows keeps as written: no trailing dot or space (stripped),
/// none of `<>"|?*` or control characters, and not a device name (`NUL`, `aux.dll`).
fn windows_name_ok(n: &str) -> bool {
    if n.is_empty()
        || n.ends_with('.')
        || n.ends_with(' ')
        || n.chars().any(|c| c.is_control() || "<>\"|?*".contains(c))
    {
        return false;
    }
    let stem = n.split('.').next().unwrap_or_default().trim_end();
    let up = stem.to_ascii_uppercase();
    let numbered = |p: &str| {
        up.strip_prefix(p).is_some_and(|d| {
            matches!(
                d.chars().collect::<Vec<_>>()[..],
                ['0'..='9' | '¹' | '²' | '³']
            )
        })
    };
    !(matches!(
        up.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || numbered("COM")
        || numbered("LPT"))
}

fn key(p: &str) -> String {
    p.to_ascii_lowercase()
}

/// Whether `p` is a symbolic link or, on Windows, any reparse point (a junction too).
fn is_link(p: &Path) -> bool {
    let Ok(m) = std::fs::symlink_metadata(p) else {
        return false;
    };
    if m.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const REPARSE_POINT: u32 = 0x400;
        if m.file_attributes() & REPARSE_POINT != 0 {
            return true;
        }
    }
    false
}

/// The first existing folder (or the file itself) on the way from `root` to `rel` that is a
/// link: a package path that looks safe would leave the OMSI 2 folder through it.
fn linked_part(root: &Path, rel: &str) -> Option<String> {
    let mut cur = root.to_path_buf();
    for part in rel.split('/') {
        cur.push(part);
        if is_link(&cur) {
            return Some(cur.display().to_string());
        }
        if !cur.exists() {
            break;
        }
    }
    None
}

/// The files Depot put in the folder, in the case it wrote them. The record Depot keeps
/// outside the OMSI 2 folder is the authority; the manifest inside the folder is not (any
/// program can write one), so for an install made without a record only the files that
/// are exactly what the manifest says count.
pub fn owned_list(root: &Path, data: &Path) -> Vec<String> {
    // Without a record nothing is owned: a manifest in the folder can be written by anyone,
    // hashes of real files included, so it never gives the right to replace or delete.
    match load_record(data).filter(|r| same_root(Path::new(&r.root), root)) {
        Some(r) => r.files.into_iter().filter_map(|p| safe_rel(&p)).collect(),
        None => Vec::new(),
    }
}

/// Whether two folder paths are the same folder as far as spelling goes: no trailing
/// separator, and on Windows no difference of case or of `/` and `\\`.
fn same_root(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| -> Vec<String> {
        p.to_string_lossy()
            .replace('\\', "/")
            .split('/')
            .filter(|c| !c.is_empty())
            .map(|c| {
                if cfg!(windows) {
                    c.to_lowercase()
                } else {
                    c.to_string()
                }
            })
            .collect()
    };
    norm(a) == norm(b)
}

fn owned_paths(root: &Path, data: &Path) -> BTreeSet<String> {
    owned_list(root, data).iter().map(|p| key(p)).collect()
}

/// `(path, sha256)` of every file a manifest lists.
pub fn read_manifest(path: &Path) -> Option<Vec<(String, String)>> {
    let text = std::fs::read_to_string(path).ok()?;
    manifest_files(&text)
}

pub fn manifest_files(text: &str) -> Option<Vec<(String, String)>> {
    let v: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    let files = v.get("files")?.as_array()?;
    let mut out = Vec::new();
    for f in files {
        let p = safe_rel(f.get("path")?.as_str()?)?;
        out.push((p, f.get("sha256")?.as_str()?.to_ascii_uppercase()));
    }
    Some(out)
}

fn open_zip(zip: &[u8]) -> Result<zip::ZipArchive<std::io::Cursor<&[u8]>>> {
    let a =
        zip::ZipArchive::new(std::io::Cursor::new(zip)).context("the package is not a zip file")?;
    if a.len() > MAX_ENTRIES {
        bail!("the package holds too many files");
    }
    Ok(a)
}

/// Lists what the package would add, without writing anything.
pub fn plan_install(root: &Path, data: &Path, zip: &[u8]) -> Result<InstallPlan> {
    let mut a = open_zip(zip)?;
    let owned = owned_paths(root, data);
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let (mut files, mut replaces, mut conflicts) = (Vec::new(), Vec::new(), Vec::new());
    let mut total = 0u64;
    for i in 0..a.len() {
        let f = a.by_index(i)?;
        if f.is_dir() {
            continue;
        }
        if f.unix_mode()
            .map(|m| m & 0o170000 == 0o120000)
            .unwrap_or(false)
        {
            bail!("the package holds a link ({})", f.name());
        }
        let path = safe_rel(f.name())
            .ok_or_else(|| anyhow!("the package holds an unsafe path ({})", f.name()))?;
        if !seen.insert(key(&path)) {
            bail!("the package lists {path} twice");
        }
        total = total.saturating_add(f.size());
        if total > MAX_UNPACKED {
            bail!("the package unpacks to too much data");
        }
        if let Some(link) = linked_part(root, &path) {
            bail!("{link} is a link: OmsiLaunch is not installed through links");
        }
        let target = root.join(&path);
        if target.symlink_metadata().is_ok() {
            if owned.contains(&key(&path)) {
                replaces.push(path.clone());
            } else {
                conflicts.push(path.clone());
            }
        }
        files.push(PlannedFile {
            path,
            bytes: f.size(),
        });
    }
    Ok(InstallPlan {
        files,
        total_bytes: total,
        replaces,
        conflicts,
    })
}

/// What Depot added, kept so a removal still works without the manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallRecord {
    pub root: String,
    pub version: String,
    pub sha256: String,
    pub files: Vec<String>,
    /// Folders Depot made for these files (the only ones a removal may take away).
    #[serde(default)]
    pub dirs: Vec<String>,
}

pub fn record_path(data: &Path) -> PathBuf {
    super::config::omsi2_dir_in(data).join("install-record.json")
}

pub fn load_record(data: &Path) -> Option<InstallRecord> {
    serde_json::from_str(&std::fs::read_to_string(record_path(data)).ok()?).ok()
}

fn copy_into(src: &Path, dst: &Path) -> Result<()> {
    if let Some(d) = dst.parent() {
        std::fs::create_dir_all(d)?;
    }
    if std::fs::rename(src, dst).is_err() {
        std::fs::copy(src, dst).with_context(|| format!("writing {}", dst.display()))?;
        let _ = std::fs::remove_file(src);
    }
    Ok(())
}

/// The folders on the way to `rel` that do not exist yet, outermost first.
fn missing_dirs(root: &Path, rel: &str) -> Vec<String> {
    let parts: Vec<&str> = rel.split('/').collect();
    let mut out = Vec::new();
    for n in 1..parts.len() {
        let d = parts[..n].join("/");
        if !root.join(&d).exists() {
            out.push(d);
        }
    }
    out
}

/// Installs the package into the OMSI 2 folder; the bytes must be the ones `sha256` names.
pub fn install(
    root: &Path,
    data: &Path,
    zip: &[u8],
    sha256: &str,
    version: &str,
) -> Result<InstallRecord> {
    if sha256_hex(zip) != sha256.to_ascii_uppercase() {
        bail!("the package does not match its SHA-256");
    }
    let plan = plan_install(root, data, zip)?;
    if !plan.conflicts.is_empty() {
        bail!(
            "these files already exist in the OMSI 2 folder and are not OmsiLaunch's: {}",
            plan.conflicts.join(", ")
        );
    }
    if plan.files.is_empty() {
        bail!("the package is empty");
    }
    let stage = super::config::omsi2_dir_in(data)
        .join("staging")
        .join(format!("{}-{}", safe_name(version), std::process::id()));
    let _ = std::fs::remove_dir_all(&stage);
    let staged = stage.join("new");
    let backup = stage.join("old");
    std::fs::create_dir_all(&staged)?;
    let result = (|| -> Result<InstallRecord> {
        // extract with a real size count (the headers could lie)
        let mut a = open_zip(zip)?;
        let mut written = 0u64;
        for i in 0..a.len() {
            let mut f = a.by_index(i)?;
            if f.is_dir() {
                continue;
            }
            let path = safe_rel(f.name()).ok_or_else(|| anyhow!("unsafe path"))?;
            let dst = staged.join(&path);
            std::fs::create_dir_all(dst.parent().unwrap())?;
            let mut out = std::fs::File::create(&dst)?;
            let n = std::io::copy(&mut (&mut f).take(MAX_UNPACKED + 1), &mut out)?;
            written += n;
            if written > MAX_UNPACKED {
                bail!("the package unpacks to too much data");
            }
        }
        // the package must agree with its own manifest before anything reaches OMSI 2
        let m = read_manifest(&staged.join("release-manifest.json"))
            .ok_or_else(|| anyhow!("the package has no readable manifest"))?;
        for (p, want) in &m {
            let got = file_hash(&staged.join(p)).ok_or_else(|| anyhow!("the package lacks {p}"))?;
            if &got != want {
                bail!("{p} does not match the package's manifest");
            }
        }
        // every file about to be installed must be one the manifest vouches for
        let vouched: BTreeSet<String> = m.iter().map(|(p, _)| key(p)).collect();
        if let Some(f) = plan
            .files
            .iter()
            .find(|f| f.path != "release-manifest.json" && !vouched.contains(&key(&f.path)))
        {
            bail!("{} is not listed in the package's manifest", f.path);
        }
        // files of the release being replaced that the new one no longer has
        let keep: BTreeSet<String> = plan.files.iter().map(|f| key(&f.path)).collect();
        let obsolete: Vec<String> = owned_list(root, data)
            .into_iter()
            .filter(|p| !keep.contains(&key(p)) && root.join(p).is_file())
            .collect();
        for p in &obsolete {
            if let Some(link) = linked_part(root, p) {
                bail!("{link} is a link: nothing is removed through links");
            }
        }
        let prev_dirs = load_record(data)
            .filter(|r| same_root(Path::new(&r.root), root))
            .map(|r| r.dirs)
            .unwrap_or_default();
        // move in; remember what to undo
        let mut created: Vec<String> = Vec::new();
        let mut replaced: Vec<String> = Vec::new();
        let mut dropped: Vec<String> = Vec::new();
        let mut made_dirs: Vec<String> = Vec::new();
        let moved = (|| -> Result<()> {
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
                copy_into(&staged.join(&f.path), &target)?;
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
            dirs.extend(
                made_dirs
                    .iter()
                    .filter(|d| !dirs.contains(d))
                    .cloned()
                    .collect::<Vec<_>>(),
            );
            let rec = InstallRecord {
                root: root.to_string_lossy().into_owned(),
                version: version.to_string(),
                sha256: sha256.to_string(),
                files: plan.files.iter().map(|f| f.path.clone()).collect(),
                dirs,
            };
            super::config::write_atomic(&record_path(data), &serde_json::to_vec_pretty(&rec)?)?;
            Ok(())
        })();
        if let Err(e) = moved {
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
            return Err(e);
        }
        load_record(data).ok_or_else(|| anyhow!("the install record was not written"))
    })();
    let _ = std::fs::remove_dir_all(&stage);
    result
}

fn safe_name(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn file_hash(p: &Path) -> Option<String> {
    let mut f = std::fs::File::open(p).ok()?;
    let mut h = Sha256::new();
    std::io::copy(&mut f, &mut h).ok()?;
    Some(format!("{:X}", h.finalize()))
}

// ---------------------------------------------------------------------------------------
// updates (only ever looked for when the player asks)

fn version_key(v: &str) -> (Vec<u64>, bool, Vec<(u64, String)>) {
    let v = v.trim().trim_start_matches('v');
    let (core, pre) = v.split_once('-').map_or((v, ""), |(a, b)| (a, b));
    let nums = core.split('.').map(|p| p.parse().unwrap_or(0)).collect();
    // "beta4" -> ("beta", 4) so beta10 sorts after beta9
    let pre = pre
        .split(['.', '-'])
        .filter(|p| !p.is_empty())
        .map(|p| {
            let at = p.find(|c: char| c.is_ascii_digit()).unwrap_or(p.len());
            (p[at..].parse().unwrap_or(0), p[..at].to_ascii_lowercase())
        })
        .collect();
    (nums, pre_is_empty(v), pre)
}

fn pre_is_empty(v: &str) -> bool {
    !v.contains('-')
}

/// Whether `latest` is newer than `installed` (a release is newer than its own betas).
pub fn update_available(installed: &str, latest: &str) -> bool {
    version_key(latest) > version_key(installed)
}

/// Whether Depot knows how to drive this OmsiLaunch: the control protocol of the package
/// must be the one the command lines used here belong to.
pub const SUPPORTED_PROTOCOL: &str = "0.1";

pub fn protocol_supported(p: Option<&str>) -> bool {
    p == Some(SUPPORTED_PROTOCOL)
}

// ---------------------------------------------------------------------------------------
// removal

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RemovePlan {
    pub files: Vec<String>,
    /// A session left files changed: it must be recovered before OmsiLaunch can go.
    pub pending_recovery: bool,
}

/// `.omsilaunch` subfolders that are the package's own; the rest (the player's session
/// profiles, module data) stays.
const OWN_DIRS: [&str; 4] = ["assets", "docs", "examples", "diagnostics"];

pub fn plan_remove(root: &Path, data: &Path) -> RemovePlan {
    let mut files: BTreeSet<String> = owned_list(root, data).into_iter().collect();
    files.retain(|p| root.join(p).exists());
    // the manifest goes last, so a failure half way leaves it to find the rest by
    let mut files: Vec<String> = files.into_iter().collect();
    files.sort_by_key(|p| p == "release-manifest.json");
    RemovePlan {
        files,
        pending_recovery: root.join(".omsilaunch").join("journal.json").exists(),
    }
}

pub fn remove(root: &Path, data: &Path) -> Result<RemovePlan> {
    let plan = plan_remove(root, data);
    if plan.pending_recovery {
        bail!("a session left OMSI 2 files changed: restore them first");
    }
    if let Some(link) = plan.files.iter().find_map(|p| linked_part(root, p)) {
        bail!("{link} is a link: nothing is removed through links");
    }
    // all or nothing: a copy of each file is kept until the last one is gone
    let stage = super::config::omsi2_dir_in(data)
        .join("staging")
        .join(format!("remove-{}", std::process::id()));
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
    let _ = std::fs::remove_dir_all(&stage);
    for d in OWN_DIRS {
        let dir = root.join(".omsilaunch").join(d);
        if linked_part(root, &format!(".omsilaunch/{d}")).is_none() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
    // only folders Depot made, and only when empty, go
    let dirs = load_record(data)
        .filter(|r| same_root(Path::new(&r.root), root))
        .map(|r| r.dirs)
        .unwrap_or_default();
    let mut dirs: Vec<String> = dirs.into_iter().filter(|d| safe_rel(d).is_some()).collect();
    dirs.sort_by_key(|d| std::cmp::Reverse(d.matches('/').count()));
    for d in dirs {
        let t = root.join(&d);
        if linked_part(root, &d).is_none() {
            let _ = std::fs::remove_dir(t);
        }
    }
    if linked_part(root, ".omsilaunch").is_none() {
        let _ = std::fs::remove_dir(root.join(".omsilaunch"));
    }
    let _ = std::fs::remove_file(record_path(data));
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("depot-oe-inst-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A package like the real one in miniature: root files, the plugin, a docs file.
    fn package(extra: &[(&str, &[u8])], bad_hash: bool) -> Vec<u8> {
        let mut entries: Vec<(&str, Vec<u8>)> = vec![
            ("OmsiLaunch.exe", b"exe".to_vec()),
            ("OmsiLaunchW.exe", b"exew".to_vec()),
            ("plugins/OmsiLaunch.Plugin.opl", b"opl".to_vec()),
            (".omsilaunch/docs/README.md", b"doc".to_vec()),
        ];
        for (n, b) in extra {
            entries.push((n, b.to_vec()));
        }
        let files: Vec<String> = entries
            .iter()
            .map(|(n, b)| {
                let h = if bad_hash && *n == "OmsiLaunch.exe" {
                    "00".into()
                } else {
                    sha256_hex(b)
                };
                format!(
                    "{{\"path\":\"{n}\",\"bytes\":{},\"sha256\":\"{h}\"}}",
                    b.len()
                )
            })
            .collect();
        let manifest = format!(
            "{{\"product_version\":\"0.1.0-beta4\",\"files\":[{}]}}",
            files.join(",")
        );
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let o = zip::write::SimpleFileOptions::default();
        for (n, b) in &entries {
            w.start_file(*n, o).unwrap();
            w.write_all(b).unwrap();
        }
        w.start_file("release-manifest.json", o).unwrap();
        w.write_all(manifest.as_bytes()).unwrap();
        w.finish().unwrap().into_inner()
    }

    #[test]
    fn release_list_picks_the_newest_complete_release() {
        let json = r#"[
          {"tag_name":"v0.2.0","draft":true,"assets":[]},
          {"tag_name":"v0.1.0-beta4","html_url":"https://github.com/OmsiModsBR/OmsiLaunch/releases/tag/v0.1.0-beta4","assets":[
            {"name":"OmsiLaunch-0.1.0-beta4.zip","browser_download_url":"https://github.com/x/OmsiLaunch-0.1.0-beta4.zip"},
            {"name":"OmsiLaunch-0.1.0-beta4.zip.sha256","browser_download_url":"https://github.com/x/OmsiLaunch-0.1.0-beta4.zip.sha256"}]}]"#;
        let r = parse_releases(json).unwrap();
        assert_eq!(r.version, "0.1.0-beta4");
        assert!(r.sha_url.ends_with(".sha256"));
        // a release without the checksum is not offered
        assert!(parse_releases(r#"[{"tag_name":"v1","assets":[{"name":"OmsiLaunch-1.zip","browser_download_url":"https://github.com/a"}]}]"#).is_none());
    }

    #[test]
    fn versions_compare_like_releases() {
        assert!(update_available("0.1.0-beta4", "0.1.0-beta5"));
        assert!(update_available("0.1.0-beta9", "0.1.0-beta10"));
        assert!(update_available("0.1.0-beta4", "0.1.0"));
        assert!(update_available("0.1.0", "0.2.0-beta1"));
        assert!(!update_available("0.1.0", "0.1.0-beta5"));
        assert!(!update_available("0.1.0-beta4", "0.1.0-beta4"));
        assert!(
            protocol_supported(Some("0.1"))
                && !protocol_supported(Some("0.2"))
                && !protocol_supported(None)
        );
    }

    #[test]
    fn sha_file_and_hosts() {
        let h = "AB".repeat(32);
        assert_eq!(
            parse_sha256_file(&format!("{}  OmsiLaunch-1.zip\n", h.to_lowercase())).unwrap(),
            h
        );
        assert!(parse_sha256_file("nothex  x").is_none());
        assert!(host_allowed("https://objects.githubusercontent.com/a/b"));
        assert!(host_allowed(
            "https://github.com/OmsiModsBR/OmsiLaunch/releases/download/x/y.zip"
        ));
        assert!(!host_allowed("http://github.com/x"));
        assert!(!host_allowed("https://github.com.evil.example/x"));
        assert!(!host_allowed("https://github.com@evil.example/x"));
    }

    struct Fake(Vec<u8>, String);
    impl Fetch for Fake {
        fn get(
            &self,
            url: &str,
            _max: u64,
            p: &mut dyn FnMut(u64, Option<u64>) -> bool,
        ) -> Result<Vec<u8>> {
            if url.ends_with(".sha256") {
                return Ok(self.1.clone().into_bytes());
            }
            if !p(self.0.len() as u64, Some(self.0.len() as u64)) {
                bail!("cancelled");
            }
            Ok(self.0.clone())
        }
    }

    #[test]
    fn download_checks_the_hash() {
        let rel = ReleaseInfo {
            version: "1".into(),
            zip_name: "OmsiLaunch-1.zip".into(),
            zip_url: "https://github.com/OmsiModsBR/OmsiLaunch/releases/download/v1/OmsiLaunch-1.zip".into(),
            sha_url: "https://github.com/OmsiModsBR/OmsiLaunch/releases/download/v1/OmsiLaunch-1.zip.sha256".into(),
            page_url: String::new(),
        };
        let z = package(&[], false);
        let ok = Fake(z.clone(), format!("{}  OmsiLaunch-1.zip", sha256_hex(&z)));
        assert_eq!(download(&ok, &rel, &mut |_, _| true).unwrap().0, z);
        let bad = Fake(z, "AA".repeat(32));
        assert!(download(&bad, &rel, &mut |_, _| true).is_err());
    }

    #[test]
    fn plan_lists_files_and_install_then_remove_leave_the_folder_as_it_was() {
        let root = tmp("root");
        let data = tmp("data");
        // a third-party plugin and the player's own file stay untouched
        std::fs::create_dir_all(root.join("plugins")).unwrap();
        std::fs::write(root.join("plugins/Other.opl"), b"theirs").unwrap();
        std::fs::write(root.join("Omsi.exe"), b"game").unwrap();
        let z = package(&[], false);
        let plan = plan_install(&root, &data, &z).unwrap();
        assert!(plan.ok() && plan.files.len() == 5 && plan.replaces.is_empty());
        let rec = install(&root, &data, &z, &sha256_hex(&z), "0.1.0-beta4").unwrap();
        assert_eq!(rec.files.len(), 5);
        assert!(root.join("plugins/OmsiLaunch.Plugin.opl").is_file());
        assert!(!data
            .join("omsi2/staging")
            .read_dir()
            .map(|mut d| d.next().is_some())
            .unwrap_or(false));
        // installing again is an update of its own files, not a conflict
        let again = plan_install(&root, &data, &z).unwrap();
        assert!(again.conflicts.is_empty() && again.replaces.len() == 5);
        let rp = remove(&root, &data).unwrap();
        assert_eq!(rp.files.len(), 5);
        assert!(!root.join("OmsiLaunch.exe").exists() && !root.join(".omsilaunch").exists());
        assert_eq!(
            std::fs::read(root.join("plugins/Other.opl")).unwrap(),
            b"theirs"
        );
        assert_eq!(std::fs::read(root.join("Omsi.exe")).unwrap(), b"game");
        assert!(load_record(&data).is_none());
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&data);
    }

    #[test]
    fn a_foreign_file_is_never_overwritten() {
        let root = tmp("conf");
        let data = tmp("confd");
        std::fs::write(root.join("OmsiLaunch.exe"), b"someone else's").unwrap();
        let z = package(&[], false);
        let plan = plan_install(&root, &data, &z).unwrap();
        assert_eq!(plan.conflicts, vec!["OmsiLaunch.exe".to_string()]);
        assert!(install(&root, &data, &z, &sha256_hex(&z), "1").is_err());
        assert_eq!(
            std::fs::read(root.join("OmsiLaunch.exe")).unwrap(),
            b"someone else's"
        );
        assert!(!root.join("plugins").exists());
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&data);
    }

    #[test]
    fn a_package_that_disagrees_with_its_manifest_installs_nothing() {
        let root = tmp("bad");
        let data = tmp("badd");
        let z = package(&[], true);
        assert!(install(&root, &data, &z, &sha256_hex(&z), "1").is_err());
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&data);
    }

    #[test]
    fn unsafe_paths_are_refused() {
        for bad in ["../x", "a/../../x", "/abs", "C:/x", "a\\b", ""] {
            assert!(safe_rel(bad).is_none(), "{bad}");
        }
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        w.start_file("../evil.dll", zip::write::SimpleFileOptions::default())
            .unwrap();
        w.write_all(b"x").unwrap();
        let z = w.finish().unwrap().into_inner();
        assert!(plan_install(&tmp("esc"), &tmp("escd"), &z).is_err());
    }

    #[test]
    fn removal_waits_for_a_pending_recovery_and_keeps_player_data() {
        let root = tmp("rm");
        let data = tmp("rmd");
        let z = package(&[], false);
        install(&root, &data, &z, &sha256_hex(&z), "1").unwrap();
        std::fs::create_dir_all(root.join(".omsilaunch/session-profiles/mine")).unwrap();
        std::fs::write(root.join(".omsilaunch/journal.json"), b"{}").unwrap();
        assert!(remove(&root, &data).is_err());
        assert!(root.join("OmsiLaunch.exe").exists());
        std::fs::remove_file(root.join(".omsilaunch/journal.json")).unwrap();
        remove(&root, &data).unwrap();
        assert!(root.join(".omsilaunch/session-profiles/mine").exists());
        assert!(!root.join("OmsiLaunch.exe").exists());
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&data);
    }

    /// A package whose manifest lists only `listed` of the `entries`.
    fn zip_with(entries: &[(&str, &[u8])], listed: &[&str]) -> Vec<u8> {
        let files: Vec<String> = entries
            .iter()
            .filter(|(n, _)| listed.contains(n))
            .map(|(n, b)| format!("{{\"path\":\"{n}\",\"sha256\":\"{}\"}}", sha256_hex(b)))
            .collect();
        let manifest = format!("{{\"files\":[{}]}}", files.join(","));
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let o = zip::write::SimpleFileOptions::default();
        for (n, b) in entries {
            w.start_file(*n, o).unwrap();
            w.write_all(b).unwrap();
        }
        w.start_file("release-manifest.json", o).unwrap();
        w.write_all(manifest.as_bytes()).unwrap();
        w.finish().unwrap().into_inner()
    }

    #[test]
    fn a_payload_file_the_manifest_does_not_list_is_refused() {
        let root = tmp("unl");
        let data = tmp("unld");
        let z = zip_with(&[("a.dll", b"a"), ("b.dll", b"b")], &["a.dll"]);
        assert!(install(&root, &data, &z, &sha256_hex(&z), "1").is_err());
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
        let z = zip_with(&[("a.dll", b"a")], &[]);
        assert!(install(&root, &data, &z, &sha256_hex(&z), "1").is_err());
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&data);
    }

    #[test]
    fn an_update_removes_files_the_new_release_dropped() {
        let root = tmp("upd");
        let data = tmp("updd");
        let v1 = zip_with(
            &[("plugins/Old.opl", b"old"), ("x.dll", b"x")],
            &["plugins/Old.opl", "x.dll"],
        );
        install(&root, &data, &v1, &sha256_hex(&v1), "1").unwrap();
        let v2 = zip_with(
            &[("plugins/New.opl", b"new"), ("x.dll", b"x2")],
            &["plugins/New.opl", "x.dll"],
        );
        let rec = install(&root, &data, &v2, &sha256_hex(&v2), "2").unwrap();
        assert!(!root.join("plugins/Old.opl").exists());
        assert!(root.join("plugins/New.opl").is_file());
        assert!(!rec.files.iter().any(|f| f == "plugins/Old.opl"));
        remove(&root, &data).unwrap();
        assert!(!root.join("plugins/New.opl").exists());
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&data);
    }

    #[test]
    fn a_failing_record_write_restores_the_previous_install() {
        let root = tmp("rec");
        let data = tmp("recd");
        let v1 = zip_with(&[("x.dll", b"one")], &["x.dll"]);
        install(&root, &data, &v1, &sha256_hex(&v1), "1").unwrap();
        // the record's place is taken by a folder, so writing it fails
        std::fs::remove_file(record_path(&data)).unwrap();
        std::fs::create_dir(record_path(&data)).unwrap();
        let v2 = zip_with(&[("x.dll", b"two"), ("y.dll", b"y")], &["x.dll", "y.dll"]);
        assert!(install(&root, &data, &v2, &sha256_hex(&v2), "2").is_err());
        assert_eq!(std::fs::read(root.join("x.dll")).unwrap(), b"one");
        assert!(!root.join("y.dll").exists());
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&data);
    }

    #[cfg(unix)]
    #[test]
    fn a_linked_folder_in_the_way_is_refused() {
        let root = tmp("lnk");
        let data = tmp("lnkd");
        let outside = tmp("lnko");
        std::os::unix::fs::symlink(&outside, root.join("plugins")).unwrap();
        let z = zip_with(&[("plugins/A.opl", b"a")], &["plugins/A.opl"]);
        assert!(plan_install(&root, &data, &z).is_err());
        assert!(install(&root, &data, &z, &sha256_hex(&z), "1").is_err());
        assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 0);
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&data);
        let _ = std::fs::remove_dir_all(&outside);
    }

    #[test]
    fn a_forged_manifest_with_the_real_hash_does_not_make_files_ours() {
        let root = tmp("forge2");
        let data = tmp("forged2");
        std::fs::write(root.join("OMSI.exe"), b"game").unwrap();
        let h = sha256_hex(b"game");
        std::fs::write(
            root.join("release-manifest.json"),
            format!(r#"{{"files":[{{"path":"OMSI.exe","sha256":"{h}"}}]}}"#),
        )
        .unwrap();
        assert!(owned_list(&root, &data).is_empty());
        assert!(remove(&root, &data).unwrap().files.is_empty());
        assert_eq!(std::fs::read(root.join("OMSI.exe")).unwrap(), b"game");
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&data);
    }

    #[test]
    fn the_record_is_found_however_the_folder_is_spelled() {
        let a = Path::new("/games/OMSI 2");
        assert!(same_root(a, Path::new("/games/OMSI 2/")));
        assert!(same_root(a, Path::new("/games//OMSI 2")));
        assert!(!same_root(a, Path::new("/games/OMSI 3")));
        if cfg!(windows) {
            assert!(same_root(
                Path::new(r"C:\Games\OMSI 2"),
                Path::new("c:/games/omsi 2/")
            ));
        }
    }

    #[test]
    fn removal_does_not_go_through_a_linked_omsilaunch_folder() {
        let root = tmp("lnkrm");
        let data = tmp("lnkrmd");
        let outside = tmp("lnkrmo");
        std::fs::create_dir_all(outside.join("cache")).unwrap();
        std::fs::write(outside.join("cache/keep.txt"), b"x").unwrap();
        std::os::unix::fs::symlink(&outside, root.join(".omsilaunch")).unwrap();
        let _ = remove(&root, &data);
        assert!(outside.join("cache/keep.txt").exists());
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&data);
        let _ = std::fs::remove_dir_all(&outside);
    }

    #[test]
    fn a_manifest_written_by_someone_else_does_not_make_files_ours() {
        let root = tmp("forge");
        let data = tmp("forged");
        std::fs::write(root.join("OMSI.exe"), b"game").unwrap();
        // a manifest in the folder that claims the game's own file (with a wrong hash)
        std::fs::write(
            root.join("release-manifest.json"),
            r#"{"files":[{"path":"OMSI.exe","sha256":"00"}]}"#,
        )
        .unwrap();
        assert!(owned_list(&root, &data).is_empty());
        // a package that carries a file of that name is a conflict, not an update
        let z = zip_with(&[("OMSI.exe", b"other")], &["OMSI.exe"]);
        assert!(plan_install(&root, &data, &z)
            .unwrap()
            .conflicts
            .contains(&"OMSI.exe".to_string()));
        assert!(install(&root, &data, &z, &sha256_hex(&z), "1").is_err());
        assert!(remove(&root, &data).unwrap().files.is_empty());
        assert_eq!(std::fs::read(root.join("OMSI.exe")).unwrap(), b"game");
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&data);
    }

    #[test]
    fn duplicate_or_case_colliding_entries_are_refused() {
        let root = tmp("dup");
        let data = tmp("dupd");
        let z = zip_with(&[("a.dll", b"a"), ("A.DLL", b"b")], &["a.dll", "A.DLL"]);
        assert!(plan_install(&root, &data, &z).is_err());
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&data);
    }

    #[test]
    fn windows_names_are_refused_in_package_paths() {
        for bad in [
            "NUL",
            "COM0",
            "lpt0.txt",
            "aux.dll",
            "plugins/COM1.txt",
            "a/b.",
            "a /b",
            "x<y",
            "a|b",
            "q?",
        ] {
            assert!(safe_rel(bad).is_none(), "{bad}");
        }
        for ok in [
            ".omsilaunch/docs/README.md",
            "plugins/OmsiLaunch.Plugin.opl",
            "COM10.dll",
            "console.cfg",
        ] {
            assert!(safe_rel(ok).is_some(), "{ok}");
        }
    }

    #[test]
    fn only_the_official_repository_path_is_a_release_asset() {
        let ok = format!("https://github.com/{REPO}/releases/download/v1/OmsiLaunch-1.zip");
        assert!(official_asset(&ok));
        assert!(!official_asset(
            "https://github.com/someone/else/releases/download/v1/x.zip"
        ));
        assert!(!official_asset(
            "https://example.com/OmsiModsBR/OmsiLaunch/releases/download/v1/x.zip"
        ));
    }

    #[test]
    fn a_package_that_is_not_the_one_hashed_is_not_installed() {
        let root = tmp("hsh");
        let data = tmp("hshd");
        let z = zip_with(&[("a.dll", b"a")], &["a.dll"]);
        assert!(install(&root, &data, &z, &"AB".repeat(32), "1").is_err());
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&data);
    }

    #[test]
    fn a_failed_install_takes_back_the_folders_it_made_and_a_removal_keeps_others() {
        let root = tmp("dirs");
        let data = tmp("dirsd");
        std::fs::create_dir_all(root.join("plugins")).unwrap();
        let z = zip_with(
            &[("plugins/A.opl", b"a"), ("new/deep/B.dll", b"b")],
            &["plugins/A.opl", "new/deep/B.dll"],
        );
        std::fs::create_dir_all(record_path(&data)).unwrap();
        assert!(install(&root, &data, &z, &sha256_hex(&z), "1").is_err());
        assert!(!root.join("new").exists() && root.join("plugins").is_dir());
        std::fs::remove_dir_all(record_path(&data)).unwrap();
        install(&root, &data, &z, &sha256_hex(&z), "1").unwrap();
        remove(&root, &data).unwrap();
        // the empty folder that was there before stays; the ones Depot made go
        assert!(root.join("plugins").is_dir() && !root.join("new").exists());
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&data);
    }

    #[cfg(unix)]
    #[test]
    fn an_update_never_removes_through_a_linked_folder() {
        let root = tmp("lnku");
        let data = tmp("lnkud");
        let outside = tmp("lnkuo");
        let v1 = zip_with(
            &[("plugins/Old.opl", b"old"), ("x.dll", b"x")],
            &["plugins/Old.opl", "x.dll"],
        );
        install(&root, &data, &v1, &sha256_hex(&v1), "1").unwrap();
        // the plugins folder becomes a link to a folder holding a file of the same name
        std::fs::write(outside.join("Old.opl"), b"theirs").unwrap();
        std::fs::remove_dir_all(root.join("plugins")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("plugins")).unwrap();
        let v2 = zip_with(&[("x.dll", b"x2")], &["x.dll"]);
        assert!(install(&root, &data, &v2, &sha256_hex(&v2), "2").is_err());
        assert_eq!(std::fs::read(outside.join("Old.opl")).unwrap(), b"theirs");
        assert!(remove(&root, &data).is_err());
        assert_eq!(std::fs::read(outside.join("Old.opl")).unwrap(), b"theirs");
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&data);
        let _ = std::fs::remove_dir_all(&outside);
    }
}
