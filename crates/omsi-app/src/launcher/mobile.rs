//! The launcher on a phone or a tablet: fingers instead of a mouse, a keyboard that comes
//! up on the screen, a narrow rail of icons, pages that scroll as a whole, and a browser of
//! the device's storage where the desktop opens Finder or Explorer (a phone has no file
//! dialog that gives a program a path).
//!
//! `OMSI_MOBILE=1` gives the desktop launcher the same layout (with `OMSI_LAUNCHER_SIZE`
//! the size of a phone), so that it can be looked at without a phone.

use super::Launcher;
use glam::Vec2;
use winit::event::{Touch, TouchPhase};

/// Width of the rail of icons on a phone.
pub const RAIL_W_MOBILE: f32 = 64.0;
/// The height the pages are laid out for on a phone (they scroll within the screen).
pub use omsi_launcher_ui::window::PAGE_H;
/// How far a finger moves (interface points) before a touch is a drag, not a tap.
const SLOP: f32 = 9.0;

pub use omsi_launcher_ui::state::mobile;
pub use omsi_launcher_ui::window::{Browser, Purpose};

/// The fingers on the launcher.
#[derive(Default)]
pub struct Fingers {
    /// The finger that works the interface: its id, where it went down, and whether it has
    /// become a drag (a scroll, or turning the bus).
    main: Option<(u64, Vec2, bool)>,
    /// Where each finger is (interface points), for the pinch over the bus.
    at: Vec<(u64, Vec2)>,
    pinch: Option<f32>,
    /// The finger came up last frame: the pointer leaves the screen this frame (no hover
    /// stays behind where it was).
    lift: bool,
}

impl Launcher {
    /// A finger on the screen, as the mouse the interface knows: a tap is a click, a drag
    /// scrolls (the list under it, else the page), a drag over the bus turns it and two
    /// fingers over it zoom.
    pub(super) fn touch(&mut self, t: Touch, scale: f32) {
        let p = Vec2::new(t.location.x as f32, t.location.y as f32) / scale;
        self.last_input = std::time::Instant::now();
        self.ui.input.touch = true;
        match t.phase {
            TouchPhase::Started => {
                self.fingers.at.retain(|(id, _)| *id != t.id);
                self.fingers.at.push((t.id, p));
                if self.fingers.at.len() == 2 {
                    // a second finger: a pinch over the bus, never a click
                    let (a, b) = (self.fingers.at[0].1, self.fingers.at[1].1);
                    self.fingers.pinch = Some(a.distance(b).max(1.0));
                    if let Some((_, _, drag)) = self.fingers.main.as_mut() {
                        *drag = true;
                    }
                    return;
                }
                if self.fingers.main.is_some() {
                    return;
                }
                self.fingers.main = Some((t.id, p, false));
                self.fingers.lift = false;
                self.ui.input.mouse = p;
                self.ui.input.pressed = true;
                self.ui.input.down = true;
                if self.preview_rect.map(|r| r.contains(p)).unwrap_or(false) {
                    self.dragging = Some(p);
                }
            }
            TouchPhase::Moved => {
                let prev = self
                    .fingers
                    .at
                    .iter()
                    .find(|(id, _)| *id == t.id)
                    .map(|f| f.1);
                if let Some(f) = self.fingers.at.iter_mut().find(|(id, _)| *id == t.id) {
                    f.1 = p;
                }
                if let (Some(d0), 2) = (self.fingers.pinch, self.fingers.at.len()) {
                    let d = self.fingers.at[0].1.distance(self.fingers.at[1].1).max(1.0);
                    self.preview.zoom_by((d0 / d).clamp(0.8, 1.25));
                    self.fingers.pinch = Some(d);
                    return;
                }
                let Some((id, start, drag)) = self.fingers.main else {
                    return;
                };
                if id != t.id {
                    return;
                }
                let delta = prev.map(|q| p - q).unwrap_or(Vec2::ZERO);
                if !drag && p.distance(start) > SLOP {
                    self.fingers.main = Some((id, start, true));
                }
                if let Some(last) = self.dragging {
                    let d = p - last;
                    self.preview.orbit(d.x, d.y);
                    self.dragging = Some(p);
                    self.ui.input.mouse = p;
                    return;
                }
                // a slider or a scroll bar held keeps the finger; anything else scrolls
                let held_slider = self.ui.active.is_some()
                    && self.ui.input.down
                    && !self.fingers.main.map(|m| m.2).unwrap_or(false);
                self.ui.input.mouse = p;
                if self.fingers.main.map(|m| m.2).unwrap_or(false) && !held_slider {
                    self.ui.input.wheel.y += delta.y / 42.0;
                }
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                self.fingers.at.retain(|(id, _)| *id != t.id);
                if self.fingers.at.len() < 2 {
                    self.fingers.pinch = None;
                }
                let Some((id, _, drag)) = self.fingers.main else {
                    return;
                };
                if id != t.id {
                    return;
                }
                self.fingers.main = None;
                self.dragging = None;
                if drag || t.phase == TouchPhase::Cancelled {
                    // a scroll is not a click on what the finger came up over
                    self.ui.input.mouse = Vec2::new(-1e4, -1e4);
                } else {
                    self.ui.input.mouse = p;
                }
                self.ui.input.released = true;
                self.ui.input.down = false;
                self.fingers.lift = true;
            }
        }
    }

    /// Once a frame after the interface was drawn: a lifted finger leaves no hover behind.
    pub(super) fn touch_frame(&mut self) {
        if self.fingers.lift && !self.ui.input.released {
            self.fingers.lift = false;
            self.ui.input.mouse = Vec2::new(-1e4, -1e4);
        }
    }

    /// The narrow rail: an icon for each page.
    /// Open the storage browser (a phone's "Browse").
    pub fn browse(&mut self, purpose: Purpose, start: &str) {
        self.browser = Some(Browser::new(purpose, start));
    }
}
