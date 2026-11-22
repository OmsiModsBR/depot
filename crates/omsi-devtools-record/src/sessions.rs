//! The record files of a folder: one per session, the last five kept.

use crate::line::civil;
use crate::writer::{RunRecord, RECORD_CAP_BYTES};
use omsi_devtools_proto::Hello;
use std::fs::File;
use std::io::{BufWriter, Result};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// How many sessions' records are kept, the one being written included.
pub const SESSIONS_KEPT: usize = 5;

/// What a record file's name starts with (`run-20261122-101500.jsonl`).
pub const RECORD_PREFIX: &str = "run-";
/// A record file's extension.
pub const RECORD_EXTENSION: &str = "jsonl";

/// The record files of the folder `dir`, the oldest first (their names sort by time).
pub fn records_in(dir: &Path) -> Vec<PathBuf> {
    let mut records: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().is_some_and(|e| e == RECORD_EXTENSION)
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with(RECORD_PREFIX))
        })
        .collect();
    // (by time, then by the number of a session started in the same second: `…-101500`,
    // `…-101500-2`, … `…-101500-10`)
    records.sort_by_key(|path| order_of(path));
    records
}

/// Where a record file stands among the others: its time stamp, then its number among the
/// sessions of that second (1 for the first, which has none in its name).
fn order_of(path: &Path) -> (String, u64) {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let stem = stem.strip_prefix(RECORD_PREFIX).unwrap_or(stem);
    // (`20261122-101500` or `20261122-101500-2`)
    let mut parts = stem.splitn(3, '-');
    let stamp = format!(
        "{}-{}",
        parts.next().unwrap_or(""),
        parts.next().unwrap_or("")
    );
    let n = parts.next().and_then(|n| n.parse().ok()).unwrap_or(1);
    (stamp, n)
}

/// Removes the oldest records of `dir` so that at most `keep` are left.
pub fn prune(dir: &Path, keep: usize) {
    let records = records_in(dir);
    let extra = records.len().saturating_sub(keep);
    for old in &records[..extra] {
        let _ = std::fs::remove_file(old);
    }
}

/// Starts the record of a new session in the folder `dir` (made when it is not there), named
/// by the time `now`, with its header line. Older records are removed first, so that
/// [`SESSIONS_KEPT`] are left with the new one: a record is only ever removed when a new
/// session starts, never while one is written.
pub fn start_in(
    dir: &Path,
    game: &Hello,
    now: SystemTime,
) -> Result<(RunRecord<BufWriter<File>>, PathBuf)> {
    std::fs::create_dir_all(dir)?;
    let (y, mo, d, h, mi, s) = civil(now);
    let stamp = format!("{y:04}{mo:02}{d:02}-{h:02}{mi:02}{s:02}");
    // (two sessions started in the same second: the later one is numbered after the last
    // of them, counted before the old records go - a number set free by their removal
    // would put the new session before older ones, and the next start would remove it)
    let last = records_in(dir)
        .iter()
        .map(|path| order_of(path))
        .filter(|(at, _)| *at == stamp)
        .map(|(_, n)| n)
        .max();
    let path = match last {
        None => dir.join(format!("{RECORD_PREFIX}{stamp}.{RECORD_EXTENSION}")),
        Some(n) => dir.join(format!(
            "{RECORD_PREFIX}{stamp}-{}.{RECORD_EXTENSION}",
            n + 1
        )),
    };
    prune(dir, SESSIONS_KEPT.saturating_sub(1));
    let file = File::create_new(&path)?;
    let record = RunRecord::start(BufWriter::new(file), RECORD_CAP_BYTES, game, now)?;
    Ok((record, path))
}
