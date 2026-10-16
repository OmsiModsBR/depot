//! Detection and repair of bloated `.hof` files.
//!
//! Some depot files carry `[infosystem_busstop_list]` tables that were multiplied by a depot
//! editor every time the file was opened and saved again: the declared sizes are powers of
//! two (2, 4, ... 16384) and each table is a few long runs of the same stop name, although
//! the route only has an origin and a destination. Such a file is tens of times bigger than
//! its content (a few MB instead of a few KB) and loading it takes minutes, because every
//! one of its entries is read and kept.
//!
//! Detection ([`inspect_lines`]) is cheap and works on already loaded lines. Nothing in the
//! game calls it yet: when to scan users' depot files is left to the maintainer of the refactors.
//! Repair ([`fix_bytes`]) removes the repetition inside the tables and leaves every other
//! byte of the file as it was. Nothing here writes a file except [`fix_file`], which keeps a
//! backup of the original next to it first; callers are expected to ask the user before
//! calling it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

const TABLE_KEYWORD: &[u8] = b"[infosystem_busstop_list]";

/// A table must be at least this long to count as "declared size is a power of two".
const POW2_MIN: usize = 128;
/// Tables of at least this many entries made of few equal runs are flagged.
const RUNS_MIN_ENTRIES: usize = 16;
const RUNS_MAX: usize = 3;
const RUN_MIN_LEN: usize = 8;

/// What the stop-list tables of a depot file look like.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Inflation {
    /// `[infosystem_busstop_list]` tables in the file.
    pub tables: usize,
    /// Declared size of the largest table.
    pub largest: usize,
    /// Sum of the declared sizes.
    pub total_entries: usize,
    /// Tables whose size is a power of two >= 128 and that mostly repeat themselves.
    pub power_of_two_tables: usize,
    /// Tables of a few equal-length runs of the same entry.
    pub run_tables: usize,
}

impl Inflation {
    /// Whether the file has the signature of the repetition described in the module docs.
    pub fn is_problematic(&self) -> bool {
        self.power_of_two_tables > 0 || self.run_tables > 0
    }
}

struct Table {
    /// Index of the `[infosystem_busstop_list]` line.
    start: usize,
    /// Entries that are really there (the declared count cut at the end of the file).
    len: usize,
    declared: usize,
}

fn trimmed(line: &[u8]) -> &[u8] {
    line.trim_ascii()
}

fn tables<L: AsRef<[u8]>>(lines: &[L]) -> Vec<Table> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if trimmed(lines[i].as_ref()).eq_ignore_ascii_case(TABLE_KEYWORD) {
            let declared = lines
                .get(i + 1)
                .map(|l| omsi_cfg::parse_i64(&String::from_utf8_lossy(l.as_ref())).max(0) as usize)
                .unwrap_or(0);
            let len = declared.min(lines.len().saturating_sub(i + 2));
            out.push(Table {
                start: i,
                len,
                declared,
            });
            i += 2 + len;
        } else {
            i += 1;
        }
    }
    out
}

/// Consecutive equal entries as (first index, length).
fn runs<L: AsRef<[u8]>>(entries: &[L]) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = Vec::new();
    for (i, e) in entries.iter().enumerate() {
        match out.last_mut() {
            Some((first, len)) if trimmed(entries[*first].as_ref()) == trimmed(e.as_ref()) => {
                *len += 1;
            }
            _ => out.push((i, 1)),
        }
    }
    out
}

fn equal_runs(r: &[(usize, usize)]) -> bool {
    r.iter().all(|&(_, l)| l == r[0].1)
}

/// Looks at the stop-list tables of a loaded depot file.
pub fn inspect_lines<L: AsRef<[u8]>>(lines: &[L]) -> Inflation {
    let mut inf = Inflation::default();
    for t in tables(lines) {
        inf.tables += 1;
        inf.largest = inf.largest.max(t.declared);
        inf.total_entries += t.declared;
        let entries = &lines[t.start + 2..t.start + 2 + t.len];
        let r = runs(entries);
        if t.declared >= POW2_MIN && t.declared.is_power_of_two() && r.len() <= t.len / 2 {
            inf.power_of_two_tables += 1;
        }
        if t.len >= RUNS_MIN_ENTRIES
            && !r.is_empty()
            && r.len() <= RUNS_MAX
            && equal_runs(&r)
            && r[0].1 >= RUN_MIN_LEN
        {
            inf.run_tables += 1;
        }
    }
    inf
}

