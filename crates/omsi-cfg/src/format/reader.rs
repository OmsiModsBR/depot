//! The sequential reader over a keyword file.

use std::path::Path;

use super::file::CfgFile;
use super::keyword::{hof_trailing, keyword_with, KeywordRule};
use crate::text::number::{parse_f32, parse_f64, parse_i32, parse_i64};

/// What [`CfgReader::next_entry`] stopped at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry<'t> {
    /// A keyword line: the keyword in lower case.
    Keyword(String),
    /// A line that is exactly one of the bare tokens asked for.
    Token(&'t str),
}

/// Sequential reader over a [`CfgFile`] with the original semantics.
#[derive(Clone)]
pub struct CfgReader<'a> {
    file: &'a CfgFile,
    pos: usize,
    block_line: usize,
    rule: KeywordRule,
    disabled_blocks: bool,
}

impl<'a> CfgReader<'a> {
    /// Read keyword lines by `rule` instead of [`KeywordRule::Exact`].
    pub fn with_rule(mut self, rule: KeywordRule) -> Self {
        self.rule = rule;
        self
    }

    /// Skip `-<DISABLED>-` … `-<ENABLED>-` between blocks, as the original's loaders of
    /// `.bus`/`.ovh`, `.sco`, `model.cfg`, `passengercabin.cfg`, `paths.cfg`, `.hum` and
    /// `envir.cfg` do (the marker lines must stand alone, exactly so; a marker read as a
    /// parameter is a parameter). Mods switch whole meshes off this way.
    pub fn disabled_blocks(mut self) -> Self {
        self.disabled_blocks = true;
        self
    }

    /// Whether `line` is a keyword line by this reader's rule.
    pub fn keyword(&self, line: &'a str) -> Option<&'a str> {
        keyword_with(line, self.rule)
    }

    pub fn path(&self) -> &Path {
        &self.file.path
    }

    /// Current line index (0-based) of the *next* line to be read.
    pub fn pos(&self) -> usize {
        self.pos
    }

    /// 1-based line number of the keyword that opened the current block (for messages).
    pub fn block_line(&self) -> usize {
        self.block_line
    }

    pub fn seek(&mut self, pos: usize) {
        self.pos = pos.min(self.file.lines.len());
    }

    pub fn at_end(&self) -> bool {
        self.pos >= self.file.lines.len()
    }

    /// The next line as a parameter, or an empty string where a keyword line starts: a fixed
    /// list of parameters ends at the next block. An `.ovh` that leaves the registration
    /// affixes out (`[registration_automatic]` straight before `[model]`, the Urumqi AI cars)
    /// must not have the `[model]` read as one of them.
    pub fn param_line(&mut self) -> &'a str {
        if self
            .file
            .lines
            .get(self.pos)
            .is_some_and(|l| keyword_with(l, self.rule).is_some())
        {
            ""
        } else {
            self.line()
        }
    }

    /// Advance to the next keyword line and return the keyword in lower case.
    pub fn next_keyword(&mut self) -> Option<String> {
        match self.next_entry(&[]) {
            Some(Entry::Keyword(k)) => Some(k),
            _ => None,
        }
    }

