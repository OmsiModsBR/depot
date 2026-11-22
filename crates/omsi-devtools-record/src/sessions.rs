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
    // (by the name without its extension: `run-…-101500` before `run-…-101500-2`)
    records.sort_by(|a, b| a.file_stem().cmp(&b.file_stem()));
    records
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
    prune(dir, SESSIONS_KEPT.saturating_sub(1));
    let (y, mo, d, h, mi, s) = civil(now);
    let stamp = format!("{y:04}{mo:02}{d:02}-{h:02}{mi:02}{s:02}");
    // (two sessions started in the same second: the later one sorts after the first)
    let path = (1..)
        .map(|n| match n {
            1 => dir.join(format!("{RECORD_PREFIX}{stamp}.{RECORD_EXTENSION}")),
            n => dir.join(format!("{RECORD_PREFIX}{stamp}-{n}.{RECORD_EXTENSION}")),
        })
        .find(|path| !path.exists())
        .expect("an endless range");
    let file = File::create_new(&path)?;
    let record = RunRecord::start(BufWriter::new(file), RECORD_CAP_BYTES, game, now)?;
    Ok((record, path))
}
