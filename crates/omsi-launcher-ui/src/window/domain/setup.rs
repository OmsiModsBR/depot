//! What the Setup page keeps between frames.

use crate::kit::Status;

/// The Setup page's edits and what it shows.
#[derive(Default)]
pub struct SetupView {
    /// The folders as typed or browsed, until Save or Revert (none: as saved).
    pub root: Option<String>,
    pub game: Option<String>,
    pub content: Option<String>,
    /// The Content card's tab: 0 what was found, 1 the disk usage.
    pub tab: usize,
    /// The part of the disk usage chart looked into (the indices of its parts from the top).
    pub focus: Vec<usize>,
    pub(crate) checks: [(String, Option<(Status, String)>); 3],
    /// Where the content folder is now (the field's hint while none is chosen).
    pub(crate) content_now: Option<String>,
    /// Browse was pressed for the OMSI 2 folder where the game's own storage browser picks it
    /// (a phone, or the game in the launcher's process): where it starts. The window opens
    /// the browser and takes this back.
    pub browse_root: Option<String>,
    /// Save chose another OMSI 2 installation: the window draws its vehicles' pictures again
    /// once its content is read, and takes this back.
    pub other_root: bool,
}
