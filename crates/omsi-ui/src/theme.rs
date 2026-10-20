//! Design tokens of the interface: the v5 concept's - flat and dark (cool neutral greys),
//! one amber accent used sparingly. Shared by the launcher and the in-game menus, so a page
//! of the launcher and a window in the game look the same.

use crate::paint::Color;

pub const ACCENT: Color = Color::hex(0xf0a30a);
pub const ACCENT_2: Color = Color::rgba(96, 160, 232, 1.0);
pub const DANGER: Color = Color::rgba(222, 78, 68, 1.0);
pub const OK: Color = Color::hex(0x4fc27a);
pub const WARN: Color = Color::hex(0xe0a030);
/// Text on the accent (the primary button's label).
pub const ON_ACCENT: Color = Color::hex(0x14120a);

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
    pub const ACCENT: Color = Color::hex(0xf0a30a);
    /// Text on the accent.
    pub const ON_ACCENT: Color = Color::hex(0x14120a);
    /// A row or a button chosen: the accent, faint, on the dark.
    pub const SELECTED: Color = Color::hex(0x2a2316);
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
            assert!(contrast(ACCENT, surface) >= 4.5, "accent on {surface:?}");
        }
    }
}
