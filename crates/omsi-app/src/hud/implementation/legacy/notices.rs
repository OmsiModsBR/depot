//! The chat and the notices drawn over the picture: the notes in the corner, the other
//! players' name tags, the mouse-over name and the headset pointer.

use super::super::super::domain::Frame;
use super::*;
use crate::hud::interface::output::{Renderer, Scene};

// ---------------------------------------------------------------------------------------
// the chat

/// How many rows the chat shows (a long line takes several), and how many lines it keeps to
/// scroll back.
const CHAT_SHOWN: usize = 8;

/// The chat's own state: shown or hidden (V), the scroll position and whether the cursor is
/// over it.
#[derive(Default)]
pub struct ChatWidget {
    pub hidden: bool,
    /// Lines scrolled back from the newest.
    pub scroll: usize,
    /// The chat's box on the screen (physical pixels) as drawn last: hovering over it shows
    /// the input box, a click there opens it.
    pub rect: [f32; 4],
    pub hovered: bool,
    pub caret_t: f32,
}

impl ChatWidget {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.rect[0] && x <= self.rect[2] && y >= self.rect[1] && y <= self.rect[3]
    }
}

impl Ui {
    /// The name tags above the other players' buses.
    pub(super) fn draw_tags(&mut self, r: &Renderer, scene: &mut Scene, f: &Frame, s: f32) {
        for ((x, y), name, sub, alpha) in &f.tags {
            let a = (alpha.clamp(0.0, 1.0) * 255.0) as u8;
            let l = self
                .text
                .label(r, scene, name, (19.0 * s) as u32, [255, 255, 255, 220]);
            let x0 = x - l.w as f32 * 0.5;
            let y0 = y - l.h as f32;
            if a > 0 {
                scene
                    .overlays
                    .push((l.tex, [x0, y0, x0 + l.w as f32, y0 + l.h as f32]));
                if !sub.is_empty() {
                    let m = self
                        .text
                        .label(r, scene, sub, (12.0 * s) as u32, [210, 225, 255, 200]);
                    let mx = x - m.w as f32 * 0.5;
                    scene.overlays.push((
                        m.tex,
                        [
                            mx,
                            y0 + l.h as f32 - 3.0 * s,
                            mx + m.w as f32,
                            y0 + l.h as f32 - 3.0 * s + m.h as f32,
                        ],
                    ));
                }
            }
        }
    }

    /// The notes, top left: white on a dark outline, one line each, on a dim plate (the
    /// outline alone left a note hard to read over a bright sky). Returns where they end.
    pub(super) fn draw_notes(
        &mut self,
        r: &Renderer,
        scene: &mut Scene,
        f: &Frame,
        s: f32,
        corner_top: f32,
    ) -> f32 {
        let px = (16.0 * s) as u32;
        let x0 = 16.0 * s;
        // (below the on-screen buttons of a phone, which keep their size when the
        // interface is made smaller)
        let mut y = if crate::core::platform::touch_controls() {
            (80.0 * f.scale.max(0.5) * f.ui_scale.max(1.0)).max(corner_top)
        } else {
            corner_top
        };
        for n in f.notes.iter().filter(|n| !n.trim().is_empty()).take(8) {
            let text = omsi_ui::tr(n);
            let text = clip_to(&self.text, &text, px as f32, f.width * 0.6);
            let l = self.text.label(r, scene, &text, px, [255, 255, 255, 235]);
            let plate = self.text.plate(r, scene, 7);
            scene.overlays.push((
                plate,
                [x0 - 5.0 * s, y, x0 + l.w as f32 + 5.0 * s, y + l.h as f32],
            ));
            scene
                .overlays
                .push((l.tex, [x0, y, x0 + l.w as f32, y + l.h as f32]));
            y += l.h as f32 + 2.0 * s;
        }
        y
    }

