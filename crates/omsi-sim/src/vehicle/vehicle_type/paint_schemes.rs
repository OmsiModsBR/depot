//! The paint schemes of a vehicle: the `[item]` groups of its `.cti` files, which replace
//! the textures of a model's `[CTCTexture]` slots.

use std::path::{Path, PathBuf};

/// One `.cti` item group: replaces the textures of `[CTCTexture]` slots and sets variables.
#[derive(Debug, Clone, Default)]
pub struct PaintScheme {
    pub name: String,
    /// Folder the item came from (textures resolve there first).
    pub dir: PathBuf,
    /// (`[CTCTexture]` name, texture file)
    pub textures: Vec<(String, String)>,
    pub set_vars: Vec<(String, f32)>,
}

/// Collect the `[item]` groups of every `.cti` file in `dir`. Consecutive items with the same
/// name form one scheme; `[setvar]` lines belong to the item before them. The folder is read
/// in every content root (a repaint installed as a mod lies in the content folder's copy of
/// the stock folder); a file of the same name in a higher-priority root replaces the other.
pub fn load_paint_schemes(dir: &Path) -> Vec<PaintScheme> {
    let mut schemes: Vec<PaintScheme> = Vec::new();
    let mut files: Vec<PathBuf> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    // the folder in every content root (and mounted archive) that has it
    for d in omsi_cfg::mirrored_dirs(dir) {
        let Some(list) = omsi_cfg::vfs::list_dir(&d) else {
            continue;
        };
        let mut here: Vec<PathBuf> = list
            .into_iter()
            .map(|(n, _)| d.join(n))
            .filter(|p| {
                p.extension()
                    .map(|e| e.eq_ignore_ascii_case("cti"))
                    .unwrap_or(false)
            })
            .collect();
        here.sort();
        for f in here {
            if seen.insert(
                f.file_name()
                    .map(|n| n.to_string_lossy().to_ascii_lowercase())
                    .unwrap_or_default(),
            ) {
                files.push(f);
            }
        }
    }
    for f in files {
        let dir = f
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| dir.to_path_buf());
        let Ok(cfg) = omsi_cfg::CfgFile::read(&f) else {
            continue;
        };
        let mut r = cfg.reader();
        while let Some(k) = r.next_keyword() {
            match k.as_str() {
                "item" => {
                    let name = r.str().to_string();
                    let ctc = r.str().to_string();
                    let tex = r.str().to_string();
                    match schemes
                        .iter_mut()
                        .find(|s| s.name.eq_ignore_ascii_case(&name))
                    {
                        Some(s) => s.textures.push((ctc, tex)),
                        None => schemes.push(PaintScheme {
                            name,
                            dir: dir.clone(),
                            textures: vec![(ctc, tex)],
                            set_vars: Vec::new(),
                        }),
                    }
                }
                "setvar" => {
                    let var = r.str().to_string();
                    let v = r.f32();
                    if let Some(s) = schemes.last_mut() {
                        s.set_vars.push((var, v));
                    }
                }
                _ => {}
            }
        }
    }
    schemes
}