/// [`inspect_lines`] on raw file bytes (as the file is on disk).
pub fn inspect_bytes(bytes: &[u8]) -> Inflation {
    inspect_lines(&split_keep_eol(bytes))
}

fn split_keep_eol(bytes: &[u8]) -> Vec<&[u8]> {
    bytes.split_inclusive(|&b| b == b'\n').collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixError {
    /// UTF-16 files are not rewritten.
    Utf16,
    /// Nothing in the file is repetition of the kind that can be removed.
    NothingToFix,
}

impl std::fmt::Display for FixError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FixError::Utf16 => write!(f, "UTF-16 depot files are not repaired"),
            FixError::NothingToFix => write!(f, "no repeated stop list to remove"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fixed {
    pub bytes: Vec<u8>,
    pub tables_changed: usize,
    /// Tables left as they were although the file is flagged (title missing, unequal runs).
    pub tables_left: usize,
    pub entries_before: usize,
    pub entries_after: usize,
}

/// The "origin" and "destination" of a route title ("Line 11, Route 2: A  >>  B").
fn route_title(line: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
    let s = String::from_utf8_lossy(trimmed(line)).into_owned();
    let lower = s.to_ascii_lowercase();
    if !lower.starts_with("line ") {
        return None;
    }
    let route = lower.find(", route ")?;
    let colon = route + s[route..].find(':')?;
    let (a, b) = s[colon + 1..].split_once(">>")?;
    Some((a.trim().as_bytes().to_vec(), b.trim().as_bytes().to_vec()))
}

/// The entries one table reduces to (indices into the table's entries), or None when it is
/// left as it is. Circular routes (origin == destination) keep two entries, routes with
/// distinct ends reduced to one repeated stop keep one.
fn reduced<L: AsRef<[u8]>>(
    entries: &[L],
    title: Option<&(Vec<u8>, Vec<u8>)>,
) -> Option<Vec<usize>> {
    let n = entries.len();
    let r = runs(entries);
    let keep: Vec<usize> = if r.len() >= 2 {
        if !equal_runs(&r) {
            return None;
        }
        r.iter().map(|&(first, _)| first).collect()
    } else if r.len() == 1 {
        let (a, b) = title?;
        if a == b {
            vec![0, 0]
        } else {
            vec![0]
        }
    } else {
        return None;
    };
    if keep.len() >= n || !n.is_multiple_of(keep.len()) {
        return None;
    }
    // the reduced table repeated n / keep.len() times per entry must be the original
    let times = n / keep.len();
    let again: Vec<&[u8]> = keep
        .iter()
        .flat_map(|&k| std::iter::repeat_n(trimmed(entries[k].as_ref()), times))
        .collect();
    let same = again
        .iter()
        .zip(entries)
        .all(|(x, y)| *x == trimmed(y.as_ref()));
    same.then_some(keep)
}

/// Removes the repetition inside the stop-list tables of a flagged file; every other byte
/// stays. Tables that do not match the rules (unequal runs, no route title, fewer than
/// four entries) are left alone and counted in `tables_left`.
pub fn fix_bytes(bytes: &[u8]) -> Result<Fixed, FixError> {
    if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        return Err(FixError::Utf16);
    }
    let lines = split_keep_eol(bytes);
    if !inspect_lines(&lines).is_problematic() {
        return Err(FixError::NothingToFix);
    }
    let found = tables(&lines);
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len() / 8);
    let mut at = 0;
    let (mut changed, mut left, mut before, mut after) = (0, 0, 0, 0);
    let mut title = None;
    let mut next_table = found.iter().peekable();
    while at < lines.len() {
        match next_table.peek() {
            Some(t) if t.start == at => {
                let t = next_table.next().unwrap();
                let entries = &lines[at + 2..at + 2 + t.len];
                let keep = if t.len >= 4 && t.len == t.declared {
                    reduced(entries, title.as_ref())
                } else {
                    None
                };
                title = None;
                before += t.declared;
                out.extend_from_slice(lines[at]);
                match keep {
                    Some(keep) => {
                        let eol = eol_of(lines[at + 1]);
                        out.extend_from_slice(keep.len().to_string().as_bytes());
                        out.extend_from_slice(eol);
                        for k in &keep {
                            out.extend_from_slice(trimmed_end_eol(entries[*k]));
                            out.extend_from_slice(eol);
                        }
                        changed += 1;
                        after += keep.len();
                    }
                    None => {
                        for l in &lines[at + 1..at + 2 + t.len] {
                            out.extend_from_slice(l);
                        }
                        left += 1;
                        after += t.declared;
                    }
                }
                at += 2 + t.len;
            }
            _ => {
                if let Some(t) = route_title(lines[at]) {
                    title = Some(t);
                }
                out.extend_from_slice(lines[at]);
                at += 1;
            }
        }
    }
    if changed == 0 {
        return Err(FixError::NothingToFix);
    }
    Ok(Fixed {
        bytes: out,
        tables_changed: changed,
        tables_left: left,
        entries_before: before,
        entries_after: after,
    })
}

