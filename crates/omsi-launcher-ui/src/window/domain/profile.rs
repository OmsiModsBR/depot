//! What the Profile page keeps between frames.

/// The Profile page's choices: the new driver's name, deleting the driver, the drive chosen
/// and its details.
#[derive(Default)]
pub struct ProfileView {
    pub new_driver: String,
    pub confirm_delete: Option<std::time::Instant>,
    /// The drive chosen on Profile (the driver's drives, most recent first).
    pub drive: usize,
    /// Its details window is open.
    pub drive_details: bool,
}
