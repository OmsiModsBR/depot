//! A package written for one test into a folder of its own.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// A folder in the system's temporary folder, with a name no other test (of this run or
/// another running at the same time) has, that goes away with the value, also when the
/// test fails.
pub struct ScratchPack {
    dir: PathBuf,
}

impl ScratchPack {
    /// A new empty folder whose name starts with `name`.
    pub fn new(name: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "omsi-{name}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Self { dir }
    }

    /// The folder.
    pub fn path(&self) -> &Path {
        &self.dir
    }

    /// A path in the folder.
    pub fn join(&self, rel: impl AsRef<Path>) -> PathBuf {
        self.dir.join(rel)
    }

    /// Writes `contents` to `rel` in the folder, with the folders on the way, and gives
    /// the file's path.
    pub fn write(&self, rel: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> PathBuf {
        let path = self.dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, contents).unwrap();
        path
    }
}

impl Drop for ScratchPack {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
