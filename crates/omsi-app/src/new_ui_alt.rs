//! The Alt bar in the running game, drawn by the new interface.
//!
//! A tap of Alt (pressed and let go with no other key or button between) shows or hides the
//! bar; a click on a button does it. The bar never pauses the game and never opens the Esc
//! menu: every button runs an action that is already there for the Esc menu, directly, and a
//! button for something the game does not have yet is drawn dim with the reason. While the
//! Esc menu is open the bar is hidden.
//!
//! Keys: Alt+Down puts the keyboard on the bar (unless the keyboard file binds that chord);
//! then the arrows move between the buttons, Enter or Space press one, and Esc or Alt+Down
//! again hands the keys back to the game. While the keyboard is on the bar the keys above are
//! the bar's own and nothing else is.

use crate::app::App;
use crate::new_ui::scale_for;
use glam::Vec2;
use omsi_ui::alt_bar::{self, Button, Glyph, State as BarState, COLUMNS};
use omsi_ui::widgets::Ui;
use winit::keyboard::KeyCode;

const MARGIN: f32 = 20.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum OwnedGlyph {
    Icon(&'static str),
    Text(&'static str),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OwnedButton {
    pub id: &'static str,
    pub glyph: OwnedGlyph,
    pub title: String,
    pub disabled: Option<String>,
}

/// What the game can do right now, as far as the buttons care.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct AltCtx {
    pub player: bool,
    pub other_vehicle: bool,
    pub stop_to_skip: bool,
}

/// Why a button for something the game does not have yet cannot be used.
const NOT_YET: &str = "Not in the game yet";
/// Why a button for something that opens a page cannot be used: the page pauses the game.
const IN_MENU: &str = "Open it from the Esc menu (it pauses the game)";

/// The buttons, in the order of the bar (rows of five).
pub(crate) fn buttons(ctx: &AltCtx) -> Vec<OwnedButton> {
    use OwnedGlyph::{Icon, Text};
    // (English: the widgets translate what they draw, a second time would garble it)
    let b = |id, glyph, title: &str, off: Option<&str>| OwnedButton {
        id,
        glyph,
        title: title.to_string(),
        disabled: off.map(String::from),
    };
    let need = |ok: bool, why: &'static str| (!ok).then_some(why);
    vec![
        b("add", Icon("add"), "Add vehicle", None),
        b(
            "swap",
            Icon("sync_alt"),
            "Switch vehicle",
            need(ctx.other_vehicle, "There is no other vehicle to drive"),
        ),
        b(
            "remove",
            Icon("close"),
            "Remove vehicle",
            need(ctx.player, "No vehicle to remove"),
        ),
        b(
            "reposition",
            Icon("download"),
            "Reposition at an entry point",
            Some(IN_MENU),
        ),
        b("ai", Text("AI"), "AI drives my vehicle", Some(NOT_YET)),
        b(
            "skip",
            Icon("turn_slight_right"),
            "Skip the next stop",
            need(ctx.stop_to_skip, "No stop to skip"),
        ),
        b(
            "accident",
            Icon("warning"),
            "Clear the accident",
            Some(NOT_YET),
        ),
        b(
            "repair",
            Icon("construction"),
            "Repair",
            need(ctx.player, "No vehicle to repair"),
        ),
        b(
            "wash",
            Icon("water_drop"),
            "Wash",
            need(ctx.player, "No vehicle to wash"),
        ),
        b(
            "refuel",
            Text("Fuel"),
            "Refuel",
            need(ctx.player, "No vehicle to refuel"),
        ),
        b("help", Icon("help"), "Help marker", Some(NOT_YET)),
        b("time", Icon("schedule"), "Date and time", Some(IN_MENU)),
        b("weather", Icon("cloud"), "Weather", Some(IN_MENU)),
        b("controls", Icon("keyboard"), "Controls", Some(IN_MENU)),
        b("shot", Icon("photo_camera"), "Screenshot", None),
    ]
}

/// What the overlay draws: the buttons and the one the keyboard is on.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct AltFrame {
    pub buttons: Vec<OwnedButton>,
    pub focus: Option<&'static str>,
}

