//! Design tokens of the interface: the v5 concept's - flat and dark (cool neutral greys),
//! one accent colour used sparingly. Shared by the launcher and the in-game menus, so a page
//! of the launcher and a window in the game look the same.
//!
//! The accent is the player's choice from a short palette (`Accent`, Settings > Interface),
//! amber by default: `accent()` is read every time something is drawn, so every menu follows
//! a change at once.

use crate::paint::Color;
use std::sync::atomic::{AtomicU32, Ordering};

/// The accent colours the player can choose from: a short fixed palette, each readable on
/// the dark surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Accent {
    #[default]
    Amber,
    Orange,
    Red,
    Pink,
    Purple,
    Blue,
    Teal,
    Green,
}

impl Accent {
    /// Every colour, in the order Settings shows them.
    pub const ALL: [Accent; 8] = [
        Accent::Amber,
        Accent::Orange,
        Accent::Red,
        Accent::Pink,
        Accent::Purple,
        Accent::Blue,
        Accent::Teal,
        Accent::Green,
    ];

    /// The name in `settings.cfg` (`ui_accent=`) and on screen.
    pub fn name(self) -> &'static str {
        match self {
            Accent::Amber => "Amber",
            Accent::Orange => "Orange",
            Accent::Red => "Red",
            Accent::Pink => "Pink",
            Accent::Purple => "Purple",
            Accent::Blue => "Blue",
            Accent::Teal => "Teal",
            Accent::Green => "Green",
        }
    }

    /// The colour called `name` (any case); `None` for anything else.
    pub fn named(name: &str) -> Option<Accent> {
        let name = name.trim();
        Accent::ALL
            .into_iter()
            .find(|a| a.name().eq_ignore_ascii_case(name))
    }

    pub fn color(self) -> Color {
        Color::hex(self.rgb())
    }

    fn rgb(self) -> u32 {
        match self {
            Accent::Amber => 0xf0a30a,
            Accent::Orange => 0xff7a29,
            Accent::Red => 0xef5a55,
            Accent::Pink => 0xe65c9c,
            Accent::Purple => 0xa07af2,
            Accent::Blue => 0x3b8cf5,
            Accent::Teal => 0x22b5b0,
            Accent::Green => 0x4cc16a,
        }
    }
}

/// The accent in use, as `0xRRGGBB`.
static ACCENT_RGB: AtomicU32 = AtomicU32::new(0xf0a30a);

/// Use `a` as the accent from the next thing drawn on.
pub fn set_accent(a: Accent) {
    ACCENT_RGB.store(a.rgb(), Ordering::Relaxed);
}

/// The accent in use (`Accent::Amber` until `set_accent`).
pub fn current_accent() -> Accent {
    let rgb = ACCENT_RGB.load(Ordering::Relaxed);
    Accent::ALL
        .into_iter()
        .find(|a| a.rgb() == rgb)
        .unwrap_or_default()
}

/// The accent colour: lit entries, the primary button, links, a switch that is on.
pub fn accent() -> Color {
    Color::hex(ACCENT_RGB.load(Ordering::Relaxed))
}

/// Text and icons on the accent (the primary button's label): dark or white, whichever
/// reads better on the accent in use.
pub fn on_accent() -> Color {
    on(accent())
}

/// Dark or white, whichever has more contrast on `c`.
pub fn on(c: Color) -> Color {
    const DARK: Color = Color::hex(0x14120a);
    if contrast(DARK, c) >= contrast(Color::WHITE, c) {
        DARK
    } else {
        Color::WHITE
    }
}

/// A chosen row or button: the accent, faint, on the dark.
pub fn accent_soft() -> Color {
    v5::CARD.mix(accent(), 0.11)
}
pub const ACCENT_2: Color = Color::rgba(96, 160, 232, 1.0);
pub const DANGER: Color = Color::rgba(222, 78, 68, 1.0);
pub const OK: Color = Color::hex(0x4fc27a);
pub const WARN: Color = Color::hex(0xe0a030);

pub const TEXT: Color = Color::hex(0xe9eaec);
pub const TEXT_SOFT: Color = Color::hex(0xc9ccd2);
pub const TEXT_DIM: Color = Color::hex(0x9aa0aa);
pub const TEXT_FAINT: Color = Color::hex(0x6b717b);

