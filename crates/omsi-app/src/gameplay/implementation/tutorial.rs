//! OMSI 2's tutorial pages as files: the folder of a lesson and its `<step>.html` pages in the
//! language's folder (English when the folder is missing), read once when the lesson opens.

use crate::gameplay::domain::tutorial::{page_text, Page, Tutorial};
use std::path::{Path, PathBuf};

impl Tutorial {
    /// Tutorial `number` (1..4) in the language (`ENG`, `DEU`, `FRA`; English when the
    /// folder is missing).
    pub fn load(root: &Path, number: usize, lang: &str) -> Option<Tutorial> {
        let dir = root.join("Tutorials").join(number.to_string());
        let lang_dir = [lang, "ENG", "DEU"]
            .iter()
            .map(|l| dir.join(l))
            .find(|d| d.is_dir())?;
        let mut steps: Vec<(u64, PathBuf)> = std::fs::read_dir(&lang_dir)
            .ok()?
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                let n = p.file_stem()?.to_string_lossy().parse::<u64>().ok()?;
                (p.extension()?.eq_ignore_ascii_case("html")).then_some((n, p))
            })
            .collect();
        steps.sort();
        let pages = steps
            .into_iter()
            .filter_map(|(n, p)| {
                let bytes = std::fs::read(&p).ok()?;
                let (title, text) = page_text(&omsi_cfg::decode_text(&bytes));
                let img = dir.join(format!("{n}.jpg"));
                Some(Page {
                    title,
                    text,
                    image: img.is_file().then_some(img),
                })
            })
            .collect::<Vec<_>>();
        log::info!(
            "tutorial {number}: {} pages from {}",
            pages.len(),
            lang_dir.display()
        );
        (!pages.is_empty()).then_some(Tutorial {
            pages,
            at: 0,
            hidden: false,
        })
    }
}
