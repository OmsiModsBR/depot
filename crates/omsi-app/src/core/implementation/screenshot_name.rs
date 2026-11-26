//! The on-screen notice of a screenshot: the message raised once the picture is saved (or
//! fails to save), so the path never ends up inside the picture itself.

/// The on-screen notice for a saved screenshot. It is raised only after the capture, so the
/// path never ends up inside the picture itself.
pub fn notice(path: &std::path::Path) -> (String, f32) {
    (format!("Screenshot: {}", path.display()), 4.0)
}

/// Whether an on-screen message is a screenshot notice (saved, or failed to save).
pub fn is_notice(msg: &str) -> bool {
    msg.starts_with("Screenshot: ") || msg.starts_with("Screenshot could not be saved")
}
