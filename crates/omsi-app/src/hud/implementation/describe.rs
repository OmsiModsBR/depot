//! Reading the cockpit names from the installation: the `Languages/<LANG>_key_*.olf` files
//! and the mods' copies. The rules of the names are in `domain/describe.rs`.

use super::super::domain::describe::*;
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

impl ControlNames {
    /// Read the key assignment texts of `lang` from the installation (and the mods' copies).
    pub fn load(root: &Path, lang: &str) -> ControlNames {
        // (OMSI's cockpit names are in English, German and French: every other language
        // reads the English ones - the interface around them is translated)
        let lang = match language_code(lang).as_str() {
            l @ ("DEU" | "FRA") => l.to_string(),
            _ => "ENG".to_string(),
        };
        let mut texts = HashMap::new();
        let mut spellings = HashMap::new();
        let mut dirs = omsi_cfg::content_dirs("Languages");
        dirs.reverse(); // the installation first, the mods' files override it
        let own = root.join("Languages");
        if !dirs.iter().any(|d| d == &own) {
            dirs.insert(0, own);
        }
        for dir in dirs {
            // (through the content file system: a mod's language files may be in an archive)
            let Some(list) = omsi_cfg::vfs::list_dir(&dir) else {
                continue;
            };
            let mut files: Vec<std::path::PathBuf> = list
                .into_iter()
                .filter(|(_, is_dir)| !*is_dir)
                .map(|(n, _)| dir.join(n))
                .filter(|p| {
                    let n = p
                        .file_name()
                        .map(|n| n.to_string_lossy().to_ascii_lowercase())
                        .unwrap_or_default();
                    n.starts_with(&format!("{}_key_", lang.to_ascii_lowercase()))
                        && n.ends_with(".olf")
                })
                .collect();
            files.sort();
            for f in files {
                if let Ok(l) = omsi_content::language::Language::load(&f) {
                    for (k, v) in l.strings {
                        if let Some(name) = k.strip_prefix("KY_").or_else(|| k.strip_prefix("ky_"))
                        {
                            let v = v.trim();
                            if !v.is_empty() && !v.starts_with('<') {
                                let key = name.to_ascii_lowercase();
                                texts.insert(key.clone(), v.to_string());
                                spellings.insert(key, name.to_string());
                            }
                        }
                    }
                }
            }
        }
        log::info!("control names: {} texts in {lang}", texts.len());
        ControlNames {
            lang,
            texts,
            spellings,
        }
    }
}

static NAMES: OnceLock<ControlNames> = OnceLock::new();

/// The process-wide names (the language is read once, from the first caller).
pub fn names(root: &Path, lang: &str) -> &'static ControlNames {
    NAMES.get_or_init(|| ControlNames::load(root, lang))
}
