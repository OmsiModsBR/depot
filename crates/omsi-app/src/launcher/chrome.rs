//! The window's own frame on a computer: the launcher draws its title bar as Windows 11 apps do
//! (no system title bar, see `resumed`): the caption buttons top right, the strip along the
//! top that moves the window (a double click maximises it), and the edges that resize it.

use super::Launcher;
use glam::Vec2;
use omsi_launcher_ui::look::{CAPTION_H, CAPTION_W, TOPBAR_H};
use omsi_launcher_ui::shell::Caption;
use std::time::{Duration, Instant};
use winit::window::{CursorIcon, ResizeDirection, Window};

/// How far in from the window's edge a press resizes it (points).
const EDGE: f32 = 5.0;

/// What the frame needs to remember between frames.
#[derive(Default)]
pub struct Chrome {
    /// The caption button clicked in the frame just drawn.
    pub clicked: Option<Caption>,
    /// The window is maximised (the middle caption button restores it then).
    pub maximized: bool,
    /// When the title strip was last pressed (two presses close together maximise).
    last_press: Option<Instant>,
}

/// The edge or corner of a window `size` points big that `p` is on, if any.
pub fn edge_at(p: Vec2, size: Vec2) -> Option<ResizeDirection> {
    let (l, r) = (p.x < EDGE, p.x > size.x - EDGE);
    let (t, b) = (p.y < EDGE, p.y > size.y - EDGE);
    if !(0.0..=size.x).contains(&p.x) || !(0.0..=size.y).contains(&p.y) {
        return None;
    }
    Some(match (l, r, t, b) {
        (true, _, true, _) => ResizeDirection::NorthWest,
        (_, true, true, _) => ResizeDirection::NorthEast,
        (true, _, _, true) => ResizeDirection::SouthWest,
        (_, true, _, true) => ResizeDirection::SouthEast,
        (true, ..) => ResizeDirection::West,
        (_, true, ..) => ResizeDirection::East,
        (_, _, true, _) => ResizeDirection::North,
        (_, _, _, true) => ResizeDirection::South,
        _ => return None,
    })
}

fn cursor_of(d: ResizeDirection) -> CursorIcon {
    match d {
        ResizeDirection::East => CursorIcon::EResize,
        ResizeDirection::West => CursorIcon::WResize,
        ResizeDirection::North => CursorIcon::NResize,
        ResizeDirection::South => CursorIcon::SResize,
        ResizeDirection::NorthEast => CursorIcon::NeResize,
        ResizeDirection::NorthWest => CursorIcon::NwResize,
        ResizeDirection::SouthEast => CursorIcon::SeResize,
        ResizeDirection::SouthWest => CursorIcon::SwResize,
    }
}

/// Whether `p` is on the strip that moves the window: along the top, `h` points high, left of
/// the caption buttons, of a window `w` points wide.
pub fn on_title_strip(p: Vec2, w: f32, h: f32) -> bool {
    p.y >= 0.0 && p.y < h && p.x >= 0.0 && p.x < w - CAPTION_W * 3.0
}

impl Launcher {
    /// Before the page is drawn: a press on the window's edge resizes it (and is not the
    /// page's). Not while maximised.
    pub(super) fn chrome_before(&mut self, window: &Window, size: Vec2) {
        if window.is_maximized() || window.fullscreen().is_some() {
            return;
        }
        let Some(d) = edge_at(self.ui.input.mouse, size) else { return };
        if self.ui.input.pressed {
            let _ = window.drag_resize_window(d);
            self.ui.input.pressed = false;
            self.ui.input.down = false;
        }
    }

    /// After the page is drawn: the caption button clicked, a press on the free title strip
    /// (moves the window, twice maximises it) and the resize cursor on an edge. True when the
    /// close button was clicked.
    pub(super) fn chrome_after(&mut self, window: &Window, size: Vec2) -> bool {
        let maximized = window.is_maximized();
        match self.chrome.clicked.take() {
            Some(Caption::Minimize) => window.set_minimized(true),
            Some(Caption::Maximize) => window.set_maximized(!maximized),
            Some(Caption::Close) => return true,
            None => {}
        }
        let p = self.ui.input.mouse;
        // (the top bar's free part moves the window too, as a title bar's)
        let strip = if self.top_nav() { TOPBAR_H } else { CAPTION_H };
        if self.ui.input.pressed && self.ui.hot.is_none() && self.ui.active.is_none() && on_title_strip(p, size.x, strip) {
            let now = Instant::now();
            let double = self.chrome.last_press.is_some_and(|t| now.duration_since(t) < Duration::from_millis(400));
            if double {
                self.chrome.last_press = None;
                window.set_maximized(!maximized);
            } else {
                self.chrome.last_press = Some(now);
                let _ = window.drag_window();
            }
            // (the press moved the window: no page sees it)
            self.ui.input.pressed = false;
            self.ui.input.down = false;
        }
        if !maximized && window.fullscreen().is_none() {
            if let Some(d) = edge_at(p, size) {
                self.ui.cursor = cursor_of(d);
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges_and_corners_resize_and_the_inside_does_not() {
        let size = Vec2::new(1440.0, 880.0);
        assert_eq!(edge_at(Vec2::new(1.0, 400.0), size), Some(ResizeDirection::West));
        assert_eq!(edge_at(Vec2::new(1438.0, 400.0), size), Some(ResizeDirection::East));
        assert_eq!(edge_at(Vec2::new(700.0, 1.0), size), Some(ResizeDirection::North));
        assert_eq!(edge_at(Vec2::new(700.0, 878.0), size), Some(ResizeDirection::South));
        assert_eq!(edge_at(Vec2::new(2.0, 2.0), size), Some(ResizeDirection::NorthWest));
        assert_eq!(edge_at(Vec2::new(1438.0, 878.0), size), Some(ResizeDirection::SouthEast));
        assert_eq!(edge_at(Vec2::new(700.0, 400.0), size), None);
        assert_eq!(edge_at(Vec2::new(-10.0, 400.0), size), None);
    }

    #[test]
    fn the_title_strip_ends_at_the_caption_buttons() {
        assert!(on_title_strip(Vec2::new(600.0, 10.0), 1440.0, CAPTION_H));
        assert!(!on_title_strip(Vec2::new(600.0, 40.0), 1440.0, CAPTION_H));
        assert!(on_title_strip(Vec2::new(600.0, 40.0), 1440.0, TOPBAR_H));
        assert!(!on_title_strip(Vec2::new(1440.0 - 60.0, 10.0), 1440.0, TOPBAR_H));
    }
}
