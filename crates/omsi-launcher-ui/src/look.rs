//! The colours and measures of the v5 launcher concept, in design units (the concept is
//! 1672x941; the window scales it uniformly).

use omsi_ui::Color;

pub const DESIGN_W: f32 = 1672.0;
pub const DESIGN_H: f32 = 941.0;
/// The window is never drawn smaller than this fraction of the design.
pub const MIN_SCALE: f32 = 0.75;

pub const BG: Color = Color::hex(0x101215);
pub const SIDE: Color = Color::hex(0x0c0d10);
pub const LINE: Color = Color::hex(0x2b2f37);
pub const FG: Color = Color::hex(0xe9eaec);
pub const MUTED: Color = Color::hex(0x9aa0aa);
pub const ACCENT: Color = Color::hex(0xf0a30a);
pub const NAV_TEXT: Color = Color::hex(0xc9ccd2);
pub const ON_ACCENT: Color = Color::hex(0x14120a);

pub const SIDEBAR_W: f32 = 264.0;
pub const NAV_ROW: f32 = 56.0;
/// Where the sidebar's list starts (under the logo).
pub const NAV_TOP: f32 = 119.0;
pub const FOOT_ROW: f32 = 48.0;
/// The concept's face is wider than ours at the same size: text is drawn this much larger.
pub const TEXT_SCALE: f32 = 1.16;

/// The scale from design units to pixels for a window of `w` x `h` pixels.
pub fn scale_for(w: u32, h: u32) -> f32 {
    (w as f32 / DESIGN_W)
        .min(h as f32 / DESIGN_H)
        .max(MIN_SCALE)
}
