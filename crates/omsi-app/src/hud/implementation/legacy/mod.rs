//! The game's own interface, drawn over the picture in Roboto with a dark outline (the
//! HUD's `.oft` fonts are OMSI's and stay for the time and speed): the chat of a LAN
//! session, the name of what the cursor points at next to the cursor, and the other
//! players' name tags above their buses.
//!
//! Every text is rendered once into a small texture and kept while it is shown; the
//! overlays are rectangles in physical pixels (`Scene::overlays`).
//!
//! By part: `text` (the texts and their measuring), `paint` (the plates and shapes),
//! `notices` (the chat, the notes, the name tags, the mouse-over name and the headset
//! pointer), `info` (the frame rate, the information bar, the timetable and a tutorial page),
//! `style` (the menu's colours and sizes), `menu` (the game menu and its lists) and
//! `settings` (a settings window). `Ui::draw` here draws them in their order.

use super::super::domain::{Frame, Hits};
use crate::hud::interface::output::{Renderer, Scene, TextureId};

mod info;
mod menu;
mod notices;
pub(crate) mod oft;
mod paint;
pub(crate) mod settings;
mod style;
mod text;

pub use info::*;
pub use notices::*;
pub use oft::Hud;
pub use style::*;
pub use text::*;

/// How much larger than designed the interface is drawn, on top of the screen's scale
/// `dpi`: on a window taller than 1080 logical pixels as much as it is taller (up to twice),
/// times the player's `size` (`Settings::ui_scale`). Laid out for 1080p, the texts were half
/// their size on a 4K screen at 100 % display scaling; a window of 1080 lines or fewer is
/// drawn as it always was. With `window` off (`Settings::ui_scale_window`) it does not grow
/// with the window at all.
pub fn size_factor(height_px: f32, dpi: f32, size: f32, window: bool) -> f32 {
    let grown = if window {
        (height_px / dpi.max(0.5) / 1080.0).clamp(1.0, 2.0)
    } else {
        1.0
    };
    grown * size
}

/// How strongly the interface's backgrounds are drawn for the opacity setting
/// (`Settings::ui_opacity`): 1 at its default of 85 %, as designed; lower, the picture shows
/// through them (never less than 0.3), higher a little darker. The texts stay solid.
pub fn backdrop(opacity: f32) -> f32 {
    (opacity / 0.85).clamp(0.3, 1.3)
}

pub struct Ui {
    origin_x: f32,
    pub text: TextCache,
    pub chat: ChatWidget,
    /// Where the menu's parts and the information bar were drawn this frame, for the mouse.
    pub hits: Hits,
    /// The colours and positions of the menu's parts that ease to their new state (a line's
    /// light, a switch's knob ...), by what they belong to.
    anim: std::collections::HashMap<u64, f32>,
    /// Seconds since the last frame, for `ease`.
    anim_dt: f32,
    /// Overlay entries belonging to the game menu.
    pub menu_overlay_range: std::ops::Range<usize>,
    /// The pointer texture, positioned separately for each headset eye.
    pub vr_cursor_overlay: Option<usize>,
    pub vr_tooltip_overlay: Option<usize>,
    /// Pictures shown in the interface (a tutorial page's), by file.
    images: hashbrown::HashMap<std::path::PathBuf, Option<(TextureId, u32, u32)>>,
}

pub(crate) fn shift_overlays(scene: &mut Scene, start: usize, x: f32) {
    for (_, rect) in &mut scene.overlays[start..] {
        rect[0] += x;
        rect[2] += x;
    }
}

impl Ui {
    /// Lay out at panel resolution, then move both pixels and mouse targets to
    /// that panel's position in the spanning window.
    pub fn draw_at(&mut self, r: &Renderer, scene: &mut Scene, f: &Frame, dt: f32, origin_x: f32) {
        self.shift_hitboxes(-self.origin_x);
        let start = scene.overlays.len();
        self.draw(r, scene, f, dt);
        shift_overlays(scene, start, origin_x);
        self.shift_hitboxes(origin_x);
        self.origin_x = origin_x;
    }

    pub fn shift_hitboxes(&mut self, x: f32) {
        self.hits.shift(x);
        self.chat.rect[0] += x;
        self.chat.rect[2] += x;
    }
    pub fn new() -> Option<Ui> {
        Some(Ui {
            origin_x: 0.0,
            text: TextCache::new()?,
            chat: ChatWidget::default(),
            hits: Hits::default(),
            anim: Default::default(),
            anim_dt: 0.0,
            menu_overlay_range: 0..0,
            vr_cursor_overlay: None,
            vr_tooltip_overlay: None,
            images: Default::default(),
        })
    }

    /// Draw the frame's interface: its overlays go after the HUD's in `scene.overlays`.
    pub fn draw(&mut self, r: &Renderer, scene: &mut Scene, f: &Frame, dt: f32) {
        let s = f.scale.max(0.5) * f.ui_scale;
        self.text.backdrop = f.opacity;
        self.draw_tags(r, scene, f, s);
        // the top of the windows in the corners: the notes on the left start on the line of
        // the timetable and a tutorial page on the right (14 px down, they stood higher)
        let corner_top = 60.0 * s;
        let notes_bottom = self.draw_notes(r, scene, f, s, corner_top);
        self.draw_chat(r, scene, f, s, dt, notes_bottom);
        self.draw_fps(r, scene, f, s);
        self.draw_info(r, scene, f, s);
        let tutorial_w = (420.0 * s).min(f.width * 0.42);
        self.draw_timetable(r, scene, f, s, corner_top, tutorial_w);
        self.draw_tutorial(r, scene, f, s, corner_top, tutorial_w);
        // --- the game menu and its lists, in the middle over a dimmed picture
        self.anim_dt = dt.clamp(0.0, 0.1);
        self.text.flat = true;
        self.draw_menu(r, scene, f);
        self.text.flat = false;
        self.draw_tooltip(r, scene, f, s);
        self.draw_pointer(r, scene, f, s);
        self.text.end_frame(r, scene);
    }
}
