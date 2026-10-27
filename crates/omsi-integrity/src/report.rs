use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

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

/// What a finding is about, for the result's tabs (one per kind of file).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Category {
    /// The map's own control files (`global.cfg`).
    Map,
    Tiles,
    Objects,
    Splines,
    /// `model.cfg` files and the meshes they name.
    Models,
    Textures,
    Sounds,
    Scripts,
    /// A vehicle's own files: the vehicle file, its paths and passenger cabin.
    Vehicle,
    /// `ailists.cfg`, the parking lists and the people of a map.
    AiLists,
    /// The timetable (`TTData`) and the AI groups its tours name.
    Timetable,
    /// Paths too long for Windows.
    Paths,
}

impl Category {
    /// Every category, in the order of the tabs.
    pub const ALL: [Category; 12] = [
        Category::Map,
        Category::Tiles,
        Category::Objects,
        Category::Splines,
        Category::Models,
        Category::Textures,
        Category::Sounds,
        Category::Scripts,
        Category::Vehicle,
        Category::AiLists,
        Category::Timetable,
        Category::Paths,
    ];

    /// The category of a finding's code (`texture-missing` is about textures).
    pub fn of(code: &str) -> Category {
        let head = code.split('-').next().unwrap_or(code);
        match head {
            "map" => Category::Map,
            "tile" => Category::Tiles,
            "object" => Category::Objects,
            "spline" => Category::Splines,
            "model" | "mesh" => Category::Models,
            "texture" => Category::Textures,
            "sound" => Category::Sounds,
            "script" => Category::Scripts,
            "ai" | "parked" | "human" | "driver" | "depot" => Category::AiLists,
            "tour" | "timetable" => Category::Timetable,
            "path" => Category::Paths,
            _ => Category::Vehicle,
        }
    }

    /// The tab's name (an interface text: translated where it is shown).
    pub fn label(self) -> &'static str {
        match self {
            Category::Map => "Map",
            Category::Tiles => "Tiles",
            Category::Objects => "Objects",
            Category::Splines => "Splines",
            Category::Models => "Models",
            Category::Textures => "Textures",
            Category::Sounds => "Sounds",
            Category::Scripts => "Scripts",
            Category::Vehicle => "Vehicle",
            Category::AiLists => "AI lists",
            Category::Timetable => "Timetable",
            Category::Paths => "Paths",
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

impl Finding {
    pub fn category(&self) -> Category {
        Category::of(self.code)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub map: String,
    pub findings: Vec<Finding>,
    pub tiles: usize,
    pub objects: usize,
    pub splines: usize,
    /// Distinct mesh files read.
    pub meshes: usize,
    pub textures: usize,
    /// The check was stopped before its end (see [`Progress::cancel`]): what it holds is a
    /// part of the content only.
    pub cancelled: bool,
}

impl Report {
    /// Findings of a category.
    pub fn count_in(&self, c: Category) -> usize {
        self.findings.iter().filter(|f| f.category() == c).count()
    }

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

/// How far a running check is, and how its caller stops it. The check looks at the flag
/// between tiles and between the objects and meshes it reads, so a stop takes effect within a
/// file read.
#[derive(Debug, Default)]
pub struct Progress {
    cancel: AtomicBool,
    done: AtomicUsize,
    total: AtomicUsize,
}

impl Progress {
    pub fn new() -> Progress {
        Progress::default()
    }

    /// Stop the check: it returns what it has, marked [`Report::cancelled`].
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// The steps done and the steps in all (the tiles of a map), once the check knows them.
    pub fn steps(&self) -> Option<(usize, usize)> {
        let total = self.total.load(Ordering::Relaxed);
        (total > 0).then(|| (self.done.load(Ordering::Relaxed).min(total), total))
    }

    pub(crate) fn set_total(&self, n: usize) {
        self.total.store(n, Ordering::Relaxed);
    }

    pub(crate) fn step(&self) {
        self.done.fetch_add(1, Ordering::Relaxed);
    }
}