    /// Advance to the next keyword line or the next line that is exactly one of `tokens`.
    /// The original's loaders see sub-commands such as `attach_trans` or `anim_rot` the way
    /// they see keywords: as a line of their own anywhere between blocks, spelled exactly so.
    pub fn next_entry<'t>(&mut self, tokens: &[&'t str]) -> Option<Entry<'t>> {
        while self.pos < self.file.lines.len() {
            let line = self.file.lines[self.pos].as_str();
            self.pos += 1;
            if let Some(k) = keyword_with(line, self.rule) {
                self.block_line = self.pos;
                return Some(Entry::Keyword(k.to_ascii_lowercase()));
            }
            if let Some(t) = tokens.iter().find(|t| **t == line) {
                self.block_line = self.pos;
                return Some(Entry::Token(t));
            }
            if self.disabled_blocks && line == "-<DISABLED>-" {
                while self.pos < self.file.lines.len() {
                    self.pos += 1;
                    if self.file.lines[self.pos - 1] == "-<ENABLED>-" {
                        break;
                    }
                }
            }
        }
        None
    }

    /// Peek at the next keyword without consuming anything.
    pub fn peek_keyword(&self) -> Option<String> {
        let mut c = self.clone();
        c.next_keyword()
    }

    /// Read the next raw parameter line. Missing lines at EOF read as empty strings, which is
    /// what the original does with a truncated file.
    pub fn line(&mut self) -> &'a str {
        if self.pos < self.file.lines.len() {
            let l = &self.file.lines[self.pos];
            self.pos += 1;
            l.as_str()
        } else {
            self.pos = self.file.lines.len() + 1;
            ""
        }
    }

    /// Like [`line`](Self::line) but with trailing whitespace removed.
    pub fn str(&mut self) -> &'a str {
        self.line().trim_end()
    }

    /// Trimmed on both sides (numbers, identifiers).
    pub fn word(&mut self) -> &'a str {
        self.line().trim()
    }

    pub fn f64(&mut self) -> f64 {
        parse_f64(self.line())
    }

    pub fn f32(&mut self) -> f32 {
        parse_f32(self.line())
    }

    pub fn i32(&mut self) -> i32 {
        parse_i32(self.line())
    }

    /// `n` integers, then any further whole-number lines that follow at once (a blank line,
    /// a keyword or anything else ends the list): OMSI's fixed-length lists read as always,
    /// and a file may give more (openOMSI: `[illumination_interior]` with more than four
    /// lamps).
    pub fn i32_list(&mut self, n: usize) -> Vec<i32> {
        let mut v: Vec<i32> = (0..n).map(|_| self.i32()).collect();
        while self.pos < self.file.lines.len() {
            let l = self.file.lines[self.pos].trim();
            let whole = !l.is_empty()
                && l.strip_prefix('-')
                    .unwrap_or(l)
                    .chars()
                    .all(|c| c.is_ascii_digit());
            if !whole || keyword_with(self.file.lines[self.pos].as_str(), self.rule).is_some() {
                break;
            }
            v.push(parse_i32(l));
            self.pos += 1;
        }
        v
    }

    pub fn i64(&mut self) -> i64 {
        parse_i64(self.line())
    }

    pub fn u32(&mut self) -> u32 {
        parse_i64(self.line()).max(0) as u32
    }

    pub fn usize(&mut self) -> usize {
        parse_i64(self.line()).max(0) as usize
    }

    /// A count of elements that follow (the number of entries of a list), safe to loop or
    /// allocate over: never more than `max`, and never more than the lines left plus one
    /// (every element takes a line; past the end only blank lines come, so a count of 10^11
    /// in a three-line file is not 10^11 iterations). A count that had to be cut is logged.
    pub fn count(&mut self, max: usize) -> usize {
        let v = self.usize();
        let left = self
            .file
            .lines
            .len()
            .saturating_sub(self.pos)
            .saturating_add(1);
        let n = v.min(max).min(left);
        if v > max {
            log::warn!(
                "{}: count {v} past the limit of {max}, cut",
                self.file.path.display()
            );
        }
        n
    }

    /// `1`/`true` → true; anything else false.
    pub fn bool(&mut self) -> bool {
        let w = self.word();
        w == "1" || w.eq_ignore_ascii_case("true")
    }

    /// Read `n` lines as `f32`.
    pub fn f32s<const N: usize>(&mut self) -> [f32; N] {
        let mut a = [0.0; N];
        for v in a.iter_mut() {
            *v = self.f32();
        }
        a
    }

    pub fn f64s<const N: usize>(&mut self) -> [f64; N] {
        let mut a = [0.0; N];
        for v in a.iter_mut() {
            *v = self.f64();
        }
        a
    }

    /// Read all lines up to (not including) a line that is `terminator` (by the reader's
    /// rule, like a keyword). Used for `[description] … [end]`.
    pub fn until(&mut self, terminator: &str) -> Vec<&'a str> {
        let mut out = Vec::new();
        while self.pos < self.file.lines.len() {
            let l = self.file.lines[self.pos].as_str();
            self.pos += 1;
            let hit = match self.rule {
                KeywordRule::Exact => l == terminator,
                KeywordRule::TrimEnd => l.trim_end_matches(hof_trailing) == terminator,
                KeywordRule::AnyCase => l.eq_ignore_ascii_case(terminator),
            };
            if hit {
                break;
            }
            out.push(l);
        }
        out
    }

    /// All remaining lines that are not keywords, until the next keyword (not consumed).
    pub fn rest_of_block(&mut self) -> Vec<&'a str> {
        let mut out = Vec::new();
        while self.pos < self.file.lines.len() {
            let l = self.file.lines[self.pos].as_str();
            if keyword_with(l, self.rule).is_some() {
                break;
            }
            self.pos += 1;
            out.push(l);
        }
        out
    }

    /// Raw access to the lines.
    pub fn lines(&self) -> &'a [String] {
        &self.file.lines
    }
}

