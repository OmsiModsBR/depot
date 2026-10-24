//! Compressing the content with NTFS (LZX), for Setup's "Free up space" card.
//!
//! Windows can store a file compressed (the "compact" kind, `compact /C /EXE:LZX`): the
//! file keeps its name, size and contents for every program, and only takes less on disk.
//! This module picks the files that may be compressed, estimates what that would save,
//! checks the disk, and does or undoes the compression in the background (it can be
//! cancelled). It writes nothing but the compressed state of the files themselves.
//!
//! Left alone: what the game writes (settings, logs, saves, caches, screenshots), archives
//! (`.zip`) and media that are compressed already, programs the system loads, files too
//! small to gain, and anything that is not an ordinary file on the disk (links, cloud files).

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

/// The kind of disk a folder is on. A disk that could not be told is treated as `Hdd`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disk {
    Ssd,
    /// A hard disk, or a disk that could not be told apart (the careful reading).
    Hdd,
}

/// Files smaller than this do not gain from compressing.
const MIN_FILE: u64 = 4096;

/// Folders the game writes in (compared in lower case; also any folder with "cache" in it).
const GAME_FOLDERS: [&str; 7] = [
    "settings",
    "savegame",
    "savegames",
    "screenshots",
    "logs",
    "backup",
    "previews",
];

/// Extensions never compressed: the game writes them, or they are compressed already.
const SKIPPED: [&str; 14] = [
    "zip", "log", "sav", "tmp", "exe", "dll", "sys", "ogg", "mp3", "jpg", "jpeg", "png", "7z",
    "rar",
];

/// Whether `file` (a name) may be compressed, by its extension.
pub fn may_compress_name(file: &str) -> bool {
    match file.rsplit_once('.') {
        Some((_, e)) => !SKIPPED.contains(&e.to_ascii_lowercase().as_str()),
        None => true,
    }
}

fn game_folder(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    GAME_FOLDERS.contains(&n.as_str()) || n.contains("cache")
}

/// The files of `dir` that may be compressed, with their sizes.
pub fn eligible(dir: &Path) -> Vec<(PathBuf, u64)> {
    let mut out = Vec::new();
    walk(dir, &mut out);
    out
}

fn walk(dir: &Path, out: &mut Vec<(PathBuf, u64)>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let Ok(meta) = std::fs::symlink_metadata(e.path()) else {
            continue;
        };
        let name = e.file_name().to_string_lossy().into_owned();
        if !ordinary(&meta) {
            continue;
        }
        if meta.is_dir() {
            if !game_folder(&name) {
                walk(&e.path(), out);
            }
        } else if meta.len() >= MIN_FILE && may_compress_name(&name) {
            out.push((e.path(), meta.len()));
        }
    }
}

/// An ordinary file or folder: not a link, not a cloud placeholder.
#[cfg(windows)]
fn ordinary(m: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    // reparse point, offline, recall on open / on data access
    const NOT: u32 = 0x400 | 0x1000 | 0x4_0000 | 0x40_0000;
    m.file_attributes() & NOT == 0
}

#[cfg(not(windows))]
fn ordinary(m: &std::fs::Metadata) -> bool {
    !m.file_type().is_symlink()
}

// ---------------------------------------------------------------------------------------
// the estimate

/// What compressing a folder would save, estimated.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Estimate {
    /// The files that may be compressed, and what they hold.
    pub files: usize,
    pub bytes: u64,
    /// What compressing them is expected to free. An estimate, not a promise.
    pub saved: u64,
}

/// How much of the files is read for the estimate, and from each file.
const SAMPLE_TOTAL: u64 = 48 << 20;
const SAMPLE_EACH: usize = 256 << 10;

