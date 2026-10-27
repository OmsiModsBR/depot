//! Design tokens of the interface: the v5 concept's - flat and dark (cool neutral greys),
//! one accent colour used sparingly. Shared by the launcher and the in-game menus, so a page
//! of the launcher and a window in the game look the same.
//!
//! The accent is the player's choice from a short palette (`Accent`, Settings > Interface),
//! amber by default: `accent()` is read every time something is drawn, so every menu follows
//! a change at once.

use crate::paint::Color;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

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
            Accent::Red => 0xf06a62,
            Accent::Pink => 0xea6aa6,
            Accent::Purple => 0xa888f5,
            Accent::Blue => 0x4f9cf7,
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
pub const DANGER: Color = Color::hex(0xee6a5f);
pub const OK: Color = Color::hex(0x5fcf8a);
pub const WARN: Color = Color::hex(0xe8ab3a);

// The launcher's surfaces and text: Windows 11 (Fluent) greys, dark, with the cards a layer
// lighter than the window and hairline strokes round them.
pub const TEXT: Color = Color::hex(0xf3f4f6);
pub const TEXT_SOFT: Color = Color::hex(0xd9dce2);
pub const TEXT_DIM: Color = Color::hex(0xa3a8b2);
pub const TEXT_FAINT: Color = Color::hex(0x767c87);

/// The window behind the pages.
pub const BG: Color = Color::hex(0x1b1c1f);
/// Rail (the sidebar), panels, fields.
pub const RAIL: Color = Color::hex(0x202225);
pub const PANEL: Color = Color::hex(0x242528);
pub const FIELD: Color = Color::hex(0x2a2b2e);
pub const HOVER: Color = Color::hex(0x303135);
pub const SELECTED: Color = Color::hex(0x36383c);
/// Panel borders and dividers.
pub const EDGE: Color = Color::hex(0x333438);
/// The lower edge of a text field (Fluent marks fields with a lighter line under them).
pub const FIELD_LINE: Color = Color::hex(0x5c5e63);

/// A card's corners.
pub const RADIUS: f32 = 8.0;
/// A control's corners (buttons, fields, list rows).
pub const CONTROL_RADIUS: f32 = 4.0;
/// Height of a control row.
pub const ROW: f32 = 36.0;
pub const GAP: f32 = 12.0;

/// Square corners (Settings > Interface > Corners: flatter, closer to Content Manager): every
/// rounded box is drawn with corners of at most one point; circles stay round.
static SQUARE: AtomicBool = AtomicBool::new(false);

/// Draw square corners from now on (or rounded again).
pub fn set_square(on: bool) {
    SQUARE.store(on, Ordering::Relaxed);
}

pub fn square() -> bool {
    SQUARE.load(Ordering::Relaxed)
}

/// `radius` as the corners are drawn now (see `set_square`).
pub fn corner(radius: f32) -> f32 {
    if square() {
        radius.min(1.0)
    } else {
        radius
    }
}

/// Motion (Windows' "Animation effects"): off, everything that would move or fade shows at
/// once where it ends.
static MOTION: AtomicBool = AtomicBool::new(true);

/// Animate from now on (or show every change at once).
pub fn set_motion(on: bool) {
    MOTION.store(on, Ordering::Relaxed);
}

pub fn motion() -> bool {
    MOTION.load(Ordering::Relaxed)
}

/// Windows 11's durations of motion (seconds): a small change (a press, a tooltip), a
/// control's or a page's entrance, and a longer travel (the navigation's indicator).
pub const MOTION_FAST: f32 = 0.083;
pub const MOTION_NORMAL: f32 = 0.167;
pub const MOTION_SLOW: f32 = 0.25;

/// Windows 11's easing for things that arrive: fast at first, slowing to a stop (`t` from 0
/// to 1).
pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Glass (Settings > Interface > Background: Scene): the launcher's panels let the blurred
/// picture behind its pages show through.
static GLASS: AtomicBool = AtomicBool::new(false);

/// Draw panels as glass from now on (or solid again).
pub fn set_glass(on: bool) {
    GLASS.store(on, Ordering::Relaxed);
}

pub fn glass() -> bool {
    GLASS.load(Ordering::Relaxed)
}

/// How much of a panel's own colour covers the scene behind it, as glass.
pub const GLASS_COVER: f32 = 0.6;

/// A panel's fill `c` as it is drawn now: solid, or seen through as glass (see `set_glass`).
pub fn surface(c: Color) -> Color {
    if glass() {
        c.alpha(GLASS_COVER)
    } else {
        c
    }
}

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
            for surface in [BG, RAIL, PANEL, FIELD, v5::CARD, v5::SIDE] {
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
