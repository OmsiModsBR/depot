//! The Alt bar in the running game, drawn by the new interface.
//!
//! A tap of Alt (pressed and let go with no other key or button between) shows or hides the
//! bar; a click on a button does it. The arrow at the bar's corner folds it down to a small
//! tab (and the tab unfolds it); Alt shows it unfolded again. The bar has the original's fifteen buttons, in its
//! order, five by three. It never pauses the game and never opens the Esc menu: every button
//! runs an action the game already has, directly, or opens a small window of its own for it
//! (`new_ui_windows`), and a button for something the game does not have yet is drawn dim
//! with the reason. While one of its windows is open the other buttons wait, except for the
//! wash and refuel menu, which leaves the bar free as the original's does. While the Esc
//! menu is open the bar is hidden.
//!
//! Keys: Alt+Down puts the keyboard on the bar (unless the keyboard file binds that chord);
//! then the arrows move between the buttons, Enter or Space press one, and Esc or Alt+Down
//! again hands the keys back to the game. While the keyboard is on the bar the keys above are
//! the bar's own and nothing else is.

use crate::app::App;
use crate::new_ui::scale_for;
use glam::Vec2;
use omsi_ui::alt_bar::{self, Button, Glyph, Hit, State as BarState, COLUMNS};
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
    /// What it switches is on (a tour is running, the pump or the wash runs): drawn pressed.
    pub on: bool,
}

/// What the game can do right now, as far as the buttons care.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct AltCtx {
    pub player: bool,
    pub other_vehicle: bool,
    /// The bus stands at a petrol station or in the depot's wash yard (where the pump and the
    /// wash work).
    pub at_station: bool,
    /// A vehicle follows the mouse, not yet put down.
    pub placing: bool,
    /// A player of a LAN session or a server: the host sets the clock and the weather and
    /// moves the vehicles on the map.
    pub lan_client: bool,
    /// On a server: no tours and no driver profile of one's own.
    pub server: bool,
    /// The map has a timetable.
    pub timetable: bool,
    /// A tour is being driven.
    pub on_tour: bool,
    /// The map has entry points to put the bus at.
    pub entry_points: bool,
    /// The icon of what the vehicle is driven with now (the keyboard, the mouse, a game
    /// controller): the controls button shows it.
    pub controls: &'static str,
    /// The pump ("refuel") or the wash ("wash") is running.
    pub pumping: Option<&'static str>,
}

/// Why a button for something the game does not have yet cannot be used.
const NOT_YET: &str = "Not in the game yet";
/// Why the other buttons wait while one of the bar's windows is open.
pub(crate) const WAIT: &str = "Close the open window first";
/// What the button of the window open says.
const OPEN: &str = "Open: Esc or Cancel closes it";

/// Why the wash and refuel button is dim away from a petrol station.
pub(crate) const NO_STATION: &str = "Only at a petrol station or in the depot's wash yard";

/// The bar's button whose menu leaves the other buttons free (the original's is not modal).
pub(crate) const FREE: &str = "service";

