//! A timetable line (`*.ttl`) with the AI group of its tours changed, line by line. A change
//! touches only the group's line: every other byte (comments, the `Dep.:` notes, the order,
//! the code page, the line breaks) is written back as it was read.
//!
//! The tours are read as `omsi_timetable::Line::parse` reads them (each `[newtour]`, then its
//! number, AI group and extra line), so tour `t` here is tour `t` there.

use crate::ailists_edit::TextLines;

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
        // (the keywords as `omsi_timetable::Line::parse` takes them: in lower case only)
        let mut r = f.reader();
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