    /// The chat, top left under the notes, as Roblox has it (from the fourth note on they ran
    /// into it).
    pub(super) fn draw_chat(
        &mut self,
        r: &Renderer,
        scene: &mut Scene,
        f: &Frame,
        s: f32,
        dt: f32,
        notes_bottom: f32,
    ) {
        if let Some(c) = f.chat.as_ref().filter(|_| !self.chat.hidden) {
            // (its own size on top of the interface's: Ctrl + the wheel over it)
            let s = s * f.chat_size.clamp(0.5, 3.0);
            let px = (17.0 * s) as u32;
            let lh = px as f32 * 1.35;
            let x0 = 14.0 * s;
            let y0 = (96.0 * s).max(notes_bottom + 10.0 * s);
            let width = (460.0 * s).min(f.width * 0.6);
            let open = c.typing.is_some();
            let n = c.lines.len();
            let end = n.saturating_sub(if open || self.chat.hovered {
                self.chat.scroll
            } else {
                0
            });
            let box_h = lh * CHAT_SHOWN as f32 + lh * 1.6;
            self.chat.rect = [x0 - 6.0 * s, y0 - 6.0 * s, x0 + width, y0 + box_h];
            self.chat.hovered = self.chat.contains(f.cursor.0, f.cursor.1);
            let show_box = open || self.chat.hovered;
            // the lines, newest at the bottom of the history area; a line too long for the box
            // goes on in rows under it, indented (the oldest line shown may lose its first rows)
            let indent = 14.0 * s;
            let mut rows: Vec<(f32, String, [u8; 4])> = Vec::new();
            {
                let tc = &self.text;
                let measure = |t: &str| tc.width_raw(t, px as f32);
                let mut i = end;
                while i > 0 && rows.len() < CHAT_SHOWN {
                    i -= 1;
                    let line = &c.lines[i];
                    let color = if line.starts_with("* ") {
                        [255, 226, 140, 230]
                    } else {
                        [255, 255, 255, 230]
                    };
                    for (k, t) in wrap_rows(line, width, indent, &measure)
                        .into_iter()
                        .enumerate()
                        .rev()
                    {
                        if rows.len() == CHAT_SHOWN {
                            break;
                        }
                        rows.push((if k == 0 { 0.0 } else { indent }, t, color));
                    }
                }
                rows.reverse();
            }
            let mut y = y0 + lh * (CHAT_SHOWN - rows.len()) as f32;
            for (dx, text, color) in rows {
                let l = self.text.label(r, scene, &text, px, color);
                scene
                    .overlays
                    .push((l.tex, [x0 + dx, y, x0 + dx + l.w as f32, y + l.h as f32]));
                y += lh;
            }
            if show_box {
                // the input box: shown when the cursor is over the chat or it is typed into
                let by = y0 + lh * CHAT_SHOWN as f32 + lh * 0.2;
                let bh = lh * 1.25;
                let plate = self.text.plate(r, scene, 0);
                scene
                    .overlays
                    .push((plate, [x0 - 4.0 * s, by, x0 + width, by + bh]));
                self.chat.caret_t += dt;
                let caret = if open && (self.chat.caret_t % 1.0) < 0.55 {
                    "|"
                } else {
                    ""
                };
                let (text, color) = match c.typing {
                    Some(t) => (format!("{t}{caret}"), [255, 255, 255, 240]),
                    None => (
                        "Click here or press / to chat".to_string(),
                        [190, 190, 190, 200],
                    ),
                };
                let text = clip_left(&self.text, &text, px as f32, width - 10.0 * s);
                let l = self.text.label(r, scene, &text, px, color);
                let ty = by + (bh - l.h as f32) * 0.5;
                scene.overlays.push((
                    l.tex,
                    [x0 + 2.0 * s, ty, x0 + 2.0 * s + l.w as f32, ty + l.h as f32],
                ));
                if self.chat.scroll > 0 && (open || self.chat.hovered) {
                    // (the words translated apart from the number: whole, the text was no key)
                    let m = self.text.label(
                        r,
                        scene,
                        &format!("{} {}", self.chat.scroll, omsi_ui::tr("newer below")),
                        (11.0 * s) as u32,
                        [200, 200, 200, 200],
                    );
                    scene.overlays.push((
                        m.tex,
                        [x0 + width - m.w as f32, by - m.h as f32, x0 + width, by],
                    ));
                }
            }
            if let Some(e) = c.error {
                let l = self.text.label(
                    r,
                    scene,
                    &format!("{}: {}", omsi_ui::tr("Not sent"), omsi_ui::tr(e)),
                    (12.0 * s) as u32,
                    [255, 150, 150, 220],
                );
                let ey = y0 + box_h;
                scene
                    .overlays
                    .push((l.tex, [x0, ey, x0 + l.w as f32, ey + l.h as f32]));
            }
        } else {
            self.chat.hovered = false;
            self.chat.rect = [0.0; 4];
        }
    }

    /// The mouse-over name, right of the cursor.
    pub(super) fn draw_tooltip(&mut self, r: &Renderer, scene: &mut Scene, f: &Frame, s: f32) {
        self.vr_tooltip_overlay = None;
        if let Some(t) = f.tooltip.as_ref().filter(|t| !t.is_empty()) {
            let l = self
                .text
                .label(r, scene, t, (14.0 * s) as u32, [255, 255, 255, 235]);
            let mut x = f.cursor.0.clamp(0.0, f.width) + 16.0 * s;
            let mut y = f.cursor.1 + 2.0 * s;
            if x + l.w as f32 > f.width {
                x = (f.cursor.0.clamp(0.0, f.width) - 8.0 * s - l.w as f32).max(5.0 * s);
            }
            if y + l.h as f32 > f.height {
                y = f.height - l.h as f32;
            }
            if f.vr {
                self.vr_tooltip_overlay = Some(scene.overlays.len());
            } else {
                // (in a headset the text alone is placed in front of the eye)
                let plate = self.text.plate(r, scene, 7);
                scene.overlays.push((
                    plate,
                    [x - 5.0 * s, y, x + l.w as f32 + 5.0 * s, y + l.h as f32],
                ));
            }
            scene
                .overlays
                .push((l.tex, [x, y, x + l.w as f32, y + l.h as f32]));
        }
    }

    /// The pointer of a headset.
    pub(super) fn draw_pointer(&mut self, r: &Renderer, scene: &mut Scene, f: &Frame, s: f32) {
        if f.vr {
            let pointer = self.text.vr_pointer(r, scene);
            self.vr_cursor_overlay = Some(scene.overlays.len());
            scene.overlays.push((pointer, [0.0, 0.0, 7.0 * s, 7.0 * s]));
        } else {
            self.vr_cursor_overlay = None;
        }
    }
}