/// Where the bar's top left corner is in a window of `size` design points.
fn origin(size: Vec2, n: usize) -> Vec2 {
    size - alt_bar::panel_size(n) - Vec2::splat(MARGIN)
}

impl AltFrame {
    pub(crate) fn draw(&self, ui: &mut Ui) {
        let bs: Vec<Button> = self
            .buttons
            .iter()
            .map(|b| Button {
                id: b.id,
                glyph: match b.glyph {
                    OwnedGlyph::Icon(i) => Glyph::Icon(i),
                    OwnedGlyph::Text(t) => Glyph::Text(t),
                },
                title: &b.title,
                disabled: b.disabled.as_deref(),
            })
            .collect();
        let at = origin(ui.size, bs.len());
        let mut state = BarState {
            chosen: None,
            focus: self.focus.map(String::from),
        };
        // (a click is the game's to take, not the overlay's: see `App::alt_click`)
        alt_bar::draw(ui, at, &bs, &mut state);
    }

    fn geometry(&self, size: (u32, u32)) -> (f32, Vec2) {
        let scale = scale_for(size.0, size.1);
        let pts = Vec2::new(size.0 as f32, size.1 as f32) / scale;
        (scale, origin(pts, self.buttons.len()))
    }

    /// The index of the button under `px` (window pixels) in a window of `size` pixels.
    pub(crate) fn hit(&self, px: Vec2, size: (u32, u32)) -> Option<usize> {
        let (scale, at) = self.geometry(size);
        alt_bar::hit(at, self.buttons.len(), px / scale)
    }

