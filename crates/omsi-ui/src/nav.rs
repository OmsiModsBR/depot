//! Keyboard focus for the widget kit: Tab and Shift+Tab walk the clickable widgets in the
//! order they are drawn, Enter or Space presses the one chosen, and a ring shows which it is.
//!
//! It is off by default (`Ui::nav.enabled`), so a page that does not ask for it behaves as
//! before. The ring appears when a key moves the focus and goes when the mouse is pressed or
//! Esc is.
//!
//! Only widgets that do something when clicked are stops (`Ui::interact_key`): buttons, icon
//! buttons, switches, dropdowns, date fields and list rows. Sliders, text fields and scroll
//! bars are not, since Enter would do nothing on them. A widget scrolled out of its list is
//! not a stop either, and nothing scrolls the chosen one into view yet (a known gap).
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
    /// Where the ring goes: the part of the chosen widget that shows, and whether all of it.
    rect: Option<(Rect, bool)>,
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
            // (the Esc that only takes the ring away is not passed on to the page)
            let before = input.keys.len();
            input.keys.retain(|k| *k != Key::Escape);
            if input.keys.len() != before {
                self.shown = false;
                self.chosen = None;
            }
        }
    }

    /// A widget that acts when clicked was drawn at `r`, `shown` being the part of it inside
    /// the clip. True when the keyboard pressed it. A widget with nothing showing (scrolled
    /// out of its list) is not a stop.
    pub(crate) fn seen(&mut self, id: Id, r: Rect, shown: Rect) -> bool {
        if !self.enabled || shown.w <= 0.0 || shown.h <= 0.0 {
            return false;
        }
        if self.now.last() != Some(&id) {
            self.now.push(id);
        }
        if self.chosen == Some(id) {
            let whole = (shown.w - r.w).abs() < 0.5 && (shown.h - r.h).abs() < 0.5;
            self.rect = Some((shown, whole));
            if self.activate {
                self.activate = false;
                return true;
            }
        }
        false
    }

    /// Where the ring goes this frame. It is drawn a little outside a widget that shows in
    /// full and on the visible part of one that is cut by a scroll area.
    pub(crate) fn ring(&mut self) -> Option<Rect> {
        let (r, whole) = self.rect.take().filter(|_| self.shown)?;
        Some(if whole {
            Rect::new(r.x - 2.0, r.y - 2.0, r.w + 4.0, r.h + 4.0)
        } else {
            r
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::{id_of, ButtonKind, Ui};
    use glam::Vec2;

    fn button_rect(i: usize) -> Rect {
        Rect::new(10.0, 10.0 + 50.0 * i as f32, 100.0, 40.0)
    }

    /// One frame with `body` drawing the page; the keys and text are this frame's input.
    fn frame_with<T>(
        ui: &mut Ui,
        keys: &[Key],
        shift: bool,
        text: &str,
        body: impl FnOnce(&mut Ui) -> T,
    ) -> T {
        ui.input.keys.extend_from_slice(keys);
        ui.input.shift = shift;
        ui.input.text.push_str(text);
        ui.begin(Vec2::new(400.0, 300.0), 1.0, 0.016);
        let out = body(ui);
        ui.finish();
        out
    }

    fn three(ui: &mut Ui) -> [bool; 3] {
        let mut out = [false; 3];
        for (i, o) in out.iter_mut().enumerate() {
            *o = ui.button(
                &format!("b{i}"),
                button_rect(i),
                "x",
                None,
                ButtonKind::Normal,
            );
        }
        out
    }

    fn frame(ui: &mut Ui, keys: &[Key], shift: bool, text: &str) -> [bool; 3] {
        frame_with(ui, keys, shift, text, three)
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
    fn enter_and_space_press_only_the_chosen_button_and_a_held_enter_repeats() {
        let mut ui = ui();
        frame(&mut ui, &[Key::Tab], false, "");
        frame(&mut ui, &[Key::Tab], false, "");
        assert_eq!(
            frame(&mut ui, &[Key::Enter], false, ""),
            [false, true, false]
        );
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
    fn a_mouse_press_takes_the_ring_away() {
        let mut ui = ui();
        frame(&mut ui, &[Key::Tab], false, "");
        assert!(ui.nav.chosen().is_some());
        ui.input.pressed = true;
        ui.input.mouse = Vec2::new(300.0, 250.0);
        frame(&mut ui, &[], false, "");
        assert_eq!(ui.nav.chosen(), None);
    }

    #[test]
    fn esc_takes_the_ring_away_and_is_not_passed_on_but_without_a_ring_it_is() {
        let mut ui = ui();
        frame(&mut ui, &[Key::Tab], false, "");
        let seen = frame_with(&mut ui, &[Key::Escape], false, "", |ui| {
            ui.input.keys.clone()
        });
        assert!(seen.is_empty());
        assert_eq!(ui.nav.chosen(), None);
        let seen = frame_with(&mut ui, &[Key::Escape], false, "", |ui| {
            ui.input.keys.clone()
        });
        assert_eq!(seen, vec![Key::Escape]);
    }

    #[test]
    fn sliders_and_text_fields_are_not_stops() {
        let mut ui = Ui::new();
        ui.nav.enabled = true;
        let page = |ui: &mut Ui| {
            let mut v = 0.5;
            let mut text = String::new();
            ui.slider("sl", button_rect(0), &mut v, 0.0, 1.0, 0.1, "", &|x| {
                format!("{x}")
            });
            ui.text_input("tx", button_rect(1), &mut text, "", None);
            ui.button("only", button_rect(2), "x", None, ButtonKind::Normal)
        };
        frame_with(&mut ui, &[], false, "", page);
        frame_with(&mut ui, &[Key::Tab], false, "", page);
        assert_eq!(ui.nav.chosen(), Some(id_of("only")));
        frame_with(&mut ui, &[Key::Tab], false, "", page);
        assert_eq!(ui.nav.chosen(), Some(id_of("only")));
    }

    #[test]
    fn a_widget_scrolled_out_of_view_is_skipped() {
        let mut ui = Ui::new();
        ui.nav.enabled = true;
        let page = |ui: &mut Ui| {
            ui.button("in", button_rect(0), "x", None, ButtonKind::Normal);
            ui.push_clip(Rect::new(0.0, 0.0, 400.0, 70.0), 0.0);
            ui.button(
                "out",
                Rect::new(10.0, 200.0, 100.0, 40.0),
                "x",
                None,
                ButtonKind::Normal,
            );
            ui.pop_clip();
        };
        frame_with(&mut ui, &[], false, "", page);
        frame_with(&mut ui, &[Key::Tab], false, "", page);
        frame_with(&mut ui, &[Key::Tab], false, "", page);
        assert_eq!(ui.nav.chosen(), Some(id_of("in")));
    }

    #[test]
    fn an_open_dropdown_keeps_the_keys() {
        let mut ui = Ui::new();
        ui.nav.enabled = true;
        let options = vec!["a".to_string(), "b".to_string()];
        let r = button_rect(0);
        let page = |ui: &mut Ui| {
            let mut sel = 0;
            ui.select("dd", r, &mut sel, &options);
            ui.button("b", button_rect(1), "x", None, ButtonKind::Normal);
        };
        frame_with(&mut ui, &[], false, "", page);
        // click the dropdown open
        ui.input.mouse = r.center();
        ui.input.pressed = true;
        ui.input.down = true;
        frame_with(&mut ui, &[], false, "", page);
        ui.input.released = true;
        ui.input.down = false;
        frame_with(&mut ui, &[], false, "", page);
        let kept = frame_with(&mut ui, &[Key::Tab], false, "", |ui| {
            page(ui);
            ui.input.keys.clone()
        });
        assert_eq!(ui.nav.chosen(), None);
        assert!(kept.is_empty() || kept == vec![Key::Tab]);
    }

    #[test]
    fn tab_leaves_a_focused_text_field() {
        let mut ui = ui();
        ui.focus = Some(id_of("field"));
        frame(&mut ui, &[Key::Tab], false, "");
        assert_eq!(ui.focus, None);
        assert_eq!(ui.nav.chosen(), Some(id_of("b0")));
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
