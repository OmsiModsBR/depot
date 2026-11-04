//! A phone's storage browser: the folder it is in, what it holds, and what it chooses.

use std::path::PathBuf;

/// What the storage browser chooses.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Purpose {
    /// The OMSI 2 folder (Setup).
    Root,
}

/// The browser of the device's storage.
pub struct Browser {
    pub purpose: Purpose,
    pub dir: PathBuf,
    /// (name, is a folder, bytes)
    pub(crate) entries: Vec<(String, bool, u64)>,
    /// The folder is a complete OMSI 2 (Root only).
    pub(crate) is_root: bool,
    pub(crate) error: Option<String>,
}

/// The places a phone keeps files: the shared storage and any card or stick.
pub fn storage_roots() -> Vec<(String, PathBuf)> {
    let mut v = Vec::new();
    let shared = PathBuf::from("/storage/emulated/0");
    if shared.is_dir() {
        v.push(("Internal storage".to_string(), shared));
    }
    if let Ok(rd) = std::fs::read_dir("/storage") {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n == "emulated" || n == "self" {
                continue;
            }
            if e.path().is_dir() {
                v.push((format!("Card {n}"), e.path()));
            }
        }
    }
    if v.is_empty() {
        if let Some(h) = std::env::var_os("HOME") {
            v.push(("Home".to_string(), PathBuf::from(h)));
        }
    }
    v
}

impl Browser {
    pub fn new(purpose: Purpose, start: &str) -> Browser {
        let start = PathBuf::from(start.trim());
        let dir = if !start.as_os_str().is_empty() && start.is_dir() {
            start
        } else {
            storage_roots()
                .first()
                .map(|r| r.1.clone())
                .unwrap_or_else(|| PathBuf::from("/"))
        };
        let mut b = Browser {
            purpose,
            dir: PathBuf::new(),
            entries: Vec::new(),
            is_root: false,
            error: None,
        };
        b.open(dir);
        b
    }

    pub(crate) fn open(&mut self, dir: PathBuf) {
        self.entries.clear();
        self.error = None;
        match std::fs::read_dir(&dir) {
            Ok(rd) => {
                for e in rd.flatten() {
                    let name = e.file_name().to_string_lossy().to_string();
                    if name.starts_with('.') {
                        continue;
                    }
                    let Ok(m) = e.metadata() else { continue };
                    if m.is_dir() {
                        self.entries.push((name, m.is_dir(), m.len()));
                    }
                }
                self.entries.sort_by(|a, b| {
                    b.1.cmp(&a.1)
                        .then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase()))
                });
            }
            Err(e) => {
                self.error = Some(if e.kind() == std::io::ErrorKind::PermissionDenied {
                    "This folder cannot be read. Allow Project Depot access to all files (Android settings → Apps → Project Depot → Permissions → Files).".to_string()
                } else {
                    format!("{e}")
                });
            }
        }
        self.is_root =
            self.purpose == Purpose::Root && omsi_cfg::missing_original_essentials(&dir).is_empty();
        self.dir = dir;
    }

    pub(crate) fn title(&self) -> &'static str {
        match self.purpose {
            Purpose::Root => "Choose the OMSI 2 folder",
        }
    }
}
