//! What the Integrity tab of the Diagnostics page keeps between frames: its choices, the
//! check running or done and the export window.

use omsi_integrity::{Category, Progress, Report, Severity};
use std::sync::mpsc::Receiver;
use std::sync::Arc;

/// The Integrity tab's choices and its check: what is checked and compared with, the check
/// running or done, the result's tab, filters and search, and the export window.
pub struct CheckView {
    /// What is checked: 0 a map, 1 a vehicle.
    pub(crate) kind: usize,
    pub(crate) item: usize,
    /// The map or vehicle compared with (0: none; k: the list's k - 1th).
    pub(crate) compare: usize,
    pub(crate) run: Option<Run>,
    pub(crate) done: Option<Done>,
    /// The result's tab: 0 all, k the kind of file `tabs(..)[k - 1]`.
    pub(crate) cat: usize,
    /// Errors, warnings, not optimized: which are shown.
    pub(crate) show: [bool; 3],
    pub(crate) search: String,
    /// The export window, while it is open.
    pub(crate) export: Option<Export>,
}

impl Default for CheckView {
    fn default() -> Self {
        CheckView {
            kind: 0,
            item: 0,
            compare: 0,
            run: None,
            done: None,
            cat: 0,
            show: [true; 3],
            search: String::new(),
            export: None,
        }
    }
}

impl CheckView {
    /// The export window is open over the page.
    pub fn export_open(&self) -> bool {
        self.export.is_some()
    }
}

/// A check running in the background; cancelling it stops it within a file read.
pub(crate) struct Run {
    pub(crate) rx: Receiver<Report>,
    pub(crate) progress: Arc<Progress>,
}

pub struct Done {
    pub name: String,
    /// The map's folder or the vehicle's file, as checked.
    pub target: String,
    /// The name of what it was compared with.
    pub compared: Option<String>,
    /// When it was checked (`YYYY-MM-DD HH:MM`), for the report.
    pub when: String,
    pub report: Report,
}

/// The export window's choices.
pub(crate) struct Export {
    pub(crate) severities: [bool; 3],
    pub(crate) why: bool,
    pub(crate) used_by: bool,
    pub(crate) known: bool,
    /// 0 sections by kind of file, 1 by severity.
    pub(crate) group: usize,
    /// 0 every kind, 1 only the result's open tab.
    pub(crate) only_tab: usize,
}

/// One line of the result.
pub enum Row {
    Head(Severity, usize),
    Item {
        severity: Severity,
        category: Category,
        file: String,
        message: String,
        tip: String,
        known: bool,
    },
}