/// Estimates what compressing `files` frees. Some of them (spread over the list, the start
/// of each) are compressed in memory by kind of file, and each kind's ratio is applied to
/// all its bytes. The stand-in is a general-purpose compressor; the real result will differ.
pub fn estimate(files: &[(PathBuf, u64)]) -> Estimate {
    let bytes: u64 = files.iter().map(|f| f.1).sum();
    // (kind -> (bytes of all its files, sampled in, sampled out))
    let mut kinds: Vec<(&'static str, u64, u64, u64)> = Vec::new();
    for (p, len) in files {
        let k = crate::disk::kind_of(&p.to_string_lossy());
        match kinds.iter_mut().find(|x| x.0 == k) {
            Some(x) => x.1 += len,
            None => kinds.push((k, *len, 0, 0)),
        }
    }
    let mut read = 0u64;
    let stride = (files.len() / 400).max(1);
    for (p, len) in files.iter().step_by(stride) {
        if read >= SAMPLE_TOTAL {
            break;
        }
        let Some(buf) = head(p, SAMPLE_EACH.min(*len as usize)) else {
            continue;
        };
        let out = deflated(&buf);
        read += buf.len() as u64;
        let k = crate::disk::kind_of(&p.to_string_lossy());
        if let Some(x) = kinds.iter_mut().find(|x| x.0 == k) {
            x.2 += buf.len() as u64;
            x.3 += out.min(buf.len()) as u64;
        }
    }
    let (all_in, all_out) = kinds
        .iter()
        .fold((0u64, 0u64), |a, k| (a.0 + k.2, a.1 + k.3));
    let overall = if all_in > 0 {
        all_out as f64 / all_in as f64
    } else {
        1.0
    };
    let saved: f64 = kinds
        .iter()
        .map(|&(_, total, i, o)| {
            let ratio = if i >= 64 << 10 {
                o as f64 / i as f64
            } else {
                overall
            };
            total as f64 * (1.0 - ratio)
        })
        .sum();
    Estimate {
        files: files.len(),
        bytes,
        saved: (saved.max(0.0) as u64).min(bytes),
    }
}

fn head(p: &Path, n: usize) -> Option<Vec<u8>> {
    let mut f = std::fs::File::open(p).ok()?;
    let mut buf = vec![0u8; n];
    let mut got = 0;
    while got < n {
        match f.read(&mut buf[got..]).ok()? {
            0 => break,
            r => got += r,
        }
    }
    buf.truncate(got);
    Some(buf)
}

fn deflated(buf: &[u8]) -> usize {
    let mut e = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::new(6));
    if e.write_all(buf).is_err() {
        return buf.len();
    }
    e.finish().map(|v| v.len()).unwrap_or(buf.len())
}

// ---------------------------------------------------------------------------------------
// the disk

/// Why a folder cannot be compressed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unsupported {
    /// Not Windows.
    System,
    /// Kept in a cloud-sync folder (OneDrive, Dropbox).
    Cloud,
    /// Not on a fixed local disk (network, removable, optical).
    NotFixed,
    /// The disk is not NTFS (its file system, "" when unknown).
    NotNtfs(String),
    /// The disk could not be read.
    Unreadable,
}

/// Whether the folder's disk can hold compressed files, and what kind of disk it is.
pub fn check(dir: &Path) -> Result<Disk, Unsupported> {
    sys::check(dir)
}

/// A folder as the Setup card shows it.
#[derive(Debug, Clone)]
pub struct Folder {
    pub name: &'static str,
    pub dir: PathBuf,
    pub disk: Result<Disk, Unsupported>,
    /// The files that may be compressed, with their sizes.
    pub files: Vec<(PathBuf, u64)>,
    pub estimate: Estimate,
    /// (files stored compressed now, bytes that saves)
    pub compressed: (usize, u64),
    /// What the disk has free now, if known.
    pub free: Option<u64>,
}

/// Reads a folder for the card: its disk, its files, the estimate and what is compressed now.
pub fn inspect(name: &'static str, dir: &Path) -> Folder {
    let disk = check(dir);
    let (files, estimate, compressed) = if disk.is_ok() {
        let files = eligible(dir);
        let estimate = estimate(&files);
        let compressed = measure(&files);
        (files, estimate, compressed)
    } else {
        (Vec::new(), Estimate::default(), (0, 0))
    };
    Folder {
        name,
        dir: dir.to_path_buf(),
        disk,
        files,
        estimate,
        compressed,
        free: free_bytes(dir),
    }
}

