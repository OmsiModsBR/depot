//! The map's own timetable put back: the launcher's copy of its `TTData` deleted, or the lines
//! saved in place put back from their `.orig`.

use super::save::COPY_MARK;
use std::path::Path;

/// What putting a map's timetable back did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reset {
    /// The launcher's copy of the map's `TTData` was deleted.
    CopyRemoved,
    /// This many lines saved over were put back from their `.orig`.
    Restored(usize),
    /// Nothing here had changed the map's timetable.
    Unchanged,
}

/// The map's own timetable back, for the map in `map_dir` (folder `map_folder`) and the
/// content folder `content`: the launcher's copy of its `TTData` deleted, or (a map unpacked
/// in the content folder) every line saved over put back from its `.orig`.
pub fn reset_timetable(
    content: Option<&Path>,
    map_dir: &Path,
    map_folder: &str,
) -> Result<Reset, String> {
    let content = content.ok_or("no content folder")?;
    let copy = content.join("maps").join(map_folder).join("TTData");
    if copy.join(COPY_MARK).is_file() {
        std::fs::remove_dir_all(&copy).map_err(|e| e.to_string())?;
        return Ok(Reset::CopyRemoved);
    }
    let own = map_dir.join("TTData");
    let mut restored = 0;
    if own.is_dir() {
        for e in std::fs::read_dir(&own)
            .map_err(|e| e.to_string())?
            .flatten()
        {
            let p = e.path();
            if let Some(orig) = p.to_str().and_then(|s| s.strip_suffix(".orig")) {
                std::fs::rename(&p, orig).map_err(|e| e.to_string())?;
                restored += 1;
            }
        }
    }
    Ok(if restored > 0 {
        Reset::Restored(restored)
    } else {
        Reset::Unchanged
    })
}