fn eol_of(line: &[u8]) -> &[u8] {
    if line.ends_with(b"\r\n") {
        b"\r\n"
    } else if line.ends_with(b"\n") {
        b"\n"
    } else {
        b"\r\n"
    }
}

fn trimmed_end_eol(line: &[u8]) -> &[u8] {
    let mut end = line.len();
    while end > 0 && matches!(line[end - 1], b'\r' | b'\n') {
        end -= 1;
    }
    &line[..end]
}

// ---------------------------------------------------------------------------------------
// Finding the copies in an installation and rewriting them

/// FNV-1a: a content key that is the same in every process.
pub fn content_key(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ *b as u64).wrapping_mul(0x100000001b3)
    })
}

/// A bloated depot file and every place it is installed with the very same content (under
/// whatever file name: `key` identifies the group on its own).
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub file_name: String,
    pub bytes: u64,
    pub key: u64,
    pub inflation: Inflation,
    pub paths: Vec<PathBuf>,
    /// What a repair would leave: (bytes, entries after); None when it cannot be repaired.
    pub repaired: Option<(u64, usize)>,
}

fn child_ci(dir: &Path, name: &str) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name))
        .map(|e| e.path())
}

fn collect_hofs(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        let Ok(ty) = e.file_type() else { continue };
        if ty.is_dir() && depth > 0 {
            collect_hofs(&p, depth - 1, out);
        } else if ty.is_file() && p.extension().is_some_and(|x| x.eq_ignore_ascii_case("hof")) {
            out.push(p);
        }
    }
}

/// The bloated depot files of an OMSI installation (`Vehicles/**` and the shared `HOFs/`),
/// grouped by identical content. `only_name` limits the search to files of that name.
pub fn scan_install(root: &Path, only_name: Option<&str>) -> Vec<Group> {
    let mut files = Vec::new();
    for sub in ["Vehicles", "HOFs"] {
        if let Some(d) = child_ci(root, sub) {
            collect_hofs(&d, 4, &mut files);
        }
    }
    files.sort();
    let mut groups: HashMap<u64, Group> = HashMap::new();
    for p in files {
        let name = p
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        if only_name.is_some_and(|n| !n.eq_ignore_ascii_case(&name)) {
            continue;
        }
        let Ok(bytes) = std::fs::read(&p) else {
            continue;
        };
        let inflation = inspect_bytes(&bytes);
        if !inflation.is_problematic() {
            continue;
        }
        let key = content_key(&bytes);
        groups
            .entry(key)
            .or_insert_with(|| Group {
                file_name: name,
                bytes: bytes.len() as u64,
                key,
                inflation,
                paths: Vec::new(),
                repaired: fix_bytes(&bytes)
                    .ok()
                    .map(|f| (f.bytes.len() as u64, f.entries_after)),
            })
            .paths
            .push(p);
    }
    let mut v: Vec<Group> = groups.into_values().collect();
    v.sort_by(|a, b| b.bytes.cmp(&a.bytes).then(a.file_name.cmp(&b.file_name)));
    v
}

