//! Where a changed line is written: in place for a map unpacked in the content folder, else in
//! the content folder's copy of the map's `TTData`.

use super::super::interface::output::Line;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Marks a `TTData` folder the launcher copied into the content folder (see `save_target`):
/// the reset deletes such a copy, and only such a one.
pub const COPY_MARK: &str = ".depot-ttdata-copy";

/// Where a map's line is written: in place when the map lies unpacked in the content folder
/// `content` (its file backed up as `<file>.orig` the first time, for the reset), else in the
/// content folder's copy of its `TTData` (made whole first - the game reads the one folder,
/// not a mix of both). A map inside a mod archive (`.zip`) counts as not in the content
/// folder: its file was "saved in place" into the archive's path, which is no folder, and
/// never saved.
pub fn save_target(
    line: &Line,
    content: &Path,
    map_folder: &str,
    original_ttdata: &Path,
) -> Result<PathBuf, String> {
    let file = line
        .path
        .file_name()
        .map(|f| f.to_owned())
        .unwrap_or_else(|| format!("{}.ttl", line.name).into());
    if line.path.starts_with(content) && omsi_cfg::vfs::archive_of(&line.path).is_none() {
        let orig = PathBuf::from(format!("{}.orig", line.path.display()));
        let copied = line
            .path
            .parent()
            .is_some_and(|d| d.join(COPY_MARK).is_file());
        if !copied && line.path.is_file() && !orig.exists() {
            std::fs::copy(&line.path, &orig).map_err(|e| e.to_string())?;
        }
        return Ok(line.path.clone());
    }
    let dir = content.join("maps").join(map_folder).join("TTData");
    if !dir.is_dir() {
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        for (name, is_dir) in omsi_cfg::vfs::list_dir(original_ttdata)
            .ok_or_else(|| format!("{} cannot be read", original_ttdata.display()))?
        {
            if !is_dir {
                let bytes =
                    omsi_cfg::vfs::read(&original_ttdata.join(&name)).map_err(|e| e.to_string())?;
                std::fs::write(dir.join(&name), bytes).map_err(|e| e.to_string())?;
            }
        }
        std::fs::write(dir.join(COPY_MARK), b"TTData copied by the Project Depot launcher's timetable editor; its reset deletes this folder\n").map_err(|e| e.to_string())?;
    }
    Ok(dir.join(file))
}

/// Save every line of `lines` whose name is in `dirty` (see `save_target`), the map lying in
/// `map_dir` and the content folder being `content`; each saved line's path becomes the file
/// it was written to. How many were saved, and the first error.
pub fn save_lines(
    lines: &mut [Line],
    dirty: &BTreeSet<String>,
    map_dir: &Path,
    content: Option<&Path>,
) -> (usize, Option<String>) {
    let folder = map_dir
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_default();
    let original = omsi_cfg::resolve_path(map_dir, "TTData");
    let mut saved = 0;
    let mut err = None;
    for line in lines.iter_mut().filter(|l| dirty.contains(&l.name)) {
        for t in &mut line.tours {
            t.trips.sort_by(|a, b| a.departure.total_cmp(&b.departure));
        }
        match content
            .ok_or_else(|| "no content folder".to_string())
            .and_then(|c| save_target(line, c, &folder, &original))
            .and_then(|p| line.save(&p).map(|_| p).map_err(|e| e.to_string()))
        {
            Ok(p) => {
                line.path = p;
                saved += 1;
            }
            Err(e) => {
                err.get_or_insert(format!("line {}: {e}", line.name));
            }
        }
    }
    if saved > 0 {
        omsi_cfg::content_changed();
    }
    (saved, err)
}