/// What the files take on disk now: (files that are stored compressed, bytes saved by it).
pub fn measure(files: &[(PathBuf, u64)]) -> (usize, u64) {
    let mut n = 0;
    let mut saved = 0;
    for (p, len) in files {
        let on = sys::on_disk(p, *len);
        if on < *len {
            n += 1;
            saved += len - on;
        }
    }
    (n, saved)
}

/// The bytes the disk of `dir` has free, if known.
pub fn free_bytes(dir: &Path) -> Option<u64> {
    sys::free(dir)
}

// ---------------------------------------------------------------------------------------
// doing it

/// A compression or an undo running in the background.
#[derive(Debug, Default)]
pub struct Job {
    /// Bytes of files handled so far, and in all.
    pub done: AtomicU64,
    pub total: AtomicU64,
    /// Files the system refused (in use, no right to write).
    pub failed: AtomicU64,
    pub cancel: AtomicBool,
    pub finished: AtomicBool,
}

impl Job {
    pub fn new() -> Arc<Job> {
        Arc::new(Job::default())
    }
}

/// The longest a command line handed to `compact` is allowed to be, in characters.
const LINE_MAX: usize = 24_000;
/// The most bytes a single `compact` call is given, so that cancelling is quick.
const BATCH_BYTES: u64 = 256 << 20;

/// Compresses `files` (LZX) when `undo` is false, else stores them as they were.
/// Returns an error text when `compact` could not be run at all.
pub fn run(files: &[(PathBuf, u64)], undo: bool, job: &Job) -> Result<(), String> {
    job.total
        .store(files.iter().map(|f| f.1).sum(), Ordering::Relaxed);
    job.done.store(0, Ordering::Relaxed);
    job.failed.store(0, Ordering::Relaxed);
    let mut batch: Vec<&(PathBuf, u64)> = Vec::new();
    let (mut chars, mut bytes) = (0usize, 0u64);
    let mut result = Ok(());
    for f in files {
        let c = f.0.as_os_str().len() + 3;
        if !batch.is_empty() && (chars + c > LINE_MAX || bytes + f.1 > BATCH_BYTES) {
            result = sys::compact(&batch, undo, job);
            batch.clear();
            (chars, bytes) = (0, 0);
            if result.is_err() || job.cancel.load(Ordering::Relaxed) {
                break;
            }
        }
        batch.push(f);
        chars += c;
        bytes += f.1;
    }
    if result.is_ok() && !batch.is_empty() && !job.cancel.load(Ordering::Relaxed) {
        result = sys::compact(&batch, undo, job);
    }
    job.finished.store(true, Ordering::Relaxed);
    result
}

/// The files of `files` that are stored compressed now, for the undo.
pub fn compressed_of(files: &[(PathBuf, u64)]) -> Vec<(PathBuf, u64)> {
    files
        .iter()
        .filter(|(p, len)| sys::on_disk(p, *len) < *len)
        .cloned()
        .collect()
}

#[cfg(windows)]
mod sys {
    use super::*;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::process::CommandExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, GetCompressedFileSizeW, GetDiskFreeSpaceExW, GetDriveTypeW,
        GetVolumeInformationW, GetVolumePathNameW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ,
        FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows::Win32::System::Ioctl::{
        PropertyStandardQuery, StorageDeviceSeekPenaltyProperty, DEVICE_SEEK_PENALTY_DESCRIPTOR,
        IOCTL_STORAGE_QUERY_PROPERTY, STORAGE_PROPERTY_QUERY,
    };
    use windows::Win32::System::IO::DeviceIoControl;

    fn wide(p: &Path) -> Vec<u16> {
        p.as_os_str().encode_wide().chain(Some(0)).collect()
    }

