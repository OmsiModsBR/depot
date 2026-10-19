//! Design tokens of the interface: dark blue-greys, one amber accent used sparingly - the
//! v5 look of the launcher concept. Shared by the launcher's pages and the kit's widgets;
//! the in-game menus (`pause_menu`, `alt_bar`) draw with the same values.

use crate::paint::Color;

pub const ACCENT: Color = Color::hex(0xf0a30a);
pub const ACCENT_2: Color = Color::rgba(96, 160, 232, 1.0);
pub const DANGER: Color = Color::hex(0xe8523f);
pub const OK: Color = Color::hex(0x4fc27a);
pub const WARN: Color = Color::hex(0xe0a030);

pub const TEXT: Color = Color::hex(0xe9eaec);
pub const TEXT_SOFT: Color = Color::hex(0xc9ccd2);
pub const TEXT_DIM: Color = Color::hex(0x9aa0aa);
pub const TEXT_FAINT: Color = Color::hex(0x646a74);

/// Rail, panels, fields.
pub const RAIL: Color = Color::hex(0x0c0d10);
pub const PANEL: Color = Color::hex(0x171a1f);
pub const FIELD: Color = Color::hex(0x1e2229);
pub const HOVER: Color = Color::hex(0x262a32);
pub const SELECTED: Color = Color::hex(0x2c313a);
pub const EDGE: Color = Color::rgba(255, 255, 255, 0.07);

pub const RADIUS: f32 = 8.0;
/// Height of a control row.
pub const ROW: f32 = 36.0;
pub const GAP: f32 = 12.0;

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
