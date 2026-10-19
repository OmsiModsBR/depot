//! Keyboard focus for the widget kit: Tab and Shift+Tab walk the clickable widgets in the
//! order they are drawn, Enter or Space presses the one chosen, and a ring shows which it is.
//!
//! It is off by default (`Ui::nav.enabled`), so a page that does not ask for it behaves as
//! before. The ring appears when a key moves the focus and goes when the mouse is pressed or
//! Esc is.
//!
//! Text fields keep their own focus (`Ui::focus`): Tab leaves one, but Tab cannot enter one
//! yet; that comes with the text field's own key handling.

use crate::widgets::{Id, Input, Key};
use crate::Rect;

#[derive(Default)]
pub struct KeyNav {
    pub enabled: bool,
    chosen: Option<Id>,
    shown: bool,
    /// The widgets of the last frame, in drawing order: what Tab walks.
    prev: Vec<Id>,
    now: Vec<Id>,
    rect: Option<Rect>,
    activate: bool,
}

impl KeyNav {
    /// The widget the keyboard has chosen, when the ring is showing.
    pub fn chosen(&self) -> Option<Id> {
        self.chosen.filter(|_| self.shown)
    }

    /// Take this frame's keys. `modal`: a dropdown or date picker is open and has the keys.
    pub(crate) fn begin(&mut self, input: &mut Input, text_focus: &mut Option<Id>, modal: bool) {
        if !self.enabled {
            return;
        }
        self.prev = std::mem::take(&mut self.now);
        self.rect = None;
        self.activate = false;
        if input.pressed {
            self.shown = false;
            self.chosen = None;
        }
        if modal {
            return;
        }
        let mut tab = false;
        input.keys.retain(|k| {
            tab |= *k == Key::Tab;
            *k != Key::Tab
        });
        if tab && !self.prev.is_empty() {
            *text_focus = None;
            let n = self.prev.len();
            let at = self
                .chosen
                .and_then(|c| self.prev.iter().position(|i| *i == c));
            let next = match (at, input.shift) {
                (Some(i), false) => (i + 1) % n,
                (Some(i), true) => (i + n - 1) % n,
                (None, false) => 0,
                (None, true) => n - 1,
            };
            self.chosen = Some(self.prev[next]);
            self.shown = true;
        }
        if self.shown && text_focus.is_none() {
            if let Some(i) = input.keys.iter().position(|k| *k == Key::Enter) {
                input.keys.remove(i);
                self.activate = true;
            } else if input.text == " " {
                input.text.clear();
                self.activate = true;
            }
            if input.keys.contains(&Key::Escape) {
                self.shown = false;
                self.chosen = None;
            }
        }
    }

    /// A clickable widget was drawn at `r`. True when the keyboard pressed it.
    pub(crate) fn seen(&mut self, id: Id, r: Rect, visible: bool) -> bool {
        if !self.enabled || !visible {
            return false;
        }
        if self.now.last() != Some(&id) {
            self.now.push(id);
        }
        if self.chosen == Some(id) {
            self.rect = Some(r);
            if self.activate {
                self.activate = false;
                return true;
            }
        }
        false
    }

    /// Where the ring goes this frame.
    pub(crate) fn ring(&mut self) -> Option<Rect> {
        let r = self.rect.take();
        r.filter(|_| self.shown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::{id_of, ButtonKind, Ui};
    use glam::Vec2;

    fn frame(ui: &mut Ui, keys: &[Key], shift: bool, text: &str) -> [bool; 3] {
        ui.input.keys.extend_from_slice(keys);
        ui.input.shift = shift;
        ui.input.text.push_str(text);
        ui.begin(Vec2::new(400.0, 300.0), 1.0, 0.016);
        let mut out = [false; 3];
        for (i, o) in out.iter_mut().enumerate() {
            let r = Rect::new(10.0, 10.0 + 50.0 * i as f32, 100.0, 40.0);
            *o = ui.button(&format!("b{i}"), r, "x", None, ButtonKind::Normal);
        }
        ui.finish();
        out
    }

    fn ui() -> Ui {
        let mut ui = Ui::new();
        ui.nav.enabled = true;
        frame(&mut ui, &[], false, "");
        ui
    }

    #[test]
    fn tab_walks_the_buttons_and_wraps_and_shift_tab_goes_back() {
        let mut ui = ui();
        assert_eq!(ui.nav.chosen(), None);
        frame(&mut ui, &[Key::Tab], false, "");
        assert_eq!(ui.nav.chosen(), Some(id_of("b0")));
        frame(&mut ui, &[Key::Tab], false, "");
        frame(&mut ui, &[Key::Tab], false, "");
        assert_eq!(ui.nav.chosen(), Some(id_of("b2")));
        frame(&mut ui, &[Key::Tab], false, "");
        assert_eq!(ui.nav.chosen(), Some(id_of("b0")));
        frame(&mut ui, &[Key::Tab], true, "");
        assert_eq!(ui.nav.chosen(), Some(id_of("b2")));
    }

    #[test]
    fn enter_and_space_press_only_the_chosen_button() {
        let mut ui = ui();
        frame(&mut ui, &[Key::Tab], false, "");
        frame(&mut ui, &[Key::Tab], false, "");
        assert_eq!(
            frame(&mut ui, &[Key::Enter], false, ""),
            [false, true, false]
        );
        assert_eq!(frame(&mut ui, &[], false, " "), [false, true, false]);
        assert_eq!(frame(&mut ui, &[], false, ""), [false; 3]);
    }

    #[test]
    fn enter_does_nothing_before_the_keyboard_has_chosen() {
        let mut ui = ui();
        assert_eq!(frame(&mut ui, &[Key::Enter], false, ""), [false; 3]);
    }

    #[test]
    fn a_mouse_press_or_esc_takes_the_ring_away() {
        let mut ui = ui();
        frame(&mut ui, &[Key::Tab], false, "");
        assert!(ui.nav.chosen().is_some());
        frame(&mut ui, &[Key::Escape], false, "");
        assert_eq!(ui.nav.chosen(), None);
        frame(&mut ui, &[Key::Tab], false, "");
        ui.input.pressed = true;
        ui.input.mouse = Vec2::new(300.0, 250.0);
        frame(&mut ui, &[], false, "");
        assert_eq!(ui.nav.chosen(), None);
    }

    #[test]
    fn it_does_nothing_unless_asked_for() {
        let mut ui = Ui::new();
        frame(&mut ui, &[], false, "");
        frame(&mut ui, &[Key::Tab], false, "");
        assert_eq!(ui.nav.chosen(), None);
        assert_eq!(frame(&mut ui, &[Key::Enter], false, ""), [false; 3]);
    }
}
