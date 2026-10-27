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

/// Where the backups of the map in `map_dir` go: `~/.depot/backups/<map folder>`. (By the
/// folder's name alone: two OMSI 2 installations with a map of the same folder share it.)
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

/// Whether `file` is a file a backup may hold and put back: an AI list (`ailists.cfg`) or a
/// parking list (`parklist_p*.txt`) right in the map folder - never a path that leaves it.
pub fn allowed(file: &str) -> bool {
    let mut parts = Path::new(file).components();
    let (Some(std::path::Component::Normal(name)), None) = (parts.next(), parts.next()) else {
        return false;
    };
    let name = name.to_string_lossy().to_ascii_lowercase();
    // (no `:`: on NTFS `ailists.cfg:x` is a hidden stream of the file, not the file)
    !name.contains(':')
        && (name == "ailists.cfg" || (name.starts_with("parklist_p") && name.ends_with(".txt")))
}

fn check(files: &[&str]) -> io::Result<()> {
    match files.iter().find(|f| !allowed(f)) {
        Some(f) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{f} is not an AI or parking list of the map folder"),
        )),
        None => Ok(()),
    }
}

fn manifest_text(files: &[BackedUp]) -> String {
    files
        .iter()
        .map(|f| format!("{}\t{}\r\n", if f.existed { "copy" } else { "new" }, f.file))
        .collect()
}

/// A new folder for a backup: `stamp`, else `stamp-2`, `stamp-3`... (made here, so two saves
/// at once never share one).
fn free_dir(store: &Path, stamp: &str) -> io::Result<PathBuf> {
    std::fs::create_dir_all(store)?;
    let mut dir = store.join(stamp);
    let mut n = 2;
    loop {
        match std::fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                dir = store.join(format!("{stamp}-{n}"));
                n += 1;
            }
            Err(e) => return Err(e),
        }
    }
}

/// Copy the files `files` (paths in the map folder) as they are now into a new backup.
fn take(map_dir: &Path, store: &Path, stamp: &str, files: &[&str]) -> io::Result<Backup> {
    check(files)?;
    let dir = free_dir(store, stamp)?;
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
/// were and the error is returned; the backup stays. Only AI and parking lists of the map
/// folder are written (see [`allowed`]).
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
            return Err(undone(map_dir, &backup, i, e));
        }
    }
    Ok(backup)
}

