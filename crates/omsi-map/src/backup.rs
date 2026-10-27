//! Copies of a map's files, taken before the launcher writes over them, and their return.
//!
//! Each save gets a folder of its own, `~/.depot/backups/<map folder>/<stamp>/`, with the
//! files as they were (by their path in the map folder) and `backup.txt`, which names them
//! and the ones the save made new. Nothing is put into the map folder itself, so a map
//! zipped up to be shared carries no copies. A copy is never replaced: a second save in the
//! same minute gets `<stamp>-2`.

use std::io;
use std::path::{Path, PathBuf};

/// The list of what a backup holds.
const MANIFEST: &str = "backup.txt";

/// One file of a backup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackedUp {
    /// The path in the map folder, with `/`.
    pub file: String,
    /// It was there before the save (and is in the backup); else the save made it.
    pub existed: bool,
}

/// A backup folder and what it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backup {
    pub dir: PathBuf,
    /// The folder's name: when it was taken.
    pub stamp: String,
    pub files: Vec<BackedUp>,
}

/// The stamp of a backup taken at that local time: `YYYY-MM-DD_HH-MM`, which sorts by time.
pub fn stamp(year: i32, month: i32, day: i32, hour: i32, minute: i32) -> String {
    format!("{year:04}-{month:02}-{day:02}_{hour:02}-{minute:02}")
}

/// Where the backups of the map in `map_dir` go: `~/.depot/backups/<map folder>`.
pub fn store_of(map_dir: &Path) -> Option<PathBuf> {
    let name = map_dir.file_name()?;
    Some(omsi_cfg::paths::data_dir()?.join("backups").join(name))
}

/// Whether `path` is a file on the disk that can be written: not one in a mounted archive,
/// not a read-only one. A file not there yet can be made when its folder is there.
pub fn writable(path: &Path) -> bool {
    match std::fs::metadata(path) {
        Ok(m) => m.is_file() && !m.permissions().readonly(),
        Err(_) => path.parent().is_some_and(|d| d.is_dir()),
    }
}

fn manifest_text(files: &[BackedUp]) -> String {
    files
        .iter()
        .map(|f| format!("{}\t{}\r\n", if f.existed { "copy" } else { "new" }, f.file))
        .collect()
}

fn free_dir(store: &Path, stamp: &str) -> PathBuf {
    let mut dir = store.join(stamp);
    let mut n = 2;
    while dir.exists() {
        dir = store.join(format!("{stamp}-{n}"));
        n += 1;
    }
    dir
}

/// Copy the files `files` (paths in the map folder) as they are now into a new backup.
fn take(map_dir: &Path, store: &Path, stamp: &str, files: &[&str]) -> io::Result<Backup> {
    let dir = free_dir(store, stamp);
    std::fs::create_dir_all(&dir)?;
    let mut held = Vec::new();
    for f in files {
        let src = map_dir.join(f);
        let existed = src.is_file();
        if existed {
            let dst = dir.join(f);
            if let Some(p) = dst.parent() {
                std::fs::create_dir_all(p)?;
            }
            std::fs::copy(&src, &dst)?;
        }
        held.push(BackedUp {
            file: f.replace('\\', "/"),
            existed,
        });
    }
    omsi_cfg::write_atomic(dir.join(MANIFEST), manifest_text(&held))?;
    Ok(Backup {
        stamp: dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        dir,
        files: held,
    })
}

/// Write `files` ((path in the map folder, bytes)) into the map folder, after a backup of
/// what they replace. When a write fails, the files already written are put back as they
/// were and the error is returned; the backup stays.
pub fn write_with_backup(
    map_dir: &Path,
    store: &Path,
    stamp: &str,
    files: &[(String, Vec<u8>)],
) -> io::Result<Backup> {
    let names: Vec<&str> = files.iter().map(|(f, _)| f.as_str()).collect();
    let backup = take(map_dir, store, stamp, &names)?;
    for (i, (f, bytes)) in files.iter().enumerate() {
        if let Err(e) = omsi_cfg::write_atomic(map_dir.join(f), bytes) {
            let _ = put_back(map_dir, &backup, &backup.files[..i]);
            return Err(e);
        }
    }
    Ok(backup)
}

fn put_back(map_dir: &Path, backup: &Backup, files: &[BackedUp]) -> io::Result<()> {
    for f in files {
        let dst = map_dir.join(&f.file);
        if f.existed {
            let bytes = std::fs::read(backup.dir.join(&f.file))?;
            omsi_cfg::write_atomic(&dst, bytes)?;
        } else if dst.is_file() {
            std::fs::remove_file(&dst)?;
        }
    }
    Ok(())
}

