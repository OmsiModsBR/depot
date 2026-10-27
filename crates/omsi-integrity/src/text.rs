//! The report as plain text, to save or paste where it is sent: a header, the counts, then
//! every finding with why it matters and which file uses it. English, as the findings are,
//! with Windows line endings so Notepad and the forums show it as it is.

use crate::report::{Category, Finding, Report, Severity};

/// What goes into the text.
#[derive(Debug, Clone)]
pub struct TextOptions {
    /// Lines of the header after the title, as (name, value): what was checked, when, with
    /// which build, compared with what. The caller knows them; the report does not.
    pub header: Vec<(String, String)>,
    /// Errors, warnings, not optimized: which of them are listed.
    pub severities: [bool; 3],
    /// Why each finding matters.
    pub why: bool,
    /// Which file names it.
    pub used_by: bool,
    /// The findings the compared content has too (see [`Report::mark_known`]).
    pub known: bool,
    /// Sections by severity instead of by kind of file.
    pub by_severity: bool,
    /// Only the findings of this kind of file.
    pub only: Option<Category>,
    /// The findings themselves; without them the text is the summary alone.
    pub items: bool,
}

impl Default for TextOptions {
    fn default() -> Self {
        TextOptions {
            header: Vec::new(),
            severities: [true; 3],
            why: true,
            used_by: true,
            known: false,
            by_severity: false,
            only: None,
            items: true,
        }
    }
}

const NL: &str = "\r\n";
/// Where a long explanation is broken.
const WIDTH: usize = 96;
const INDENT: &str = "        ";

fn severity_index(s: Severity) -> usize {
    match s {
        Severity::Error => 0,
        Severity::Warning => 1,
        Severity::NotOptimized => 2,
    }
}

fn section_name(s: Severity) -> &'static str {
    match s {
        Severity::Error => "Errors",
        Severity::Warning => "Warnings",
        Severity::NotOptimized => "Not optimized",
    }
}

/// `text` broken into lines of at most [`WIDTH`] characters, each after `first` (the first
/// line) or [`INDENT`] plus as many spaces as `first` is long.
fn wrapped(out: &mut String, first: &str, text: &str) {
    let pad = format!("{INDENT}{}", " ".repeat(first.chars().count()));
    let mut line = format!("{INDENT}{first}");
    let mut empty = true;
    for word in text.split_whitespace() {
        if !empty && line.chars().count() + 1 + word.chars().count() > WIDTH {
            out.push_str(&line);
            out.push_str(NL);
            line = pad.clone();
            empty = true;
        }
        if !empty {
            line.push(' ');
        }
        line.push_str(word);
        empty = false;
    }
    out.push_str(&line);
    out.push_str(NL);
}

/// `text` with the home folder `home` (as `\\` or `/` is written) as `%USERPROFILE%`.
fn without_home(text: String, home: Option<&std::path::Path>) -> String {
    let Some(home) = home.map(|h| h.display().to_string()) else {
        return text;
    };
    let home = home.trim_end_matches(['/', '\\']);
    // (a home of "/" or "C:" names no one)
    if home.len() < 3 {
        return text;
    }
    [home.replace('/', "\\"), home.replace('\\', "/")]
        .iter()
        .fold(text, |t, h| t.replace(h.as_str(), "%USERPROFILE%"))
}

impl Report {
    /// The report as text (see [`TextOptions`]). The user's home folder, where a path still
    /// names it, is written `%USERPROFILE%`: the text is made to be sent along.
    pub fn to_text(&self, o: &TextOptions) -> String {
        without_home(self.text(o), omsi_cfg::paths::home().as_deref())
    }

