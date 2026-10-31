//! Whether the OMSI 2 folder chosen has what the game needs.

/// The answer for one folder, kept until the folder changes or the window gets the focus back
/// (a folder repaired outside the launcher): the status bar asks for it every frame.
#[derive(Default)]
pub struct RootCheck(Option<(String, bool)>);

impl RootCheck {
    /// Whether `root` has what the game needs.
    pub fn ok(&mut self, root: &str) -> bool {
        match &self.0 {
            Some((r, ok)) if r == root => *ok,
            _ => {
                let ok =
                    omsi_cfg::missing_original_essentials(std::path::Path::new(root)).is_empty();
                self.0 = Some((root.to_string(), ok));
                ok
            }
        }
    }

    /// Look at the folder again on the next question.
    pub fn forget(&mut self) {
        self.0 = None;
    }
}