#[derive(Debug, Clone, PartialEq)]
pub enum FileResult {
    Fixed {
        backup: PathBuf,
        bytes_before: u64,
        bytes_after: u64,
    },
    /// The file no longer matches what was offered (edited, already repaired): untouched.
    Skipped(String),
    Failed(String),
}

fn free_backup_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let mut n = 0;
    loop {
        let b = path.with_file_name(if n == 0 {
            format!("{name}.bak")
        } else {
            format!("{name}.bak{}", n + 1)
        });
        if !b.exists() {
            return b;
        }
        n += 1;
    }
}

/// Rewrites one depot file without its repetition. The original is copied to `<name>.bak`
/// (or `.bak2`, ...: an older backup is never replaced) first; the new file is written
/// beside it and moved over the original, then read back and checked. `expected_key` is the
/// content the user was shown: a file that changed since is left alone.
pub fn fix_file(path: &Path, expected_key: Option<u64>) -> FileResult {
    use std::io::Write;
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => return FileResult::Failed(format!("reading: {e}")),
    };
    if expected_key.is_some_and(|k| k != content_key(&bytes)) {
        return FileResult::Skipped("the file changed since it was checked".into());
    }
    let fixed = match fix_bytes(&bytes) {
        Ok(f) => f,
        Err(e) => return FileResult::Skipped(e.to_string()),
    };
    if inspect_bytes(&fixed.bytes).is_problematic() {
        return FileResult::Failed("the repaired text still looks repeated; not written".into());
    }
    let backup = free_backup_path(path);
    let made = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&backup)
        .and_then(|mut f| f.write_all(&bytes).and_then(|_| f.sync_all()));
    if let Err(e) = made {
        let _ = std::fs::remove_file(&backup);
        return FileResult::Failed(format!(
            "backup {} failed (is the game closed and the folder writable?): {e}",
            backup.display()
        ));
    }
    let tmp = path.with_extension("hof.depot-tmp");
    let written = std::fs::write(&tmp, &fixed.bytes).and_then(|_| std::fs::rename(&tmp, path));
    if let Err(e) = written {
        let _ = std::fs::remove_file(&tmp);
        return FileResult::Failed(format!(
            "writing failed, the original is untouched (is the game closed?): {e}"
        ));
    }
    match std::fs::read(path) {
        Ok(b) if b == fixed.bytes => FileResult::Fixed {
            backup,
            bytes_before: bytes.len() as u64,
            bytes_after: b.len() as u64,
        },
        _ => FileResult::Failed(format!(
            "read back differs; the original is saved as {}",
            backup.display()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A synthetic depot file in the shape of the bloated ones: `copies` is how often each
    /// table was multiplied.
    fn depot(copies: usize, circular_every: usize) -> String {
        let mut s = String::from(
            "[name]\r\nTest Map\r\nstringcount_terminus\r\n1\r\n[addterminus]\r\n1\r\nAlpha\r\nALPHA\r\n",
        );
        for i in 0..6 {
            let (a, b) = if circular_every > 0 && i % circular_every == 0 {
                ("Central".to_string(), "Central".to_string())
            } else {
                (format!("Terminal {i}"), format!("Harbour {i}"))
            };
            s.push_str(&format!(
                "[infosystem_trip]\r\n{i}\r\nLine {i}, Route 1: {a}  >>  {b}\r\n{i}\r\n{i}\r\n[infosystem_busstop_list]\r\n"
            ));
            let n = 2 * copies;
            s.push_str(&format!("{n}\r\n"));
            if a == b {
                for _ in 0..n {
                    s.push_str(&format!("{a}\r\n"));
                }
            } else {
                for _ in 0..n / 2 {
                    s.push_str(&format!("{a}\r\n"));
                }
                for _ in 0..n / 2 {
                    s.push_str(&format!("{b}\r\n"));
                }
            }
            s.push_str("\r\n");
        }
        s
    }

    #[test]
    fn bloated_depot_is_flagged_and_normal_is_not() {
        let big = depot(4096, 3);
        let inf = inspect_bytes(big.as_bytes());
        assert_eq!(inf.tables, 6);
        assert_eq!(inf.largest, 8192);
        assert_eq!(inf.power_of_two_tables, 6);
        assert_eq!(inf.run_tables, 6);
        assert!(inf.is_problematic());

        // a normal file: real stop lists with distinct stops, short
        let mut normal = String::from("[infosystem_busstop_list]\r\n5\r\n");
        for s in ["A", "B", "C", "D", "E"] {
            normal.push_str(&format!("{s}\r\n"));
        }
        assert!(!inspect_bytes(normal.as_bytes()).is_problematic());
        // the corrected shape (two entries per table) is fine too
        assert!(!inspect_bytes(depot(1, 3).as_bytes()).is_problematic());
        // a long list with a repeated stop name (a loop line) is not an inflated one
        let mut long = String::from("[infosystem_busstop_list]\r\n32\r\n");
        for i in 0..32 {
            long.push_str(&format!("Stop {}\r\n", i % 20));
        }
        assert!(!inspect_bytes(long.as_bytes()).is_problematic());
    }

    #[test]
    fn small_doubling_is_found_by_equal_runs() {
        // 3 cycles of doubling on a 2-stop route is 16 entries: below 128, but two runs of 8
        assert!(inspect_bytes(depot(8, 0).as_bytes()).is_problematic());
    }

    #[test]
    fn fix_gives_the_two_stops_and_scores_ok() {
        let big = depot(4096, 3);
        let fixed = fix_bytes(big.as_bytes()).unwrap();
        assert_eq!(fixed.tables_changed, 6);
        assert_eq!(fixed.tables_left, 0);
        assert_eq!(fixed.entries_after, 12);
        assert!(!inspect_bytes(&fixed.bytes).is_problematic());
        // output equals the same file written with two entries per table
        assert_eq!(String::from_utf8(fixed.bytes).unwrap(), depot(1, 3));
    }

    #[test]
    fn fix_leaves_everything_else_byte_for_byte() {
        let big = depot(512, 0);
        let fixed = fix_bytes(big.as_bytes()).unwrap();
        // non-table lines are the same lines in the same order
        let keep = |s: &str| -> Vec<String> {
            let mut out = Vec::new();
            let lines: Vec<&str> = s.split("\r\n").collect();
            let mut i = 0;
            while i < lines.len() {
                if lines[i] == "[infosystem_busstop_list]" {
                    out.push(lines[i].to_string());
                    i += 2 + lines[i + 1].parse::<usize>().unwrap();
                } else {
                    out.push(lines[i].to_string());
                    i += 1;
                }
            }
            out
        };
        assert_eq!(keep(&big), keep(&String::from_utf8(fixed.bytes).unwrap()));
    }

    #[test]
    fn circular_and_single_rules() {
        // origin == destination: [A, A]; one stop repeated with distinct ends: [A]
        let text = "Line 1, Route 1: Loop  >>  Loop\r\n[infosystem_busstop_list]\r\n16\r\n"
            .to_string()
            + &"Loop\r\n".repeat(16)
            + "Line 2, Route 1: A  >>  B\r\n[infosystem_busstop_list]\r\n16\r\n"
            + &"A\r\n".repeat(16);
        let fixed = fix_bytes(text.as_bytes()).unwrap();
        assert_eq!(
            String::from_utf8(fixed.bytes).unwrap(),
            "Line 1, Route 1: Loop  >>  Loop\r\n[infosystem_busstop_list]\r\n2\r\nLoop\r\nLoop\r\nLine 2, Route 1: A  >>  B\r\n[infosystem_busstop_list]\r\n1\r\nA\r\n"
        );
    }

    #[test]
    fn unreducible_tables_are_left_and_reported() {
        // flagged by its first table; the second has unequal runs and no title
        let mut text =
            String::from("Line 1, Route 1: A  >>  B\r\n[infosystem_busstop_list]\r\n16\r\n");
        text += &"A\r\n".repeat(8);
        text += &"B\r\n".repeat(8);
        text += "[infosystem_busstop_list]\r\n6\r\n";
        text += &"X\r\n".repeat(4);
        text += &"Y\r\n".repeat(2);
        let fixed = fix_bytes(text.as_bytes()).unwrap();
        assert_eq!((fixed.tables_changed, fixed.tables_left), (1, 1));
        assert!(String::from_utf8(fixed.bytes)
            .unwrap()
            .ends_with("X\r\nX\r\nX\r\nX\r\nY\r\nY\r\n"));
    }

    #[test]
    fn nothing_to_fix_and_utf16() {
        assert_eq!(
            fix_bytes(depot(1, 0).as_bytes()),
            Err(FixError::NothingToFix)
        );
        assert_eq!(fix_bytes(&[0xFF, 0xFE, 0x41, 0]), Err(FixError::Utf16));
    }

    #[test]
    fn identical_content_under_different_names_is_one_group() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("depot-hofgroup-{stamp}"));
        let d = root.join("Vehicles/BusA");
        std::fs::create_dir_all(&d).unwrap();
        let big = depot(512, 2);
        let other = depot(256, 3);
        std::fs::write(d.join("One.hof"), &big).unwrap();
        std::fs::write(d.join("Two.hof"), &big).unwrap();
        std::fs::write(d.join("Three.hof"), &other).unwrap();
        let groups = scan_install(&root, None);
        assert_eq!(groups.len(), 2);
        assert_ne!(groups[0].key, groups[1].key);
        assert_eq!(groups.iter().map(|g| g.paths.len()).sum::<usize>(), 3);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn copies_are_grouped_and_fixed_with_backups() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("depot-hofcheck-{stamp}"));
        let big = depot(512, 2);
        for m in ["BusA", "BusB"] {
            let d = root.join("Vehicles").join(m);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("Map.hof"), &big).unwrap();
        }
        let d = root.join("Vehicles/BusC");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("Map.hof"), depot(1, 2)).unwrap();
        std::fs::write(d.join("Other.hof"), "[name]\r\nx\r\n").unwrap();

        let groups = scan_install(&root, Some("map.hof"));
        assert_eq!(groups.len(), 1);
        let g = &groups[0];
        assert_eq!(g.paths.len(), 2);
        assert!(g.repaired.unwrap().0 < g.bytes / 20);

        for p in &g.paths {
            match fix_file(p, Some(g.key)) {
                FileResult::Fixed {
                    backup,
                    bytes_before,
                    bytes_after,
                } => {
                    assert_eq!(std::fs::read(&backup).unwrap(), big.as_bytes());
                    assert!(bytes_after < bytes_before);
                }
                other => panic!("{other:?}"),
            }
            assert!(!inspect_bytes(&std::fs::read(p).unwrap()).is_problematic());
        }
        assert!(scan_install(&root, None).is_empty());
        // a second run does not replace the first backup
        std::fs::write(g.paths[0].clone(), &big).unwrap();
        assert!(matches!(
            fix_file(&g.paths[0], None),
            FileResult::Fixed { .. }
        ));
        assert!(g.paths[0].with_file_name("Map.hof.bak2").exists());
        // a file that changed since it was shown is left alone
        std::fs::write(g.paths[1].clone(), &big).unwrap();
        assert!(matches!(
            fix_file(&g.paths[1], Some(g.key ^ 1)),
            FileResult::Skipped(_)
        ));
        let _ = std::fs::remove_dir_all(root);
    }
}
