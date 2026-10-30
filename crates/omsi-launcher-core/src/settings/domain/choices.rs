//! The values an option can take, with the name each shows under.

/// The window sizes the settings offer (`resolution`).
pub const RESOLUTIONS: &[(&str, &str)] = &[
    ("auto", "Automatic"),
    ("1280x720", "1280 x 720"),
    ("1280x800", "1280 x 800 (Steam Deck)"),
    ("1366x768", "1366 x 768"),
    ("1600x900", "1600 x 900"),
    ("1920x1080", "1920 x 1080"),
    ("1920x1200", "1920 x 1200"),
    ("2560x1440", "2560 x 1440"),
    ("3840x2160", "3840 x 2160"),
];