/// The window behind the pages.
pub const BG: Color = Color::hex(0x101215);
/// Rail (the sidebar), panels, fields.
pub const RAIL: Color = Color::hex(0x0c0d10);
pub const PANEL: Color = Color::hex(0x171a1f);
pub const FIELD: Color = Color::hex(0x1e2229);
pub const HOVER: Color = Color::hex(0x252a32);
pub const SELECTED: Color = Color::hex(0x2b313a);
/// Panel borders and dividers.
pub const EDGE: Color = Color::hex(0x2b2f37);

pub const RADIUS: f32 = 8.0;
/// Height of a control row.
pub const ROW: f32 = 36.0;
pub const GAP: f32 = 12.0;

/// The in-game look of the new interface (the Esc menu and its pages, the Alt bar, the city
/// map): darker than the launcher's, with a cooler grey and a stronger amber. Shared so the
/// screens drawn by different code keep the same colours.
pub mod v5 {
    use crate::paint::Color;

    /// The pause menu's column.
    pub const SIDE: Color = Color::hex(0x0c0d10);
    /// Bars over the top of a screen.
    pub const BAR: Color = Color::hex(0x0e1013);
    /// Cards on a page.
    pub const CARD: Color = Color::hex(0x15171b);
    /// Panels laid over the picture (the Alt bar's).
    pub const PANEL: Color = Color::hex(0x171a1f);
    /// A button's square in a panel.
    pub const CELL: Color = Color::hex(0x1b1d22);
    /// Fields and buttons in a card.
    pub const FIELD: Color = Color::hex(0x1d2025);
    /// Hairlines round cards and under bars.
    pub const LINE: Color = Color::hex(0x2b2f37);
    /// The fainter lines between a card's rows.
    pub const ROW_LINE: Color = Color::hex(0x20242b);
    pub const FG: Color = Color::hex(0xe9eaec);
    pub const MUTED: Color = Color::hex(0x9aa0aa);
    pub const DANGER: Color = Color::hex(0xe8523f);
    pub const OK: Color = Color::hex(0x4fc27a);
    /// A switch that is off, and its knob.
    pub const TRACK_OFF: Color = Color::hex(0x3a3e46);
    pub const KNOB: Color = Color::hex(0xf2f2f2);
    /// The text is drawn this much larger than its nominal size (the concept's face is wider).
    pub const TS: f32 = 1.16;
}

/// Relative luminance of an opaque colour (WCAG 2.x).
pub fn luminance(c: Color) -> f32 {
    let lin = |v: f32| {
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c.0[0]) + 0.7152 * lin(c.0[1]) + 0.0722 * lin(c.0[2])
}

/// Contrast ratio between two opaque colours (1 to 21).
pub fn contrast(a: Color, b: Color) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readable_text_keeps_its_contrast_on_every_surface() {
        for surface in [RAIL, PANEL, FIELD, HOVER, SELECTED] {
            for text in [TEXT, TEXT_SOFT] {
                assert!(contrast(text, surface) >= 7.0, "{text:?} on {surface:?}");
            }
            assert!(
                contrast(TEXT_DIM, surface) >= 4.5,
                "dim text on {surface:?}"
            );
        }
    }

    #[test]
    fn every_accent_reads_on_the_surfaces_and_under_its_label() {
        for a in Accent::ALL {
            let c = a.color();
            for surface in [RAIL, PANEL, FIELD, v5::CARD, v5::SIDE] {
                assert!(contrast(c, surface) >= 4.5, "{a:?} on {surface:?}");
            }
            assert!(contrast(on(c), c) >= 4.5, "label on {a:?}");
            assert!(
                contrast(TEXT, v5::CARD.mix(c, 0.11)) >= 7.0,
                "text on {a:?}'s chosen row"
            );
        }
    }

    #[test]
    fn the_accent_is_chosen_by_name_and_amber_by_default() {
        assert_eq!(Accent::default(), Accent::Amber);
        assert_eq!(Accent::named(" blue "), Some(Accent::Blue));
        assert_eq!(Accent::named("TEAL"), Some(Accent::Teal));
        assert_eq!(Accent::named("magenta"), None);
        for a in Accent::ALL {
            assert_eq!(Accent::named(a.name()), Some(a));
        }
    }
}