    fn text(&self, o: &TextOptions) -> String {
        let listed: Vec<&Finding> = self
            .findings
            .iter()
            .filter(|f| o.severities[severity_index(f.severity)])
            .filter(|f| o.known || !f.known)
            .filter(|f| o.only.is_none_or(|c| f.category() == c))
            .collect();
        let mut out = String::new();
        out.push_str("Project Depot - integrity report");
        out.push_str(NL);
        let wide = o
            .header
            .iter()
            .map(|(k, _)| k.chars().count())
            .chain(["Content".len()])
            .max()
            .unwrap_or(0);
        for (k, v) in &o.header {
            out.push_str(&format!("{k:<wide$} : {v}{NL}"));
        }
        out.push_str(&format!(
            "{:<wide$} : {} tiles, {} objects, {} splines, {} meshes, {} textures{NL}",
            "Content", self.tiles, self.objects, self.splines, self.meshes, self.textures
        ));
        if self.cancelled {
            out.push_str(
                "The check was stopped before its end: only part of the content was looked at.",
            );
            out.push_str(NL);
        }
        out.push_str(NL);
        let n = |s: Severity| listed.iter().filter(|f| f.severity == s).count();
        out.push_str(&format!(
            "Result: {} errors, {} warnings, {} not optimized{NL}",
            n(Severity::Error),
            n(Severity::Warning),
            n(Severity::NotOptimized)
        ));
        // (the known findings the other options would have listed)
        let left_known = self
            .findings
            .iter()
            .filter(|f| f.known && !o.known)
            .filter(|f| o.severities[severity_index(f.severity)])
            .filter(|f| o.only.is_none_or(|c| f.category() == c))
            .count();
        if left_known > 0 {
            out.push_str(&format!(
                "Left out: {left_known} the compared one has too.{NL}"
            ));
        }
        if let Some(c) = o.only {
            out.push_str(&format!("Only: {}{NL}", c.label()));
        }
        if !o.items {
            // the summary: how many of each kind
            for c in Category::ALL {
                let k = listed.iter().filter(|f| f.category() == c).count();
                if k > 0 {
                    out.push_str(&format!("  {:<12}{k}{NL}", c.label()));
                }
            }
            return out;
        }
        let item = |out: &mut String, f: &Finding, tag: &str| {
            out.push_str(&format!("[{tag}] {}{NL}", f.file));
            wrapped(out, "", &f.message);
            if o.why {
                if let Some(d) = &f.detail {
                    wrapped(out, "Why: ", d);
                }
            }
            if o.used_by {
                if let Some((by, k)) = &f.referenced_by {
                    let more = if *k > 1 {
                        format!(" and {} more", k - 1)
                    } else {
                        String::new()
                    };
                    wrapped(out, "Used by: ", &format!("{by}{more}"));
                }
            }
            if f.known {
                wrapped(out, "", "(already in the compared one)");
            }
        };
        if o.by_severity {
            for s in [Severity::Error, Severity::Warning, Severity::NotOptimized] {
                let these: Vec<&&Finding> = listed.iter().filter(|f| f.severity == s).collect();
                if these.is_empty() {
                    continue;
                }
                out.push_str(NL);
                out.push_str(&format!("== {} ({}) =={NL}", section_name(s), these.len()));
                for f in these {
                    item(&mut out, f, f.category().label());
                }
            }
        } else {
            for c in Category::ALL {
                let mut these: Vec<&&Finding> =
                    listed.iter().filter(|f| f.category() == c).collect();
                if these.is_empty() {
                    continue;
                }
                these.sort_by_key(|f| f.severity);
                out.push_str(NL);
                out.push_str(&format!("== {} ({}) =={NL}", c.label(), these.len()));
                for f in these {
                    item(&mut out, f, f.severity.label());
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finding(severity: Severity, code: &'static str, file: &str) -> Finding {
        Finding {
            severity,
            code,
            file: file.into(),
            message: "A texture a mesh names is missing; the surface shows white.".into(),
            detail: None,
            referenced_by: Some(("Sceneryobjects/a/house.sco".into(), 4)),
            known: false,
        }
    }

    fn report() -> Report {
        let mut npot = finding(Severity::NotOptimized, "texture-npot", "Texture/big.bmp");
        npot.message = "1400x1400: not a power of two, takes 2048x2048 in video memory.".into();
        npot.detail = Some("OMSI 2 uses DirectX 9, which enlarges a texture whose size is not a power of two to the next one, so this file takes far more memory than it looks.".into());
        npot.referenced_by = Some(("Sceneryobjects/a/model/house.x".into(), 1));
        let mut known = finding(
            Severity::Error,
            "object-missing",
            "Sceneryobjects/b/lamp.sco",
        );
        known.known = true;
        Report {
            map: "Test".into(),
            findings: vec![
                finding(Severity::Warning, "texture-missing", "Texture/wall.bmp"),
                known,
                npot,
            ],
            tiles: 2,
            objects: 5,
            splines: 1,
            meshes: 3,
            textures: 4,
            cancelled: false,
        }
    }

    #[test]
    fn findings_are_grouped_by_kind_with_why_and_users() {
        let o = TextOptions {
            header: vec![("Map".into(), "Test (Maps/Test)".into())],
            ..Default::default()
        };
        let t = report().to_text(&o);
        assert!(
            t.contains("\r\n") && !t.replace("\r\n", "").contains('\n'),
            "{t}"
        );
        let lines: Vec<&str> = t.split("\r\n").collect();
        assert_eq!(lines[0], "Project Depot - integrity report");
        assert_eq!(lines[1], "Map     : Test (Maps/Test)");
        assert_eq!(
            lines[2],
            "Content : 2 tiles, 5 objects, 1 splines, 3 meshes, 4 textures"
        );
        // the finding the compared one has too is left out, and said so
        assert!(
            t.contains("Result: 0 errors, 1 warnings, 1 not optimized"),
            "{t}"
        );
        assert!(t.contains("Left out: 1 the compared one has too."), "{t}");
        assert!(!t.contains("lamp.sco"), "{t}");
        // one section for textures, the warning before the not optimized
        assert!(t.contains("== Textures (2) =="), "{t}");
        let w = t.find("[warning] Texture/wall.bmp").unwrap();
        let n = t.find("[not optimized] Texture/big.bmp").unwrap();
        assert!(w < n, "{t}");
        assert!(
            t.contains("        Used by: Sceneryobjects/a/house.sco and 3 more"),
            "{t}"
        );
        assert!(
            t.contains("        Used by: Sceneryobjects/a/model/house.x\r\n"),
            "{t}"
        );
        // a long explanation is broken, its next lines under the first's text
        let why = lines
            .iter()
            .position(|l| l.starts_with("        Why: OMSI 2"))
            .unwrap();
        assert!(lines[why].len() <= WIDTH, "{t}");
        assert!(lines[why + 1].starts_with("             "), "{t}");
    }

    #[test]
    fn options_choose_what_is_listed() {
        let o = TextOptions {
            known: true,
            by_severity: true,
            why: false,
            used_by: false,
            severities: [true, true, false],
            ..Default::default()
        };
        let t = report().to_text(&o);
        assert!(t.contains("== Errors (1) =="), "{t}");
        assert!(t.contains("[Objects] Sceneryobjects/b/lamp.sco"), "{t}");
        assert!(t.contains("(already in the compared one)"), "{t}");
        assert!(
            !t.contains("big.bmp") && !t.contains("Used by") && !t.contains("Why"),
            "{t}"
        );

        let only = TextOptions {
            only: Some(Category::Objects),
            known: true,
            ..Default::default()
        };
        let t = report().to_text(&only);
        assert!(
            t.contains("Only: Objects") && !t.contains("wall.bmp"),
            "{t}"
        );

        let summary = TextOptions {
            items: false,
            ..Default::default()
        };
        let t = report().to_text(&summary);
        assert!(t.contains("  Textures    2"), "{t}");
        assert!(!t.contains("[warning]"), "{t}");
    }

    #[test]
    fn a_stopped_check_says_so() {
        let mut r = report();
        r.cancelled = true;
        assert!(r
            .to_text(&TextOptions::default())
            .contains("The check was stopped before its end"));
    }

    #[test]
    fn categories_follow_the_codes() {
        assert_eq!(Category::of("texture-npot"), Category::Textures);
        assert_eq!(Category::of("mesh-missing"), Category::Models);
        assert_eq!(Category::of("path-too-long"), Category::Paths);
        assert_eq!(Category::of("paths-missing"), Category::Vehicle);
        assert_eq!(Category::of("cabin-missing"), Category::Vehicle);
        assert_eq!(Category::of("map-unreadable"), Category::Map);
        assert_eq!(Category::of("ai-vehicle-missing"), Category::AiLists);
        assert_eq!(Category::of("tour-group-unknown"), Category::Timetable);
    }

    #[test]
    fn the_home_folder_is_not_in_the_text() {
        let home = std::path::Path::new("C:\\Users\\leo");
        let t = "[error] C:\\Users\\leo\\Depot\\a.sco and C:/Users/leo/x.dds".to_string();
        assert_eq!(
            without_home(t, Some(home)),
            "[error] %USERPROFILE%\\Depot\\a.sco and %USERPROFILE%/x.dds"
        );
        assert_eq!(
            without_home("/a/b".into(), Some(std::path::Path::new("/"))),
            "/a/b"
        );
    }
}