/// The buttons, in the order of the bar (rows of five): the original's fifteen, in its order.
pub(crate) fn buttons(ctx: &AltCtx) -> Vec<OwnedButton> {
    use OwnedGlyph::{Icon, Text};
    // (English: the widgets translate what they draw, a second time would garble it)
    let b = |id, glyph, title: &str, off: Option<&str>| OwnedButton {
        id,
        glyph,
        title: title.to_string(),
        disabled: off.map(String::from),
        on: false,
    };
    let need = |ok: bool, why: &'static str| (!ok).then_some(why);
    let first = |checks: &[(bool, &'static str)]| checks.iter().find(|c| !c.0).map(|c| c.1);
    vec![
        b(
            "add",
            Icon("add"),
            "Add vehicle",
            ctx.placing.then_some(crate::new_ui_add::PLACING),
        ),
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
            first(&[
                (ctx.player, "No vehicle to reposition"),
                (!ctx.lan_client, "In a LAN session only the host moves vehicles"),
                (ctx.entry_points, "This map has no entry points"),
            ]),
        ),
        b("ai", Text("AI"), "AI drives my vehicle", Some(NOT_YET)),
        OwnedButton {
            title: if ctx.on_tour { "End the tour" } else { "Line and tour" }.to_string(),
            on: ctx.on_tour,
            // (a tour running can always be ended)
            ..b(
                "tour",
                Icon("route"),
                "Line and tour",
                if ctx.on_tour {
                    None
                } else {
                    first(&[
                        (ctx.player, "No vehicle to drive a tour with"),
                        (!ctx.server, "Not on a server"),
                        (ctx.timetable, "No timetable on this map"),
                    ])
                },
            )
        },
        b(
            "destination",
            Icon("departure_board"),
            "Line and destination",
            need(ctx.player, "No vehicle"),
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
        // (a menu of two switches, as the original's: the pump and the wash run until they
        // are done or the bus drives off)
        OwnedButton {
            on: ctx.pumping.is_some(),
            ..b(
                "service",
                Icon("local_gas_station"),
                "Wash and refuel",
                first(&[
                    (ctx.player, "No vehicle to service"),
                    (ctx.at_station || ctx.pumping.is_some(), NO_STATION),
                ]),
            )
        },
        b("help", Icon("help"), "Help marker", Some(NOT_YET)),
        b(
            "time",
            Icon("schedule"),
            "Date and time",
            need(!ctx.lan_client, "In a LAN session the host sets the clock"),
        ),
        b(
            "weather",
            Icon("cloud"),
            "Weather",
            need(!ctx.lan_client, "In a LAN session the host sets the weather"),
        ),
        b(
            "controls",
            Icon(if ctx.controls.is_empty() {
                "keyboard"
            } else {
                ctx.controls
            }),
            "Controls",
            None,
        ),
        b(
            "driver",
            Icon("badge"),
            "Timetable evaluation and driver profile",
            None,
        ),
    ]
}

/// The bar while the window of button `open` is open: that button is drawn pressed and the
/// others wait (the window is modal over the bar), except while the wash and refuel menu is
/// open: it holds nothing up, as the original's does not.
pub(crate) fn wait_for(buttons: &mut [OwnedButton], open: &str) {
    if open == FREE {
        for b in buttons.iter_mut().filter(|b| b.id == open) {
            b.on = true;
        }
        return;
    }
    for b in buttons {
        let why = if b.id == open {
            b.on = true;
            OPEN
        } else {
            WAIT
        };
        b.disabled = Some(why.to_string());
    }
}

/// What the overlay draws: the buttons and the one the keyboard is on.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct AltFrame {
    pub buttons: Vec<OwnedButton>,
    pub focus: Option<&'static str>,
    /// Folded down to the tab in the corner.
    pub folded: bool,
}

/// The line at the bottom left while the bar is shown.
pub(crate) const HINT: &str = "Alt closes the bar · the simulation keeps running";

/// Where the bar's top left corner is in a window of `size` design points.
fn origin(size: Vec2, n: usize) -> Vec2 {
    size - alt_bar::panel_size(n) - Vec2::splat(MARGIN)
}

/// The folded bar's corner (its tab's bottom right) in a window of `size` design points.
fn corner(size: Vec2) -> Vec2 {
    size - Vec2::splat(MARGIN)
}

impl AltFrame {
    pub(crate) fn draw(&self, ui: &mut Ui) {
        alt_bar::hint(ui, HINT);
        if self.folded {
            alt_bar::draw_folded(ui, corner(ui.size));
            return;
        }
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
                on: b.on,
            })
            .collect();
        let at = origin(ui.size, bs.len());
        // while a window is open its button names it in the header
        let open = self
            .buttons
            .iter()
            .find(|b| b.on && b.disabled.as_deref() == Some(OPEN));
        let mut state = BarState {
            chosen: open.map(|b| b.id.to_string()),
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
    #[cfg(test)]
    pub(crate) fn hit(&self, px: Vec2, size: (u32, u32)) -> Option<usize> {
        match self.hit_any(px, size) {
            Some(Hit::Button(i)) => Some(i),
            _ => None,
        }
    }

    /// What is under `px`: a button or the arrow (the folded bar's tab is all arrow).
    pub(crate) fn hit_any(&self, px: Vec2, size: (u32, u32)) -> Option<Hit> {
        let (scale, at) = self.geometry(size);
        let p = px / scale;
        if self.folded {
            let pts = Vec2::new(size.0 as f32, size.1 as f32) / scale;
            return inside(alt_bar::tab_rect(corner(pts)), p).then_some(Hit::Arrow);
        }
        alt_bar::hit_any(at, self.buttons.len(), p)
    }

    /// Whether `px` is on the panel at all (buttons, gaps, the arrow), or on the folded tab.
    pub(crate) fn over(&self, px: Vec2, size: (u32, u32)) -> bool {
        let (scale, at) = self.geometry(size);
        let p = px / scale;
        if self.folded {
            let pts = Vec2::new(size.0 as f32, size.1 as f32) / scale;
            return inside(alt_bar::tab_rect(corner(pts)), p);
        }
        inside(alt_bar::panel_rect(at, self.buttons.len()), p)
    }
}

fn inside(r: omsi_ui::Rect, p: Vec2) -> bool {
    p.x >= r.x && p.x < r.x + r.w && p.y >= r.y && p.y < r.y + r.h
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

/// The button Enter or Space presses with the keyboard on button `focus`: none when that one
/// is dim.
pub(crate) fn pressable(bs: &[OwnedButton], focus: Option<usize>) -> Option<&'static str> {
    let b = bs.get(focus?)?;
    b.disabled.is_none().then_some(b.id)
}

/// The bar's state between frames.
#[derive(Debug, Default)]
pub(crate) struct AltState {
    /// The bar is shown (when nothing hides it).
    pub open: bool,
    /// Folded down to its tab (the arrow); Alt shows it unfolded again.
    pub folded: bool,
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
            self.folded = false;
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
        let controller = self
            .controllers
            .as_ref()
            .is_some_and(|c| c.enabled && c.any());
        AltCtx {
            player: self.player.is_some(),
            other_vehicle: !self.placed.is_empty(),
            at_station: self.at_service_station(),
            placing: self.placing.is_some(),
            lan_client: self.is_lan_client(),
            server: crate::input::on_server(&self.args),
            timetable: self.schedule.is_some(),
            on_tour: self.duty.is_some(),
            entry_points: self
                .world
                .as_ref()
                .is_some_and(|w| !w.global.entry_points.is_empty()),
            controls: if controller {
                "sports_esports"
            } else if self.mouse_drive {
                "mouse"
            } else {
                "keyboard"
            },
            pumping: self.pumping.map(|p| p.0),
        }
    }

    /// Whether the bus driven stands where the pump and the wash work.
    pub(crate) fn at_service_station(&self) -> bool {
        match (self.world.as_ref(), self.player.as_ref()) {
            (Some(w), Some(p)) => crate::services::on_petrol_station(w, &p.vehicle),
            _ => false,
        }
    }

    /// The bar as it is drawn now: while one of its windows is open the other buttons wait.
    pub(crate) fn alt_frame(&self) -> AltFrame {
        let mut buttons = buttons(&self.alt_ctx());
        if let Some(open) = self.alt_window_open() {
            wait_for(&mut buttons, open);
        }
        let focus = self
            .new_ui
            .alt
            .focus
            .and_then(|i| buttons.get(i))
            .map(|b| b.id);
        AltFrame {
            buttons,
            focus,
            folded: self.new_ui.alt.folded,
        }
    }

    /// The button whose window is open now, if one is.
    fn alt_window_open(&self) -> Option<&'static str> {
        if self.new_ui.add.is_some() {
            return Some("add");
        }
        self.new_ui.tool.window.as_ref().map(|w| w.button())
    }

    /// Run a button's action. None of them goes through the Esc menu: they neither pause the
    /// game nor close or open any menu; a window they open is the bar's own and the game
    /// goes on behind it.
    fn alt_do(&mut self, id: &str) {
        match id {
            "add" => self.open_add_vehicle(),
            "swap" => self.switch_vehicle(),
            "remove" => self.remove_driven_vehicle(),
            "repair" => self.run_service(id),
            "reposition" | "tour" | "destination" | "service" | "time" | "weather" | "controls"
            | "driver" => self.open_tool(id),
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
        match frame.hit_any(at, size) {
            Some(Hit::Arrow) => {
                self.new_ui.alt.folded = !self.new_ui.alt.folded;
                self.new_ui.alt.focus = None;
            }
            Some(Hit::Button(i)) => {
                if let Some(b) = frame.buttons.get(i).filter(|b| b.disabled.is_none()) {
                    self.new_ui.alt.focus = None;
                    self.alt_do(b.id);
                }
            }
            None => {}
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
            // (a folded bar unfolds for the keyboard)
            self.new_ui.alt.folded = false;
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
            // (a dim button does nothing, as with a click: the one of the window open would
            // open it again, and drop what was changed in it)
            if !repeat {
                if let Some(id) = pressable(&frame.buttons, self.new_ui.alt.focus) {
                    self.alt_do(id);
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
            at_station: true,
            placing: false,
            lan_client: false,
            server: false,
            timetable: true,
            on_tour: false,
            entry_points: true,
            controls: "keyboard",
            pumping: None,
        }
    }

    fn by_id<'a>(bs: &'a [OwnedButton], id: &str) -> &'a OwnedButton {
        bs.iter().find(|b| b.id == id).unwrap()
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
        // the original's fifteen, in its order: five by three
        let ids: Vec<&str> = bs.iter().map(|b| b.id).collect();
        assert_eq!(
            ids,
            vec![
                "add",
                "swap",
                "remove",
                "reposition",
                "ai",
                "tour",
                "destination",
                "accident",
                "repair",
                "service",
                "help",
                "time",
                "weather",
                "controls",
                "driver",
            ]
        );
        assert_eq!(bs.len(), COLUMNS * 3);
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
        assert_eq!(
            enabled_ids(&AltCtx::default()),
            vec!["add", "time", "weather", "controls", "driver"]
        );
        assert_eq!(
            enabled_ids(&all_on()),
            vec![
                "add",
                "swap",
                "remove",
                "reposition",
                "tour",
                "destination",
                "repair",
                "service",
                "time",
                "weather",
                "controls",
                "driver",
            ]
        );
        // one vehicle on the mouse at a time
        let placing = AltCtx {
            placing: true,
            ..all_on()
        };
        assert!(!enabled_ids(&placing).contains(&"add"));
        // a player of a LAN session: the host sets the clock and the weather and moves the
        // vehicles on the map
        let client = AltCtx {
            lan_client: true,
            ..all_on()
        };
        for id in ["reposition", "time", "weather"] {
            assert!(!enabled_ids(&client).contains(&id), "{id}");
        }
        // no entry points, no timetable, on a server: each says why
        let bs = buttons(&AltCtx {
            entry_points: false,
            timetable: false,
            ..all_on()
        });
        assert_eq!(
            by_id(&bs, "reposition").disabled.as_deref(),
            Some("This map has no entry points")
        );
        assert_eq!(
            by_id(&bs, "tour").disabled.as_deref(),
            Some("No timetable on this map")
        );
        let bs = buttons(&AltCtx {
            server: true,
            ..all_on()
        });
        assert_eq!(by_id(&bs, "tour").disabled.as_deref(), Some("Not on a server"));
        // what is not in the game yet stays dim whatever the state
        let bs = buttons(&all_on());
        for id in ["ai", "accident", "help"] {
            assert_eq!(by_id(&bs, id).disabled.as_deref(), Some(NOT_YET), "{id}");
        }
    }

    #[test]
    fn nothing_on_the_bar_sends_the_player_to_the_esc_menu() {
        let ctxs = [
            AltCtx::default(),
            all_on(),
            AltCtx {
                lan_client: true,
                server: true,
                on_tour: true,
                pumping: Some("wash"),
                ..all_on()
            },
        ];
        for ctx in ctxs {
            let mut bs = buttons(&ctx);
            wait_for(&mut bs, "weather");
            for b in buttons(&ctx).iter().chain(&bs) {
                for text in [Some(&b.title), b.disabled.as_ref()].into_iter().flatten() {
                    assert!(!text.contains("Esc menu"), "{}: {text}", b.id);
                    assert!(!text.contains("pauses"), "{}: {text}", b.id);
                }
            }
        }
    }

    #[test]
    fn a_running_tour_shows_pressed_and_its_button_ends_it() {
        let tour = |ctx: &AltCtx| by_id(&buttons(ctx), "tour").clone();
        let idle = tour(&all_on());
        assert!(!idle.on);
        assert_eq!(idle.title, "Line and tour");
        let on = AltCtx {
            on_tour: true,
            ..all_on()
        };
        let running = tour(&on);
        assert!(running.on && running.disabled.is_none());
        assert_eq!(running.title, "End the tour");
        // (a tour running can be ended whatever else holds)
        let anyway = tour(&AltCtx {
            timetable: false,
            server: true,
            ..on
        });
        assert!(anyway.disabled.is_none());
        // the wash and refuel button shows pressed while the pump or the wash runs
        let bs = buttons(&AltCtx {
            pumping: Some("refuel"),
            ..all_on()
        });
        assert!(by_id(&bs, "service").on);
        assert!(!by_id(&buttons(&all_on()), "service").on);
        // the controls button shows what drives the vehicle
        let pad = buttons(&AltCtx {
            controls: "sports_esports",
            ..all_on()
        });
        assert_eq!(
            by_id(&pad, "controls").glyph,
            OwnedGlyph::Icon("sports_esports")
        );
    }

    #[test]
    fn wash_and_refuel_works_at_a_petrol_station_only_and_holds_nothing_up() {
        let service = |ctx: &AltCtx| by_id(&buttons(ctx), "service").clone();
        assert!(service(&all_on()).disabled.is_none());
        let away = AltCtx {
            at_station: false,
            ..all_on()
        };
        assert_eq!(service(&away).disabled.as_deref(), Some(NO_STATION));
        // (running, it can be stopped wherever the bus is: driving off stops it anyway)
        let running = AltCtx {
            pumping: Some("wash"),
            ..away
        };
        assert!(service(&running).disabled.is_none());
        assert_eq!(
            service(&AltCtx::default()).disabled.as_deref(),
            Some("No vehicle to service")
        );
        // its menu open: the button shows pressed and the others stay usable
        let mut bs = buttons(&all_on());
        wait_for(&mut bs, FREE);
        assert!(by_id(&bs, FREE).on);
        assert_eq!(bs, {
            let mut plain = buttons(&all_on());
            plain.iter_mut().find(|b| b.id == FREE).unwrap().on = true;
            plain
        });
        assert_eq!(step_focus(&bs, None, Nav::Right), Some(0));
    }

    #[test]
    fn while_a_window_is_open_the_other_buttons_wait() {
        let mut bs = buttons(&all_on());
        wait_for(&mut bs, "time");
        for b in &bs {
            assert!(b.disabled.is_some(), "{} should wait", b.id);
            assert_eq!(b.on, b.id == "time", "{}", b.id);
        }
        assert_eq!(by_id(&bs, "add").disabled.as_deref(), Some(WAIT));
        // a click on the bar finds the button but does nothing with it (see `alt_click`)
        let frame = AltFrame {
            buttons: bs,
            ..Default::default()
        };
        let size = (1672, 941);
        let at = alt_bar::cell_rect(origin(Vec2::new(1672.0, 941.0), frame.buttons.len()), 0).center();
        let hit = frame.hit(at, size).and_then(|i| frame.buttons.get(i));
        assert!(hit.is_some_and(|b| b.disabled.is_some()));
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
        // a bar folded and hidden comes back unfolded
        s.alt_down();
        assert!(s.alt_up(true));
        s.folded = true;
        s.alt_down();
        assert!(s.alt_up(true));
        s.alt_down();
        assert!(s.alt_up(true));
        assert!(s.open && !s.folded);
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
        let at = |id: &str| bs.iter().position(|b| b.id == id);
        let first = step_focus(&bs, None, Nav::Right);
        assert_eq!(id(first), "add");
        let next = step_focus(&bs, first, Nav::Right);
        assert_eq!(id(next), "swap");
        // Left from the first goes round to the last
        assert_eq!(id(step_focus(&bs, first, Nav::Left)), "driver");
        // Down goes along the column to the next usable one, round the bar if need be
        assert_eq!(id(step_focus(&bs, at("service"), Nav::Down)), "driver");
        assert_eq!(id(step_focus(&bs, at("driver"), Nav::Up)), "service");
        assert_eq!(id(step_focus(&bs, at("repair"), Nav::Down)), "controls");
        assert_eq!(id(step_focus(&bs, at("controls"), Nav::Down)), "reposition");
        // (over the dim "Help marker" below it, to the line and tour)
        assert_eq!(id(step_focus(&bs, first, Nav::Up)), "tour");
        // a few usable: Right goes round them, Down stays where its column has none
        let few = buttons(&AltCtx {
            lan_client: true,
            ..Default::default()
        });
        let at = |id: &str| few.iter().position(|b| b.id == id);
        assert_eq!(step_focus(&few, at("driver"), Nav::Right), at("add"));
        assert_eq!(step_focus(&few, at("add"), Nav::Right), at("controls"));
        assert_eq!(step_focus(&few, at("controls"), Nav::Down), at("controls"));
        assert_eq!(step_focus(&few, None, Nav::Right), at("add"));
        // a window open: nothing on the bar takes the keyboard, and Enter on the button the
        // keyboard was left on does not open its window again
        let mut waiting = buttons(&all_on());
        assert_eq!(pressable(&waiting, at("time")), Some("time"));
        wait_for(&mut waiting, "time");
        assert_eq!(step_focus(&waiting, None, Nav::Right), None);
        assert_eq!(pressable(&waiting, at("time")), None);
        assert_eq!(pressable(&waiting, None), None);
    }

    #[test]
    fn clicks_find_the_buttons_at_any_window_size() {
        let mut frame = AltFrame {
            buttons: buttons(&all_on()),
            ..Default::default()
        };
        for size in [(1280u32, 720u32), (1672, 941), (3840, 2160), (960, 540)] {
            let scale = scale_for(size.0, size.1);
            let pts = Vec2::new(size.0 as f32, size.1 as f32) / scale;
            let at = origin(pts, frame.buttons.len());
            for i in 0..frame.buttons.len() {
                // the centre of cell i, in window pixels
                let p = alt_bar::cell_rect(at, i).center();
                assert_eq!(frame.hit(p * scale, size), Some(i), "{size:?} button {i}");
                assert!(frame.over(p * scale, size));
            }
            assert!(!frame.over(Vec2::new(2.0, 2.0), size));
            // the panel sits inside the window
            let r = alt_bar::panel_rect(at, frame.buttons.len());
            assert!(r.x >= 0.0 && r.y >= 0.0 && r.x + r.w <= pts.x && r.y + r.h <= pts.y);
            // the arrow folds it; folded, only the tab in the corner is the bar's
            let arrow = alt_bar::arrow_rect(at, frame.buttons.len()).center();
            assert_eq!(frame.hit_any(arrow * scale, size), Some(Hit::Arrow), "{size:?}");
            frame.folded = true;
            let mid = alt_bar::cell_rect(at, 7).center();
            assert!(!frame.over(mid * scale, size));
            let tab = alt_bar::tab_rect(corner(pts)).center();
            assert_eq!(frame.hit_any(tab * scale, size), Some(Hit::Arrow));
            frame.folded = false;
        }
    }
}
