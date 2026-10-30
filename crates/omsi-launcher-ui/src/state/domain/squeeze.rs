//! The Free up space card's choices and the job running.

use super::super::interface::output::core;

/// How a compression or an undo ended.
#[derive(Debug, Clone)]
pub enum Squeezed {
    /// Ended; this many calls of the system tool failed (files in use or not allowed).
    Done(u64),
    Cancelled,
    Error(String),
}

/// The Free up space card: what was read of the folders, what is ticked, the step the
/// person is at and the job running.
#[derive(Default)]
pub struct Squeeze {
    pub folders: Option<std::sync::Arc<Vec<core::compress::Folder>>>,
    /// The ticks were set from the first reading (and are kept over the next ones).
    pub(crate) chosen_set: bool,
    pub asked: bool,
    pub(crate) gen: u64,
    /// Ticked: [the Depot's content, the original OMSI 2].
    pub chosen: [bool; 2],
    /// Asking to confirm: Some(false) a compression, Some(true) an undo.
    pub confirm: Option<bool>,
    /// "I understand it can be slower on a hard disk" is ticked.
    pub hdd_ok: bool,
    pub job: Option<(std::sync::Arc<core::compress::Job>, bool)>,
    pub done: Option<Squeezed>,
}

impl Squeeze {
    pub const DEPOT: &'static str = "Depot content folder";
    pub const ORIGINAL: &'static str = "OMSI 2 folder";

    /// The Depot's content folder and the original OMSI 2 folder as they are now. The
    /// content folder is left out when it is the original folder itself.
    pub fn dirs(
        root: &std::path::Path,
    ) -> (Option<std::path::PathBuf>, Option<std::path::PathBuf>) {
        let root = (!root.as_os_str().is_empty() && root.is_dir()).then(|| root.to_path_buf());
        let content = core::content_dir().filter(|c| {
            !root
                .as_deref()
                .is_some_and(|r| core::compress::same_path(r, c))
        });
        (content, root)
    }

    pub fn index(name: &str) -> usize {
        usize::from(name == Self::ORIGINAL)
    }
}