    fn root_of(dir: &Path) -> Option<Vec<u16>> {
        let w = wide(dir);
        let mut root = vec![0u16; 261];
        unsafe { GetVolumePathNameW(PCWSTR(w.as_ptr()), &mut root) }.ok()?;
        Some(root)
    }

    pub fn check(dir: &Path) -> Result<Disk, Unsupported> {
        let lower = dir.to_string_lossy().to_ascii_lowercase();
        if lower.contains("onedrive") || lower.contains("dropbox") {
            return Err(Unsupported::Cloud);
        }
        let root = root_of(dir).ok_or(Unsupported::Unreadable)?;
        // fixed disks only (not network, optical or removable)
        if unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) } != 3 {
            return Err(Unsupported::NotFixed);
        }
        let mut fs = [0u16; 64];
        unsafe {
            GetVolumeInformationW(PCWSTR(root.as_ptr()), None, None, None, None, Some(&mut fs))
        }
        .map_err(|_| Unsupported::Unreadable)?;
        let name = String::from_utf16_lossy(&fs);
        let name = name.trim_end_matches('\0');
        if !name.eq_ignore_ascii_case("NTFS") {
            return Err(Unsupported::NotNtfs(name.to_string()));
        }
        Ok(disk_kind(&root))
    }

    /// SSD when the disk says it has no seek penalty; anything not certain is a hard disk.
    fn disk_kind(root: &[u16]) -> Disk {
        let s = String::from_utf16_lossy(root);
        let letter = s.chars().next().filter(|c| c.is_ascii_alphabetic());
        let Some(l) = letter else {
            return Disk::Hdd;
        };
        let path: Vec<u16> = format!("\\\\.\\{}:", l)
            .encode_utf16()
            .chain(Some(0))
            .collect();
        unsafe {
            let Ok(h) = CreateFileW(
                PCWSTR(path.as_ptr()),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                None,
            ) else {
                return Disk::Hdd;
            };
            let q = STORAGE_PROPERTY_QUERY {
                PropertyId: StorageDeviceSeekPenaltyProperty,
                QueryType: PropertyStandardQuery,
                AdditionalParameters: [0],
            };
            let mut out = DEVICE_SEEK_PENALTY_DESCRIPTOR::default();
            let mut got = 0u32;
            let ok = DeviceIoControl(
                h,
                IOCTL_STORAGE_QUERY_PROPERTY,
                Some(&q as *const _ as *const _),
                std::mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32,
                Some(&mut out as *mut _ as *mut _),
                std::mem::size_of::<DEVICE_SEEK_PENALTY_DESCRIPTOR>() as u32,
                Some(&mut got),
                None,
            )
            .is_ok();
            let _ = CloseHandle(h);
            if ok
                && got as usize >= std::mem::size_of::<DEVICE_SEEK_PENALTY_DESCRIPTOR>()
                && !out.IncursSeekPenalty
            {
                Disk::Ssd
            } else {
                Disk::Hdd
            }
        }
    }

    pub fn on_disk(p: &Path, len: u64) -> u64 {
        let w = wide(p);
        let mut high = 0u32;
        let low = unsafe { GetCompressedFileSizeW(PCWSTR(w.as_ptr()), Some(&mut high)) };
        if low == u32::MAX {
            return len;
        }
        ((high as u64) << 32) | low as u64
    }

    pub fn free(dir: &Path) -> Option<u64> {
        let w = wide(dir);
        let mut avail = 0u64;
        unsafe { GetDiskFreeSpaceExW(PCWSTR(w.as_ptr()), Some(&mut avail), None, None) }.ok()?;
        Some(avail)
    }

    /// One `compact` call over `batch`, waiting for it and watching the cancel flag.
    pub fn compact(batch: &[&(PathBuf, u64)], undo: bool, job: &Job) -> Result<(), String> {
        let mut cmd = std::process::Command::new("compact.exe");
        if undo {
            cmd.args(["/U", "/EXE"]);
        } else {
            cmd.args(["/C", "/EXE:LZX"]);
        }
        cmd.args(["/I", "/Q"]);
        for f in batch {
            cmd.arg(&f.0);
        }
        cmd.stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .creation_flags(0x0800_0000);
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("compact.exe could not be started: {e}"))?;
        let size: u64 = batch.iter().map(|f| f.1).sum();
        loop {
            match child.try_wait() {
                Ok(Some(st)) => {
                    if !st.success() {
                        job.failed.fetch_add(1, Ordering::Relaxed);
                    }
                    break;
                }
                Ok(None) if job.cancel.load(Ordering::Relaxed) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Ok(());
                }
                Ok(None) => std::thread::sleep(std::time::Duration::from_millis(100)),
                Err(e) => return Err(format!("compact.exe stopped unexpectedly: {e}")),
            }
        }
        job.done.fetch_add(size, Ordering::Relaxed);
        Ok(())
    }
}

