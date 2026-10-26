use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Error,
    Warning,
    NotOptimized,
}

impl Severity {
    pub fn label(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::NotOptimized => "not optimized",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub severity: Severity,
    /// Stable id of the kind of finding (`texture-missing`, `texture-npot`, ...).
    pub code: &'static str,
    /// The file the finding is about, relative to the installation when it lies in it.
    pub file: String,
    pub message: String,
    /// Why it matters, for a tooltip next to the result.
    pub detail: Option<String>,
    /// The first file that names it, and how many files do.
    pub referenced_by: Option<(String, usize)>,
    /// The same finding is in the baseline report (see [`Report::mark_known`]): it was there
    /// before, not something a change made.
    pub known: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub map: String,
    pub findings: Vec<Finding>,
    pub tiles: usize,
    pub objects: usize,
    pub splines: usize,
    pub textures: usize,
}

impl Report {
    pub fn count(&self, s: Severity) -> usize {
        self.findings.iter().filter(|f| f.severity == s).count()
    }

    /// Mark the findings that `baseline` (the same content checked as it was, e.g. the
    /// untouched original of an optimized map) has too, by kind and file.
    pub fn mark_known(&mut self, baseline: &Report) {
        let before: std::collections::HashSet<(&str, &str)> = baseline
            .findings
            .iter()
            .map(|f| (f.code, f.file.as_str()))
            .collect();
        for f in &mut self.findings {
            f.known = before.contains(&(f.code, f.file.as_str()));
        }
    }

    /// Findings of a severity that are not in the baseline.
    pub fn count_new(&self, s: Severity) -> usize {
        self.findings
            .iter()
            .filter(|f| f.severity == s && !f.known)
            .count()
    }

    /// Findings per code, for a summary line.
    pub fn by_code(&self) -> BTreeMap<&'static str, usize> {
        let mut m = BTreeMap::new();
        for f in &self.findings {
            *m.entry(f.code).or_insert(0) += 1;
        }
        m
    }

    /// True when nothing is an error: the map is complete.
    pub fn is_complete(&self) -> bool {
        self.count(Severity::Error) == 0
    }
}
