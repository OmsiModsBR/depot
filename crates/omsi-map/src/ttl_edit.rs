//! A timetable line (`*.ttl`) with the AI group of its tours changed, line by line. A change
//! touches only the group's line: every other byte (comments, the `Dep.:` notes, the order,
//! the code page, the line breaks) is written back as it was read.
//!
//! The tours are read as `omsi_timetable::Line::parse` reads them (each `[newtour]`, then its
//! number, AI group and extra line), so tour `t` here is tour `t` there.

use crate::ailists_edit::TextLines;
use omsi_cfg::KeywordRule;

/// What a change of a line's tour cannot do.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TtlEditError {
    #[error("there is no such tour")]
    NoTour,
    #[error("the tour ends before its AI group line")]
    Truncated,
    #[error("an AI group needs a name without a tab or a line break")]
    BadGroup,
    #[error("the file's text encoding cannot hold this name")]
    Unwritable,
}

/// A tour of the line and where its group is in the file.
#[derive(Debug, Clone, PartialEq)]
pub struct DocTour {
    pub number: String,
    pub ai_group: String,
    group_line: Option<usize>,
}

/// A `.ttl` to change and write back.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TtlDoc {
    text: TextLines,
    tours: Vec<DocTour>,
}

impl TtlDoc {
    pub fn from_bytes(bytes: &[u8]) -> TtlDoc {
        let mut d = TtlDoc {
            text: TextLines::from_bytes(bytes),
            tours: Vec::new(),
        };
        d.index();
        d
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.text.to_bytes()
    }

    /// Whether the file can be written back unchanged (see [`TextLines::exact`]).
    pub fn exact(&self) -> bool {
        self.text.exact()
    }

    pub fn tours(&self) -> &[DocTour] {
        &self.tours
    }

    fn index(&mut self) {
        let f = self.text.cfg();
        let n = f.lines.len();
        let mut r = f.reader().with_rule(KeywordRule::AnyCase);
        let mut tours = Vec::new();
        while let Some(k) = r.next_keyword() {
            if k != "newtour" {
                continue;
            }
            let number = r.str().to_string();
            let at = r.pos();
            let ai_group = r.str().to_string();
            tours.push(DocTour {
                number,
                ai_group,
                group_line: (at < n).then_some(at),
            });
        }
        self.tours = tours;
    }

    /// The AI group of tour `t` becomes `group`.
    pub fn set_group(&mut self, t: usize, group: &str) -> Result<(), TtlEditError> {
        let group = group.trim();
        if group.is_empty() || group.contains(['\t', '\r', '\n']) {
            return Err(TtlEditError::BadGroup);
        }
        if !self.text.holds(group) {
            return Err(TtlEditError::Unwritable);
        }
        let line = self
            .tours
            .get(t)
            .ok_or(TtlEditError::NoTour)?
            .group_line
            .ok_or(TtlEditError::Truncated)?;
        self.text.set(line, group);
        self.index();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "-----\r\nTime Table Line File\r\n-----\r\n\r\n[userallowed]\r\n\r\n[priority]\r\n1\r\n\
        ------\r\n[newtour]\r\n1\r\nBusse\r\n\r\n------\r\n\r\n  Dep.: 5:12:0\r\n[addtrip]\r\nA\r\n0\r\n312.000\r\n\r\n\
        [newtour]\r\n2\r\nBusse_Gelenk\r\n0\r\n[addtrip]\r\nB\r\n0\r\n400.000\r\n";

    #[test]
    fn a_tour_changes_its_group_line_only() {
        let mut d = TtlDoc::from_bytes(SAMPLE.as_bytes());
        assert_eq!(d.to_bytes(), SAMPLE.as_bytes());
        assert_eq!(d.tours()[1].ai_group, "Busse_Gelenk");
        d.set_group(1, "Busse").unwrap();
        assert_eq!(
            String::from_utf8(d.to_bytes()).unwrap(),
            SAMPLE.replace("Busse_Gelenk", "Busse")
        );
        let groups: Vec<_> = d.tours().iter().map(|t| t.ai_group.clone()).collect();
        assert_eq!(groups, ["Busse", "Busse"]);
    }

    #[test]
    fn refused_changes_leave_the_file() {
        let mut d = TtlDoc::from_bytes(SAMPLE.as_bytes());
        assert_eq!(d.set_group(5, "x"), Err(TtlEditError::NoTour));
        assert_eq!(d.set_group(0, " "), Err(TtlEditError::BadGroup));
        assert_eq!(d.set_group(0, "a\tb"), Err(TtlEditError::BadGroup));
        assert_eq!(d.to_bytes(), SAMPLE.as_bytes());
        let mut cut = TtlDoc::from_bytes(b"[newtour]\r\n7");
        assert_eq!(cut.set_group(0, "x"), Err(TtlEditError::Truncated));
    }
}