    /// Whether `px` is on the panel at all (buttons, gaps, header).
    pub(crate) fn over(&self, px: Vec2, size: (u32, u32)) -> bool {
        let (scale, at) = self.geometry(size);
        let r = alt_bar::panel_rect(at, self.buttons.len());
        let p = px / scale;
        p.x >= r.x && p.x < r.x + r.w && p.y >= r.y && p.y < r.y + r.h
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Nav {
    Left,
    Right,
    Up,
    Down,
}

/// The button the keyboard moves to from `from`: Left and Right walk the bar in order, Up and
/// Down walk the column; greyed-out buttons are stepped over. With no button chosen yet, the
/// first usable one. `from` when there is nowhere to go.
pub(crate) fn step_focus(bs: &[OwnedButton], from: Option<usize>, nav: Nav) -> Option<usize> {
    let n = bs.len();
    let usable = |i: usize| bs.get(i).is_some_and(|b| b.disabled.is_none());
    let Some(from) = from.filter(|&f| f < n) else {
        return (0..n).find(|&i| usable(i));
    };
    match nav {
        Nav::Left | Nav::Right => {
            for k in 1..=n {
                let i = if nav == Nav::Right {
                    (from + k) % n
                } else {
                    (from + n - k % n) % n
                };
                if usable(i) {
                    return Some(i);
                }
            }
            Some(from)
        }
        Nav::Up | Nav::Down => {
            let rows = n.div_ceil(COLUMNS);
            let (row, col) = (from / COLUMNS, from % COLUMNS);
            for k in 1..rows {
                let r = if nav == Nav::Down {
                    (row + k) % rows
                } else {
                    (row + rows - k) % rows
                };
                let i = r * COLUMNS + col;
                if usable(i) {
                    return Some(i);
                }
            }
            Some(from)
        }
    }
}

/// The bar's state between frames.
#[derive(Debug, Default)]
pub(crate) struct AltState {
    /// The bar is shown (when nothing hides it).
    pub open: bool,
    /// The keyboard is on this button.
    pub focus: Option<usize>,
    /// Alt went down and nothing else has since.
    tap: bool,
    /// A press on the bar was taken, so its release is too.
    swallowed: bool,
    /// Keys whose press the bar took: their release is the bar's too.
    taken: std::collections::HashSet<KeyCode>,
}

impl AltState {
    /// The window lost the keys (Alt+Tab, a dialog): no tap, press or key is pending any more,
    /// since their releases will not come.
    pub(crate) fn reset_input(&mut self) {
        self.tap = false;
        self.swallowed = false;
        self.taken.clear();
        self.focus = None;
    }

    pub(crate) fn alt_down(&mut self) {
        self.tap = true;
    }

    /// Another key or a mouse button went down: Alt is a modifier now, not a tap.
    pub(crate) fn other_down(&mut self) {
        self.tap = false;
    }

    /// Alt came up. True when it was a tap and the bar was shown or hidden by it. With
    /// `allowed` false (the Esc menu is open) the tap ends and does nothing.
    pub(crate) fn alt_up(&mut self, allowed: bool) -> bool {
        let tap = std::mem::take(&mut self.tap) && allowed;
        if tap {
            self.open = !self.open;
            if !self.open {
                self.focus = None;
            }
        }
        tap
    }
}

fn is_alt(code: KeyCode) -> bool {
    matches!(code, KeyCode::AltLeft | KeyCode::AltRight)
}

impl App {
    /// Whether the bar is showing: a session is running, the Esc menu is not open, and the
    /// picture is the plain window's.
    pub(crate) fn alt_bar_active(&self) -> bool {
        self.new_ui.alt.open
            && self.menu.is_none()
            && self.game_menu.is_none()
            && !self.vr_active()
            && !self.settings.triple.enabled
    }

    fn alt_ctx(&self) -> AltCtx {
        AltCtx {
            player: self.player.is_some(),
            other_vehicle: !self.placed.is_empty(),
            stop_to_skip: self.duty.as_ref().is_some_and(|d| d.stop_to_skip()),
        }
    }

    /// The bar as it is drawn now.
    pub(crate) fn alt_frame(&self) -> AltFrame {
        let buttons = buttons(&self.alt_ctx());
        let focus = self
            .new_ui
            .alt
            .focus
            .and_then(|i| buttons.get(i))
            .map(|b| b.id);
        AltFrame { buttons, focus }
    }

    /// Run a button's action. None of them goes through the Esc menu: they neither pause the
    /// game nor close or open any menu.
    fn alt_do(&mut self, id: &str) {
        match id {
            "add" => self.open_add_vehicle(),
            "swap" => self.switch_vehicle(),
            "remove" => self.remove_driven_vehicle(),
            "skip" => self.skip_next_stop(),
            "repair" | "wash" | "refuel" => self.run_service(id),
            "shot" => self.take_screenshot(),
            _ => {}
        }
    }

    /// A press or release of the left button (or a tap). True when the bar takes it; the game
    /// does not see it then.
    pub(crate) fn alt_click(&mut self, pressed: bool) -> bool {
        if !pressed {
            return std::mem::take(&mut self.new_ui.alt.swallowed);
        }
        self.new_ui.alt.other_down();
        if !self.alt_bar_active() {
            return false;
        }
        let Some(s) = self.surface.as_ref() else {
            return false;
        };
        let size = (s.config.width, s.config.height);
        let at = Vec2::new(self.cursor.0, self.cursor.1);
        let frame = self.alt_frame();
        if !frame.over(at, size) {
            return false;
        }
        self.new_ui.alt.swallowed = true;
        if let Some(b) = frame.hit(at, size).and_then(|i| frame.buttons.get(i)) {
            if b.disabled.is_none() {
                self.new_ui.alt.focus = None;
                self.alt_do(b.id);
            }
        }
        true
    }

    /// A key press or release, before the game sees it. True when the bar takes it.
    pub(crate) fn alt_key(&mut self, code: KeyCode, pressed: bool, repeat: bool) -> bool {
        if self.menu.is_some() {
            return false;
        }
        if is_alt(code) {
            if pressed && !repeat {
                self.new_ui.alt.alt_down();
            } else if !pressed {
                self.new_ui.alt.alt_up(self.game_menu.is_none());
            }
            return false;
        }
        // the release of a key whose press the bar took is the bar's; any other reaches the
        // game, whatever the state of the bar (a bound chord must see both its ends)
        if !pressed {
            return self.new_ui.alt.taken.remove(&code);
        }
        // (a held key the bar took repeats for the bar; only the arrows move the keyboard again)
        let arrow = matches!(
            code,
            KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::ArrowUp | KeyCode::ArrowDown
        );
        if repeat
            && self.new_ui.alt.taken.contains(&code)
            && !(arrow && self.new_ui.alt.focus.is_some())
        {
            return true;
        }
        let took = self.alt_key_press(code, repeat);
        if took && !repeat {
            self.new_ui.alt.taken.insert(code);
        }
        took
    }

    /// A key press (or its repeat) while the bar may want it. True when the bar takes it.
    fn alt_key_press(&mut self, code: KeyCode, repeat: bool) -> bool {
        let alt_held =
            self.keys.contains(&KeyCode::AltLeft) || self.keys.contains(&KeyCode::AltRight);
        if alt_held {
            self.new_ui.alt.other_down();
        }
        if !self.alt_bar_active() {
            return false;
        }
        let frame = self.alt_frame();
        let focused = self.new_ui.alt.focus.is_some();
        // Alt+Down puts the keyboard on the bar, and takes it off again
        if code == KeyCode::ArrowDown && alt_held {
            if repeat {
                return self.new_ui.alt.taken.contains(&code);
            }
            if self.chord_bound(code, false, false, true) {
                return false;
            }
            self.new_ui.alt.focus = if focused {
                None
            } else {
                step_focus(&frame.buttons, None, Nav::Right)
            };
            return true;
        }
        // (Alt+Enter stays the full screen switch)
        if !focused || alt_held {
            return false;
        }
        let nav = match code {
            KeyCode::ArrowLeft => Some(Nav::Left),
            KeyCode::ArrowRight => Some(Nav::Right),
            KeyCode::ArrowUp => Some(Nav::Up),
            KeyCode::ArrowDown => Some(Nav::Down),
            _ => None,
        };
        let press = matches!(code, KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space);
        if nav.is_none() && !press && code != KeyCode::Escape {
            return false;
        }
        if let Some(nav) = nav {
            let from = self.new_ui.alt.focus;
            self.new_ui.alt.focus = step_focus(&frame.buttons, from, nav);
        } else if press {
            if !repeat {
                if let Some(b) = self.new_ui.alt.focus.and_then(|i| frame.buttons.get(i)) {
                    self.alt_do(b.id);
                }
            }
        } else {
            self.new_ui.alt.focus = None;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_on() -> AltCtx {
        AltCtx {
            player: true,
            other_vehicle: true,
            stop_to_skip: true,
        }
    }

    #[test]
    fn every_button_has_a_unique_id_a_real_icon_and_a_reason_when_dim() {
        let bs = buttons(&AltCtx::default());
        let mut ids: Vec<_> = bs.iter().map(|b| b.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), bs.len());
        for b in &bs {
            if let OwnedGlyph::Icon(i) = b.glyph {
                assert!(omsi_ui::icons::svg(i).is_some(), "no icon {i}");
            }
            assert!(!b.title.is_empty());
            if let Some(why) = &b.disabled {
                assert!(!why.is_empty());
            }
        }
        // a bar of three rows of five
        assert_eq!(bs.len(), 3 * COLUMNS);
    }

    #[test]
    fn what_the_game_can_do_is_enabled_and_the_rest_says_why() {
        let enabled_ids = |ctx: &AltCtx| -> Vec<&'static str> {
            buttons(ctx)
                .into_iter()
                .filter(|b| b.disabled.is_none())
                .map(|b| b.id)
                .collect()
        };
        assert_eq!(enabled_ids(&AltCtx::default()), vec!["add", "shot"]);
        assert_eq!(
            enabled_ids(&all_on()),
            vec!["add", "swap", "remove", "skip", "repair", "wash", "refuel", "shot"]
        );
        // what is not in the game yet stays dim whatever the state
        let bs = buttons(&all_on());
        for id in ["ai", "accident", "help"] {
            let b = bs.iter().find(|b| b.id == id).unwrap();
            assert!(b.disabled.is_some(), "{id} should be dim");
        }
    }

    #[test]
    fn a_tap_of_alt_shows_the_bar_and_a_second_hides_it_but_alt_with_a_key_does_not() {
        let mut s = AltState::default();
        s.alt_down();
        assert!(s.alt_up(true));
        assert!(s.open);
        // Alt+S (the quicksave) is not a tap
        s.alt_down();
        s.other_down();
        assert!(!s.alt_up(true));
        assert!(s.open);
        // nor is Alt with a click
        s.alt_down();
        s.other_down();
        assert!(!s.alt_up(true));
        s.focus = Some(3);
        s.alt_down();
        assert!(s.alt_up(true));
        assert!(!s.open);
        assert_eq!(s.focus, None);
        // an Alt release with no press seen (the window had lost the focus) does nothing
        assert!(!s.alt_up(true));
        assert!(!s.open);
    }

    #[test]
    fn a_tap_with_the_esc_menu_open_ends_without_a_toggle_and_lost_input_clears_everything() {
        let mut s = AltState::default();
        // Alt down and up while the Esc menu is open: nothing is left pending
        s.alt_down();
        assert!(!s.alt_up(false));
        assert!(!s.open);
        // (the menu closed meanwhile: a release without a new press is not a tap)
        assert!(!s.alt_up(true));
        assert!(!s.open);
        // Alt+Tab: the release of Alt never comes, and what was pending must not toggle later
        s.open = true;
        s.alt_down();
        s.swallowed = true;
        s.focus = Some(2);
        s.taken.insert(KeyCode::ArrowDown);
        s.reset_input();
        assert!(!s.alt_up(true));
        assert!(s.open, "the bar stays as it was");
        assert!(!s.swallowed && s.focus.is_none() && s.taken.is_empty());
    }

    #[test]
    fn the_keyboard_walks_the_usable_buttons_only() {
        let bs = buttons(&all_on());
        let id = |i: Option<usize>| bs[i.unwrap()].id;
        let first = step_focus(&bs, None, Nav::Right);
        assert_eq!(id(first), "add");
        let next = step_focus(&bs, first, Nav::Right);
        assert_eq!(id(next), "swap");
        // Left from the first goes round to the last
        assert_eq!(id(step_focus(&bs, first, Nav::Left)), "shot");
        // Down goes along the column to the next usable one, round the bar if need be
        let refuel = bs.iter().position(|b| b.id == "refuel");
        assert_eq!(id(step_focus(&bs, refuel, Nav::Down)), "shot");
        let shot = bs.iter().position(|b| b.id == "shot");
        assert_eq!(id(step_focus(&bs, shot, Nav::Up)), "refuel");
        // nothing usable in the column: stays
        assert_eq!(id(step_focus(&bs, next, Nav::Down)), "swap");
        // only two usable: Right goes between them, Down stays
        let none = buttons(&AltCtx::default());
        let shot = none.iter().position(|b| b.id == "shot");
        let add = none.iter().position(|b| b.id == "add");
        assert_eq!(step_focus(&none, shot, Nav::Right), add);
        assert_eq!(step_focus(&none, shot, Nav::Down), shot);
        assert_eq!(step_focus(&none, None, Nav::Right), add);
    }

    #[test]
    fn clicks_find_the_buttons_at_any_window_size() {
        let frame = AltFrame {
            buttons: buttons(&all_on()),
            focus: None,
        };
        for size in [(1280u32, 720u32), (1672, 941), (3840, 2160), (960, 540)] {
            let scale = scale_for(size.0, size.1);
            let pts = Vec2::new(size.0 as f32, size.1 as f32) / scale;
            let at = origin(pts, frame.buttons.len());
            for i in 0..frame.buttons.len() {
                // the centre of cell i, in window pixels
                let (col, row) = ((i % COLUMNS) as f32, (i / COLUMNS) as f32);
                let p = at
                    + Vec2::new(
                        9.0 + col * (alt_bar::CELL_W + alt_bar::GAP) + alt_bar::CELL_W * 0.5,
                        alt_bar::HEADER_H
                            + 9.0
                            + row * (alt_bar::CELL_H + 6.0)
                            + alt_bar::CELL_H * 0.5,
                    );
                assert_eq!(frame.hit(p * scale, size), Some(i), "{size:?} button {i}");
                assert!(frame.over(p * scale, size));
            }
            assert!(!frame.over(Vec2::new(2.0, 2.0), size));
            // the panel sits inside the window
            let r = alt_bar::panel_rect(at, frame.buttons.len());
            assert!(r.x >= 0.0 && r.y >= 0.0 && r.x + r.w <= pts.x && r.y + r.h <= pts.y);
        }
    }
}