/// The backups in `store`, newest first.
pub fn list(store: &Path) -> Vec<Backup> {
    let mut out: Vec<Backup> = std::fs::read_dir(store)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let dir = e.path();
            let text = std::fs::read_to_string(dir.join(MANIFEST)).ok()?;
            let files = text
                .lines()
                .filter_map(|l| {
                    let (kind, file) = l.split_once('\t')?;
                    Some(BackedUp {
                        file: file.trim().to_string(),
                        existed: kind == "copy",
                    })
                })
                .collect();
            Some(Backup {
                stamp: e.file_name().to_string_lossy().into_owned(),
                dir,
                files,
            })
        })
        .collect();
    // (`-2` after the plain stamp of the same minute; `-10` after `-9`)
    out.sort_by_key(|b| {
        let (base, n) = match b.stamp.rsplit_once('-') {
            Some((base, n)) if b.stamp.len() > 16 => (base.to_string(), n.parse().unwrap_or(0)),
            _ => (b.stamp.clone(), 1),
        };
        std::cmp::Reverse((base, n))
    });
    out
}

/// The map's files as `backup` holds them: those it copied are written back, those the
/// save made are taken out. What is there now is backed up first (at `stamp`), so a
/// restore can itself be undone; that new backup is returned.
pub fn restore(map_dir: &Path, store: &Path, stamp: &str, backup: &Backup) -> io::Result<Backup> {
    let names: Vec<&str> = backup.files.iter().map(|f| f.file.as_str()).collect();
    let now = take(map_dir, store, stamp, &names)?;
    put_back(map_dir, backup, &backup.files)?;
    Ok(now)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("omsi-map-backup-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_save_keeps_what_it_replaced_and_a_restore_brings_it_back() {
        let base = temp("save");
        let map = base.join("Grundorf");
        let store = base.join("backups").join("Grundorf");
        std::fs::create_dir_all(&map).unwrap();
        std::fs::write(map.join("ailists.cfg"), b"old lists").unwrap();

        let files = vec![
            ("ailists.cfg".to_string(), b"new lists".to_vec()),
            ("parklist_p_1.txt".to_string(), b"a.sco".to_vec()),
        ];
        let b = write_with_backup(&map, &store, &stamp(2026, 10, 27, 4, 30), &files).unwrap();
        assert_eq!(b.stamp, "2026-10-27_04-30");
        assert_eq!(
            std::fs::read(map.join("ailists.cfg")).unwrap(),
            b"new lists"
        );
        assert_eq!(
            std::fs::read(b.dir.join("ailists.cfg")).unwrap(),
            b"old lists"
        );
        assert!(!b.dir.join("parklist_p_1.txt").exists());
        assert_eq!(
            b.files,
            [
                BackedUp {
                    file: "ailists.cfg".into(),
                    existed: true
                },
                BackedUp {
                    file: "parklist_p_1.txt".into(),
                    existed: false
                }
            ]
        );

        // a second save in the same minute does not touch the first backup
        let files2 = vec![("ailists.cfg".to_string(), b"newer".to_vec())];
        let b2 = write_with_backup(&map, &store, "2026-10-27_04-30", &files2).unwrap();
        assert_eq!(b2.stamp, "2026-10-27_04-30-2");
        assert_eq!(
            std::fs::read(b2.dir.join("ailists.cfg")).unwrap(),
            b"new lists"
        );
        let all = list(&store);
        assert_eq!(
            all.iter().map(|b| b.stamp.as_str()).collect::<Vec<_>>(),
            ["2026-10-27_04-30-2", "2026-10-27_04-30"]
        );
        assert_eq!(all[1], b);

        // back to before the first save: the new parking list goes, the old lists return
        let undo = restore(&map, &store, "2026-10-27_04-31", &all[1]).unwrap();
        assert_eq!(
            std::fs::read(map.join("ailists.cfg")).unwrap(),
            b"old lists"
        );
        assert!(!map.join("parklist_p_1.txt").exists());
        // and that restore can be undone too
        assert_eq!(
            std::fs::read(undo.dir.join("ailists.cfg")).unwrap(),
            b"newer"
        );
        assert_eq!(
            std::fs::read(undo.dir.join("parklist_p_1.txt")).unwrap(),
            b"a.sco"
        );
        restore(&map, &store, "2026-10-27_04-32", &undo).unwrap();
        assert_eq!(std::fs::read(map.join("ailists.cfg")).unwrap(), b"newer");
        assert_eq!(
            std::fs::read(map.join("parklist_p_1.txt")).unwrap(),
            b"a.sco"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_failed_write_puts_back_what_was_written() {
        let base = temp("fail");
        let map = base.join("M");
        let store = base.join("store");
        std::fs::create_dir_all(&map).unwrap();
        std::fs::write(map.join("ailists.cfg"), b"old").unwrap();
        // the second file's folder is not there: its write fails
        let files = vec![
            ("ailists.cfg".to_string(), b"new".to_vec()),
            ("missing/parklist_p.txt".to_string(), b"x".to_vec()),
        ];
        assert!(write_with_backup(&map, &store, "s", &files).is_err());
        assert_eq!(std::fs::read(map.join("ailists.cfg")).unwrap(), b"old");
        assert!(writable(&map.join("ailists.cfg")));
        assert!(writable(&map.join("parklist_p.txt")));
        assert!(!writable(&map.join("missing/parklist_p.txt")));
        let _ = std::fs::remove_dir_all(&base);
    }
}
