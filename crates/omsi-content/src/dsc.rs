//! `.dsc` description files: the `[name]` (or `[friendlyname]`) lines and the
//! `[description]` text that name and describe a map, a bus or a weather in one language.

use std::path::{Path, PathBuf};

/// A `.dsc` file: the `[name]` / `[friendlyname]` lines and the `[description]` text.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Dsc {
    pub name: Vec<String>,
    pub description: String,
}

impl Dsc {
    /// The file decoded with OMSI's text rules (UTF-16 with its mark, or the code page it
    /// was written in).
    pub fn from_bytes(bytes: &[u8]) -> Dsc {
        Dsc::parse(&omsi_cfg::decode_text(bytes))
    }

    /// The sections are matched trimmed and in any case; a `[name]` ends at its first
    /// empty line.
    pub fn parse(text: &str) -> Dsc {
        let mut name = Vec::new();
        let mut desc: Vec<&str> = Vec::new();
        let mut section = "";
        for line in text.lines() {
            let t = line.trim();
            if t.starts_with('[') && t.ends_with(']') {
                section = if t.eq_ignore_ascii_case("[name]")
                    || t.eq_ignore_ascii_case("[friendlyname]")
                {
                    "name"
                } else if t.eq_ignore_ascii_case("[description]") {
                    "description"
                } else {
                    ""
                };
                continue;
            }
            match section {
                "name" if !t.is_empty() => name.push(t.to_string()),
                "name" => section = "",
                "description" => desc.push(line.trim_end()),
                _ => {}
            }
        }
        Dsc {
            name,
            description: desc.join("\n").trim().to_string(),
        }
    }

    /// The description files OMSI reads for `file` (`global.cfg` -> `global_ENG.dsc`), in
    /// the order they are tried: the language itself, then English for a language that has
    /// none (German is the files' own language, so German falls back to the file itself).
    pub fn candidates(file: &Path, lang: &str) -> Vec<PathBuf> {
        let stem = file
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let mut langs = vec![lang];
        if lang != "DEU" && lang != "ENG" {
            langs.push("ENG");
        }
        langs
            .into_iter()
            .map(|l| file.with_file_name(format!("{stem}_{l}.dsc")))
            .collect()
    }

    /// The first of [`Dsc::candidates`] that can be read.
    pub fn find(file: &Path, lang: &str) -> Option<Dsc> {
        Dsc::candidates(file, lang)
            .iter()
            .find_map(|p| omsi_cfg::vfs::read(p).ok())
            .map(|b| Dsc::from_bytes(&b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dsc_files_give_name_and_description() {
        let d = Dsc::parse("\r\n[friendlyname]\r\nMAN\r\nNL202 - EN92\r\nBeige\r\n\r\n[description]\r\nAlthough the BVG did not purchase\r\n\r\n-Technical specifications-\r\n[end]\r\n");
        assert_eq!(d.name, vec!["MAN", "NL202 - EN92", "Beige"]);
        assert_eq!(
            d.description,
            "Although the BVG did not purchase\n\n-Technical specifications-"
        );
        let w = Dsc::parse("[name]\r\nGround Fog\r\n\r\n[description]\r\nHeavy ground fog limits the maximum visibility dangerously!\r\n[end]\r\n");
        assert_eq!(w.name, vec!["Ground Fog"]);
        assert_eq!(
            w.description,
            "Heavy ground fog limits the maximum visibility dangerously!"
        );
    }

    #[test]
    fn candidates_follow_the_language() {
        let p = Path::new("/x/maps/Spandau/global.cfg");
        assert_eq!(
            Dsc::candidates(p, "ENG"),
            vec![PathBuf::from("/x/maps/Spandau/global_ENG.dsc")]
        );
        assert_eq!(Dsc::candidates(p, "FRA").len(), 2);
        assert_eq!(
            Dsc::candidates(Path::new("/v/MAN_EN92_main.bus"), "DEU"),
            vec![PathBuf::from("/v/MAN_EN92_main_DEU.dsc")]
        );
    }

    #[test]
    fn descriptions_are_read_in_the_code_page_they_were_written_in() {
        // "Красный" and "Łódź" as a Russian and a Polish author's Windows saved them
        let ru = [
            b"[name]\r\n".as_slice(),
            &[0xCA, 0xF0, 0xE0, 0xF1, 0xED, 0xFB, 0xE9],
            b"\r\n",
        ]
        .concat();
        assert_eq!(Dsc::from_bytes(&ru).name, vec!["Красный"]);
        let pl = [
            b"[name]\r\n".as_slice(),
            &[0xA3, 0xF3, 0x64, 0x9F, 0x20, 0xA3, 0xF3, 0x64, 0x9F],
            b"\r\n",
        ]
        .concat();
        assert_eq!(Dsc::from_bytes(&pl).name, vec!["Łódź Łódź"]);
        // a Western one, with the 1252 quotation marks Latin-1 has no letters for
        let de = [
            b"[description]\r\n".as_slice(),
            &[0x84, 0x47, 0x72, 0xFC, 0x6E, 0x93],
            b"\r\n",
        ]
        .concat();
        assert_eq!(Dsc::from_bytes(&de).description, "„Grün“");
        let utf16 = omsi_cfg::encode_utf16_bom("[name]\r\nGrünau\r\n");
        assert_eq!(Dsc::from_bytes(&utf16).name, vec!["Grünau"]);
    }
}
