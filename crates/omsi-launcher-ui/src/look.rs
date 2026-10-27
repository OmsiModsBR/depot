//! The colours (`omsi_ui::theme`'s, named as the concept names them) and measures of the v5
//! launcher concept, in design units (the concept is
//! 1672x941; the window scales it uniformly).

/// The accent (the player's choice, see `omsi_ui::theme::Accent`) and the text on it.
pub use omsi_ui::theme::{accent, on_accent};
use omsi_ui::Color;

pub const DESIGN_W: f32 = 1672.0;
pub const DESIGN_H: f32 = 941.0;
/// The window is never drawn smaller than this fraction of the design.
pub const MIN_SCALE: f32 = 0.75;

pub const BG: Color = omsi_ui::theme::BG;
pub const SIDE: Color = omsi_ui::theme::RAIL;
pub const PANEL: Color = omsi_ui::theme::PANEL;
pub const PANEL_2: Color = omsi_ui::theme::FIELD;
/// Around a picture that does not fill its card (the map's): darker than the card.
pub const PICTURE_BG: Color = Color::hex(0x18191c);
pub const LINE: Color = omsi_ui::theme::EDGE;
pub const FG: Color = omsi_ui::theme::TEXT;
pub const MUTED: Color = omsi_ui::theme::TEXT_DIM;
pub const NAV_TEXT: Color = omsi_ui::theme::TEXT_SOFT;
/// The chosen sidebar entry and one under the mouse: a layer lighter than the sidebar (solid:
/// translucent white blends in linear light and came out twice as light).
pub const SELECTED_LAYER: Color = Color::hex(0x313336);
pub const HOVER_LAYER: Color = Color::hex(0x2a2c2f);
pub use omsi_ui::theme::CONTROL_RADIUS;
pub const OK: Color = omsi_ui::theme::OK;
pub const WARN: Color = omsi_ui::theme::WARN;

pub const SIDEBAR_W: f32 = 248.0;
/// The sidebar with only its icons (a narrow window).
pub const SIDEBAR_COMPACT_W: f32 = 56.0;
/// A sidebar entry's height and the space under it (Windows 11's navigation view).
pub const NAV_ROW: f32 = 42.0;
pub const NAV_ITEM_H: f32 = 40.0;
/// Where the sidebar's list starts (under the logo).
pub const NAV_TOP: f32 = 78.0;
pub const FOOT_ROW: f32 = 42.0;
/// The driver's line at the sidebar's foot.
pub const WHO_H: f32 = 42.0;
/// The bar along the top that lists the pages instead of the sidebar (Settings > Interface >
/// Navigation: Top bar).
pub const TOPBAR_H: f32 = 56.0;
/// An entry of the top bar that shows only its icon.
pub const TOPBAR_ICON_W: f32 = 44.0;
/// The window's own caption buttons (minimise, maximise, close), top right.
pub const CAPTION_W: f32 = 46.0;
pub const CAPTION_H: f32 = 32.0;
/// The concept's face is wider than ours at the same size: text is drawn this much larger.
pub const TEXT_SCALE: f32 = 1.16;

/// The scale from design units to pixels for a window of `w` x `h` pixels.
pub fn scale_for(w: u32, h: u32) -> f32 {
    (w as f32 / DESIGN_W)
        .min(h as f32 / DESIGN_H)
        .max(MIN_SCALE)
}
