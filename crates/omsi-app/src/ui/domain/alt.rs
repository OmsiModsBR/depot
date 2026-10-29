//! The Alt bar's state: its buttons and why one is dim, what the overlay draws of it, where
//! it stands in the window, the keyboard's walk over it and the tap of Alt that shows it.

use super::super::interface::output::PLACING;
use glam::Vec2;
use omsi_ui::alt_bar::{self, COLUMNS};
use winit::keyboard::KeyCode;

const MARGIN: f32 = 20.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OwnedGlyph {
    Icon(&'static str),
    Sign(&'static str),
}

#[derive(Clone, Debug, PartialEq)]
pub struct OwnedButton {
    pub id: &'static str,
    pub glyph: OwnedGlyph,
    pub title: String,
    pub disabled: Option<String>,
    /// What it switches is on (a tour is running, the pump or the wash runs): drawn pressed.
    pub on: bool,
}

/// What the game can do right now, as far as the buttons care.
#[derive(Clone, Copy, Debug, Default)]
pub struct AltCtx {
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
    /// The route arrows over the road are on (the help marker button is drawn pressed).
    pub route_arrows: bool,
}

/// Why a button for something the game does not have yet cannot be used.
pub const NOT_YET: &str = "Not in the game yet";

/// Why the other buttons wait while one of the bar's windows is open.
pub const WAIT: &str = "Close the open window first";

/// What the button of the window open says.
pub(in crate::ui) const OPEN: &str = "Open: Esc or Cancel closes it";

/// Why the wash and refuel button is dim away from a petrol station.
pub const NO_STATION: &str = "Only at a petrol station or in the depot's wash yard";

/// The bar's button whose menu leaves the other buttons free (the original's is not modal).
pub const FREE: &str = "service";

/// The buttons, in the order of the bar (rows of five): the original's fifteen, in its order.
pub fn buttons(ctx: &AltCtx) -> Vec<OwnedButton> {
    use OwnedGlyph::{Icon, Sign};
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
            Icon("alt-add"),
            "Add vehicle",
            ctx.placing.then_some(PLACING),
        ),
        b(
            "swap",
            Icon("alt-swap"),
            "Switch vehicle",
            need(ctx.other_vehicle, "There is no other vehicle to drive"),
        ),
        b(
            "remove",
            Icon("alt-remove"),
            "Remove vehicle",
            need(ctx.player, "No vehicle to remove"),
        ),
        b(
            "reposition",
            Icon("alt-reposition"),
            "Reposition at an entry point",
            first(&[
                (ctx.player, "No vehicle to reposition"),
                (
                    !ctx.lan_client,
                    "In a LAN session only the host moves vehicles",
                ),
                (ctx.entry_points, "This map has no entry points"),
            ]),
        ),
        b("ai", Icon("alt-ai"), "AI drives my vehicle", Some(NOT_YET)),
        OwnedButton {
            title: if ctx.on_tour {
                "Timetable"
            } else {
                "Line and tour"
            }
            .to_string(),
            on: ctx.on_tour,
            // (a tour running can always be ended)
            ..b(
                "tour",
                Icon("alt-tour"),
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
            Sign("92E"),
            "Line and destination",
            need(ctx.player, "No vehicle"),
        ),
        b(
            "accident",
            Icon("alt-accident"),
            "Clear the accident",
            Some(NOT_YET),
        ),
        b(
            "repair",
            Icon("alt-repair"),
            "Repair",
            need(ctx.player, "No vehicle to repair"),
        ),
        // (a menu of two switches, as the original's: the pump and the wash run until they
        // are done or the bus drives off)
        OwnedButton {
            on: ctx.pumping.is_some(),
            ..b(
                "service",
                Icon("alt-fuel"),
                "Wash and refuel",
                first(&[
                    (ctx.player, "No vehicle to service"),
                    (ctx.at_station || ctx.pumping.is_some(), NO_STATION),
                ]),
            )
        },
        // (switches the route arrows over the road, as the Esc menu's switch does)
        OwnedButton {
            on: ctx.route_arrows,
            ..b("help", Icon("alt-help"), "Help marker", None)
        },
        b(
            "time",
            Icon("alt-clock"),
            "Date and time",
            need(!ctx.lan_client, "In a LAN session the host sets the clock"),
        ),
        b(
            "weather",
            Icon("alt-weather"),
            "Weather",
            need(
                !ctx.lan_client,
                "In a LAN session the host sets the weather",
            ),
        ),
        b(
            "controls",
            Icon(if ctx.controls.is_empty() {
                "alt-wheel"
            } else {
                ctx.controls
            }),
            "Controls",
            None,
        ),
        b(
            "driver",
            Icon("alt-driver"),
            "Timetable evaluation and driver profile",
            None,
        ),
    ]
}

/// The bar while the window of button `open` is open: that button is drawn pressed and the
/// others wait (the window is modal over the bar), except while the wash and refuel menu is
/// open: it holds nothing up, as the original's does not.
pub fn wait_for(buttons: &mut [OwnedButton], open: &str) {
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
pub struct AltFrame {
    pub buttons: Vec<OwnedButton>,
    pub focus: Option<&'static str>,
    /// Folded down to the tab in the corner.
    pub folded: bool,
}

/// Where the bar's top left corner is in a window of `size` design points.
pub fn origin(size: Vec2, n: usize) -> Vec2 {
    size - alt_bar::panel_size(n) - Vec2::splat(MARGIN)
}

/// The folded bar's corner (its tab's bottom right) in a window of `size` design points.
pub fn corner(size: Vec2) -> Vec2 {
    size - Vec2::splat(MARGIN)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Nav {
    Left,
    Right,
    Up,
    Down,
}

/// The button the keyboard moves to from `from`: Left and Right walk the bar in order, Up and
/// Down walk the column; greyed-out buttons are stepped over. With no button chosen yet, the
/// first usable one. `from` when there is nowhere to go.
pub fn step_focus(bs: &[OwnedButton], from: Option<usize>, nav: Nav) -> Option<usize> {
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
pub fn pressable(bs: &[OwnedButton], focus: Option<usize>) -> Option<&'static str> {
    let b = bs.get(focus?)?;
    b.disabled.is_none().then_some(b.id)
}

/// The bar's state between frames.
#[derive(Debug, Default)]
pub struct AltState {
    /// The bar is shown (when nothing hides it).
    pub open: bool,
    /// Folded down to its tab (the arrow); Alt shows it unfolded again.
    pub folded: bool,
    /// The keyboard is on this button.
    pub focus: Option<usize>,
    /// Alt went down and nothing else has since.
    tap: bool,
    /// A press on the bar was taken, so its release is too.
    pub swallowed: bool,
    /// Keys whose press the bar took: their release is the bar's too.
    pub taken: std::collections::HashSet<KeyCode>,
}

impl AltState {
    /// The window lost the keys (Alt+Tab, a dialog): no tap, press or key is pending any more,
    /// since their releases will not come.
    pub fn reset_input(&mut self) {
        self.tap = false;
        self.swallowed = false;
        self.taken.clear();
        self.focus = None;
    }

    pub fn alt_down(&mut self) {
        self.tap = true;
    }

    /// Another key or a mouse button went down: Alt is a modifier now, not a tap.
    pub fn other_down(&mut self) {
        self.tap = false;
    }

    /// Alt came up. True when it was a tap and the bar was shown or hidden by it. With
    /// `allowed` false (the Esc menu is open) the tap ends and does nothing.
    pub fn alt_up(&mut self, allowed: bool) -> bool {
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