#[cfg(not(windows))]
mod sys {
    use super::*;

    pub fn check(_: &Path) -> Result<Disk, Unsupported> {
        Err(Unsupported::System)
    }

    pub fn on_disk(_: &Path, len: u64) -> u64 {
        len
    }

    pub fn free(_: &Path) -> Option<u64> {
        None
    }

    pub fn compact(_: &[&(PathBuf, u64)], _: bool, _: &Job) -> Result<(), String> {
        Err("Compression with NTFS is only available on Windows.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(p: &Path, bytes: usize, fill: u8) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, vec![fill; bytes]).unwrap();
    }

    #[test]
    fn what_the_game_writes_and_packed_files_are_left_alone() {
        let d = std::env::temp_dir().join(format!("depot-compress-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        file(&d.join("Vehicles/Bus/model.o3d"), 100_000, 7);
        file(&d.join("Vehicles/Bus/script.osc"), 50_000, 7);
        file(&d.join("Vehicles/Bus/sound.ogg"), 100_000, 7);
        file(&d.join("Archives/pack.zip"), 100_000, 7);
        file(&d.join("Settings/controls.cfg"), 100_000, 7);
        file(&d.join("Maps/Town/TileCache/t.map"), 100_000, 7);
        file(&d.join("Maps/Town/tile_0_0.map"), 100_000, 7);
        file(&d.join("Maps/Town/log.log"), 100_000, 7);
        file(&d.join("Maps/Town/tiny.cfg"), 100, 7);
        let mut names: Vec<String> = eligible(&d)
            .into_iter()
            .map(|(p, _)| {
                p.strip_prefix(&d)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        names.sort();
        assert_eq!(
            names,
            [
                "Maps/Town/tile_0_0.map",
                "Vehicles/Bus/model.o3d",
                "Vehicles/Bus/script.osc"
            ]
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_estimate_follows_how_well_the_files_compress() {
        let d = std::env::temp_dir().join(format!("depot-estimate-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        // a very repetitive file, and one that does not compress
        file(&d.join("a.cfg"), 400_000, 1);
        let mut noise = vec![0u8; 400_000];
        let mut x = 12345u32;
        for b in noise.iter_mut() {
            x = x.wrapping_mul(1664525).wrapping_add(1013904223);
            *b = (x >> 24) as u8;
        }
        std::fs::write(d.join("b.dds"), noise).unwrap();
        let files = eligible(&d);
        let e = estimate(&files);
        assert_eq!((e.files, e.bytes), (2, 800_000));
        assert!(e.saved > 350_000 && e.saved < 450_000, "{}", e.saved);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_batches_end_when_cancelled() {
        let job = Job::new();
        job.cancel.store(true, Ordering::Relaxed);
        let files = vec![(PathBuf::from("x"), 10u64)];
        // (off Windows this says it is unavailable; on Windows a cancelled job runs nothing)
        let r = run(&files, false, &job);
        assert!(job.finished.load(Ordering::Relaxed));
        let _ = r;
    }
}