impl CfgFile {
    pub fn reader(&self) -> CfgReader<'_> {
        CfgReader {
            file: self,
            pos: 0,
            block_line: 0,
            rule: KeywordRule::Exact,
            disabled_blocks: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The stock files' indented keywords are the original's help texts and switched-off
    /// blocks: the SD202 cabin's `[exit]` help line adds no exit.
    #[test]
    fn indented_help_text() {
        let text = "\t[entry]\t\tdefiniert Eingang\r\n\t[exit]\r\n\tnum\t\tanalog f\u{fc}r Ausgang\r\n\r\n###\r\n[exit]\r\n20\r\n";
        let f = CfgFile::from_str("passengercabin.cfg", text);
        let mut r = f.reader();
        assert_eq!(r.next_keyword().as_deref(), Some("exit"));
        assert_eq!(r.i32(), 20);
        assert_eq!(r.next_keyword(), None);
    }

    #[test]
    fn disabled_blocks() {
        let text = "[mesh]\na.o3d\n-<DISABLED>-\n[mesh]\nb.o3d\n-<ENABLED>-\n[mesh]\n-<DISABLED>-\n[mesh]\nd.o3d\n";
        let f = CfgFile::from_str("model.cfg", text);
        let mut r = f.reader().disabled_blocks();
        let mut meshes = Vec::new();
        while let Some(k) = r.next_keyword() {
            assert_eq!(k, "mesh");
            meshes.push(r.str().to_string());
        }
        // a marker read as a parameter is a parameter; an unclosed block runs to the end
        assert_eq!(meshes, vec!["a.o3d", "-<DISABLED>-", "d.o3d"]);
        // loaders without the feature see the blocks
        let mut r = f.reader();
        let mut n = 0;
        while r.next_keyword().is_some() {
            n += 1;
        }
        assert_eq!(n, 4);
        let f = CfgFile::from_str(
            "model.cfg",
            "-<DISABLED>-\n[mesh]\nx\n -<ENABLED>-\n[mesh]\ny\n",
        );
        assert_eq!(f.reader().disabled_blocks().next_keyword(), None);
    }

    #[test]
    fn bare_tokens() {
        let text = "[new_attachment]\n\ntext\n[complexity]\n2\n\nattach_trans\n1\n2\n3\n\tattach_rot_x\nAttach_rot_y\n";
        let f = CfgFile::from_str("x.sco", text);
        let mut r = f.reader();
        const T: &[&str] = &["attach_trans", "attach_rot_x", "attach_rot_y"];
        assert_eq!(
            r.next_entry(T),
            Some(Entry::Keyword("new_attachment".into()))
        );
        assert_eq!(r.next_entry(T), Some(Entry::Keyword("complexity".into())));
        assert_eq!(r.i32(), 2);
        assert_eq!(r.next_entry(T), Some(Entry::Token("attach_trans")));
        assert_eq!(r.f32s::<3>(), [1.0, 2.0, 3.0]);
        assert_eq!(r.next_entry(T), None);
    }

    #[test]
    fn until_follows_the_rule() {
        let f = CfgFile::from_str("x.owt", "[description]\na\n[END]\n[end] \nb\n[end]\nc\n");
        let mut r = f.reader();
        r.next_keyword();
        assert_eq!(r.until("[end]"), vec!["a", "[END]", "[end] ", "b"]);
        let f = CfgFile::from_str("x.hof", "[addbusstop_list]\na\t\n[end]\t\t\nc\n");
        let mut r = f.reader().with_rule(KeywordRule::TrimEnd);
        r.next_keyword();
        assert_eq!(r.until("[end]"), vec!["a\t"]);
        assert_eq!(r.str(), "c");
    }

    #[test]
    fn reader() {
        let f = CfgFile::from_str("x.cfg", "hello\r\n[mesh]\r\na.o3d\r\n\r\n[LOD]\r\n0.5\r\n");
        let mut r = f.reader();
        assert_eq!(r.next_keyword().as_deref(), Some("mesh"));
        assert_eq!(r.str(), "a.o3d");
        assert_eq!(r.next_keyword().as_deref(), Some("lod"));
        assert_eq!(r.f32(), 0.5);
        assert_eq!(r.next_keyword(), None);
    }
}
