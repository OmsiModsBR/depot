//! The graphics mode's name in a settings file, for what the game and the launcher must
//! read alike: the names of the removed Enhanced+ mode.

/// Whether `text` names Enhanced+, the graphics mode that was removed: a settings file (or
/// `OMSI_GRAPHICS`) that still says so is read as Enhanced.
pub fn names_enhanced_plus(text: &str) -> bool {
    matches!(
        text.trim()
            .to_ascii_lowercase()
            .replace(['-', ' '], "_")
            .as_str(),
        "enhanced_plus" | "enhanced+" | "enhancedplus" | "enhanced_+" | "2"
    )
}

/// Say in the log, once per run, that the Enhanced+ a settings file asks for is read as
/// Enhanced.
pub fn note_enhanced_plus_read_as_enhanced() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        log::info!("settings: Enhanced+ graphics were removed; the setting is read as Enhanced")
    });
}