/// The error `e` of the `i`th file, after the files before it are put back as `backup`
/// holds them; when that fails too, the error says which backup holds them.
fn undone(map_dir: &Path, backup: &Backup, i: usize, e: io::Error) -> io::Error {
    match put_back(map_dir, backup, &backup.files[..i]) {
        Ok(()) => e,
        Err(back) => io::Error::new(
            e.kind(),
            format!(
                "{e}; the files written before it could not be put back either ({back}): \
                 the backup {} holds them",
                backup.dir.display()
            ),
        ),
    }
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

/// The backups in `store`, newest first. A backup whose list names a file it may not hold
/// (see [`allowed`]) is left out.
pub fn list(store: &Path) -> Vec<Backup> {
    let mut out: Vec<Backup> = std::fs::read_dir(store)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let dir = e.path();
            let text = std::fs::read_to_string(dir.join(MANIFEST)).ok()?;
            let files: Vec<BackedUp> = text
                .lines()
                .filter_map(|l| {
                    let (kind, file) = l.split_once('\t')?;
                    Some(BackedUp {
                        file: file.trim().to_string(),
                        existed: kind == "copy",
                    })
                })
                .collect();
            if !files.iter().all(|f| allowed(&f.file)) {
                return None;
            }
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
    // (`take` refuses a file that is not an AI or parking list of the map folder)
    let now = take(map_dir, store, stamp, &names)?;
    // (every copy is read first: a backup that lost one changes nothing)
    let mut copies = Vec::new();
    for f in &backup.files {
        copies.push(match f.existed {
            true => Some(std::fs::read(backup.dir.join(&f.file))?),
            false => None,
        });
    }
    // (when a file cannot be put back, the ones before it go back to what they were now)
    for (i, (f, bytes)) in backup.files.iter().zip(copies).enumerate() {
        let dst = map_dir.join(&f.file);
        let done = match bytes {
            Some(bytes) => omsi_cfg::write_atomic(&dst, bytes),
            None if dst.is_file() => std::fs::remove_file(&dst),
            None => Ok(()),
        };
        if let Err(e) = done {
            return Err(undone(map_dir, &now, i, e));
        }
    }
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
        // a folder where the second file goes: its write fails
        std::fs::create_dir(map.join("parklist_p.txt")).unwrap();
        let files = vec![
            ("ailists.cfg".to_string(), b"new".to_vec()),
            ("parklist_p.txt".to_string(), b"x".to_vec()),
        ];
        assert!(write_with_backup(&map, &store, "s", &files).is_err());
        assert_eq!(std::fs::read(map.join("ailists.cfg")).unwrap(), b"old");
        assert!(writable(&map.join("ailists.cfg")));
        assert!(writable(&map.join("parklist_p_2.txt")));
        assert!(!writable(&map.join("missing/parklist_p.txt")));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_restore_that_cannot_finish_leaves_the_map_as_it_was() {
        let base = temp("restore-fail");
        let map = base.join("M");
        let store = base.join("store");
        std::fs::create_dir_all(&map).unwrap();
        std::fs::write(map.join("ailists.cfg"), b"old").unwrap();
        std::fs::write(map.join("parklist_p.txt"), b"a.sco").unwrap();
        let files = vec![
            ("ailists.cfg".to_string(), b"new".to_vec()),
            ("parklist_p.txt".to_string(), b"b.sco".to_vec()),
        ];
        let saved = write_with_backup(&map, &store, "s", &files).unwrap();

        // a copy the backup lost: nothing is put back
        std::fs::remove_file(saved.dir.join("parklist_p.txt")).unwrap();
        assert!(restore(&map, &store, "r", &saved).is_err());
        assert_eq!(std::fs::read(map.join("ailists.cfg")).unwrap(), b"new");
        assert_eq!(std::fs::read(map.join("parklist_p.txt")).unwrap(), b"b.sco");

        // the second file cannot be written: the first goes back to what it was
        std::fs::write(saved.dir.join("parklist_p.txt"), b"a.sco").unwrap();
        std::fs::remove_file(map.join("parklist_p.txt")).unwrap();
        std::fs::create_dir(map.join("parklist_p.txt")).unwrap();
        assert!(restore(&map, &store, "r", &saved).is_err());
        assert_eq!(std::fs::read(map.join("ailists.cfg")).unwrap(), b"new");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn nothing_outside_the_map_folder_is_written_or_taken_out() {
        let base = temp("outside");
        let map = base.join("maps").join("M");
        let store = base.join("store");
        std::fs::create_dir_all(&map).unwrap();
        std::fs::write(map.join("ailists.cfg"), b"lists").unwrap();
        let victim = base.join("victim.txt");
        std::fs::write(&victim, b"keep").unwrap();

        for name in [
            "../../victim.txt".to_string(),
            "../ailists.cfg".to_string(),
            "ailists.cfg:hidden".to_string(),
            victim.display().to_string(),
            "sub/ailists.cfg".to_string(),
            "global.cfg".to_string(),
        ] {
            assert!(!allowed(&name), "{name}");
            let files = vec![(name.clone(), b"x".to_vec())];
            let e = write_with_backup(&map, &store, "s", &files).unwrap_err();
            assert_eq!(e.kind(), io::ErrorKind::InvalidInput, "{name}");
        }
        assert_eq!(std::fs::read(&victim).unwrap(), b"keep");
        assert!(!base.join("maps").join("ailists.cfg").exists());
        for name in [
            "ailists.cfg",
            "AILISTS.CFG",
            "parklist_p.txt",
            "parklist_p_3.txt",
        ] {
            assert!(allowed(name), "{name}");
        }

        // a forged list in a backup: the backup is not offered, and a restore of it is refused
        let forged = store.join("2026-01-01_00-00");
        std::fs::create_dir_all(&forged).unwrap();
        std::fs::write(forged.join(MANIFEST), "new\t../../victim.txt\r\n").unwrap();
        assert!(list(&store).is_empty());
        let b = Backup {
            dir: forged.clone(),
            stamp: "2026-01-01_00-00".into(),
            files: vec![BackedUp {
                file: "../../victim.txt".into(),
                existed: false,
            }],
        };
        assert!(restore(&map, &store, "t", &b).is_err());
        assert_eq!(std::fs::read(&victim).unwrap(), b"keep");
        assert_eq!(std::fs::read(map.join("ailists.cfg")).unwrap(), b"lists");
        let _ = std::fs::remove_dir_all(&base);
    }
}
