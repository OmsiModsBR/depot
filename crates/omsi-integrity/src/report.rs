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
