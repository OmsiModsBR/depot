//! The Esc menu's column: the sections and lines it shows, where it stands in the window
//! and how the keys walk it, with no drawing.

use glam::Vec2;
use omsi_ui::pause_menu::Kind;

/// The design the menu is made for, in points (the same as the launcher's).
pub(in crate::ui) const DESIGN: (f32, f32) = (1672.0, 941.0);

pub(in crate::ui) const MIN_SCALE: f32 = 0.75;

/// Pixels per design point for a window of `w` x `h` pixels.
pub fn scale_for(w: u32, h: u32) -> f32 {
    (w as f32 / DESIGN.0)
        .min(h as f32 / DESIGN.1)
        .max(MIN_SCALE)
}

/// Where the new interface is drawn in the window, in pixels: the whole window, or the
/// centre screen of a triple screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
    /// Its top-left corner in the window.
    pub origin: Vec2,
    pub size: (u32, u32),
}

impl Area {
    /// The whole of a window of `size` pixels.
    pub fn whole(size: (u32, u32)) -> Area {
        Area {
            origin: Vec2::ZERO,
            size,
        }
    }

    /// Window pixels `at` as pixels of the area.
    pub fn local(&self, at: Vec2) -> Vec2 {
        at - self.origin
    }

    /// The area where it lies inside a picture of `size` pixels; else (another picture) the
    /// whole of it.
    pub fn within(self, size: (u32, u32)) -> Area {
        let fits = self.size.0 > 0
            && self.size.1 > 0
            && self.origin.x >= 0.0
            && self.origin.y >= 0.0
            && self.origin.x + self.size.0 as f32 <= size.0 as f32
            && self.origin.y + self.size.1 as f32 <= size.1 as f32;
        if fits {
            self
        } else {
            Area::whole(size)
        }
    }
}

/// Where the menus are drawn in a window of `size` pixels: on a triple screen (`triple`, not
/// in the headset: `vr`) the centre screen, as the renderer lays the panels out
/// (`omsi_render::panel_width`) - the menus are made for one screen's shape, so they stand
/// there whether or not the rest of the HUD spans the three; else the whole window (the
/// headset's menu plane shows the whole window's picture).
pub fn menu_area(triple: bool, vr: bool, size: (u32, u32)) -> Area {
    if triple && !vr && size.0 >= 3 {
        let w = omsi_render::panel_width(size.0);
        Area {
            origin: Vec2::new(w as f32, 0.0),
            size: (w, size.1),
        }
    } else {
        Area::whole(size)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct OwnedItem {
    pub id: String,
    pub(in crate::ui) icon: &'static str,
    pub(in crate::ui) label: String,
    pub(in crate::ui) key: &'static str,
    pub kind: Kind,
    pub(in crate::ui) disabled: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OwnedSection {
    pub(in crate::ui) title: &'static str,
    pub items: Vec<OwnedItem>,
}

/// What one frame of the menu shows, owned so that it can cross to the renderer.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuFrame {
    pub sections: Vec<OwnedSection>,
    pub title: String,
    pub subtitle: String,
    pub(in crate::ui) clock: String,
    pub state: &'static str,
    pub selected: Option<String>,
}

/// Where a game menu line goes in the column, its icon, the key that does the same, and how it
/// is drawn. A line not listed here is shown under "Game" with a plain icon, so that a line
/// added to the game's menu is never lost.
pub fn place(id: &str) -> (usize, &'static str, &'static str, Kind) {
    match id {
        "resume" => (0, "play_arrow", "Esc", Kind::Primary),
        "tobus" => (0, "directions_bus", "", Kind::Normal),
        "duty" => (0, "route", "", Kind::Normal),
        "skipstop" => (0, "turn_slight_right", "", Kind::Normal),
        "endduty" => (0, "stop_circle", "", Kind::Normal),
        "options" => (1, "settings", "", Kind::Normal),
        "controls" => (1, "keyboard", "", Kind::Normal),
        "camera" => (1, "videocam", "", Kind::Normal),
        "vehicle" => (1, "directions_bus", "", Kind::Normal),
        "world" => (1, "cloud", "", Kind::Normal),
        "map" => (1, "map", "", Kind::Normal),
        "copycode" => (1, "content_copy", "", Kind::Normal),
        "admin" => (1, "settings", "", Kind::Normal),
        "save" => (2, "save", "", Kind::Normal),
        "saveslot" => (2, "download", "", Kind::Normal),
        "load" => (2, "upload", "", Kind::Normal),
        "shot" => (2, "photo_camera", "", Kind::Normal),
        "quit" => (2, "power_settings_new", "", Kind::Danger),
        _ => (2, "settings", "", Kind::Normal),
    }
}

const SECTIONS: [&str; 3] = ["Session", "Settings", "Game"];

/// The menu for the game menu's lines `items` ((what, label), as `game_menu_items` gives
/// them), the line `selected` on the keyboard (none while the mouse leads), and `off(k)` for a greyed-out line `k`.
pub fn menu_frame(
    items: &[(&str, &str)],
    selected: Option<usize>,
    off: &dyn Fn(usize) -> bool,
    clock: String,
    paused: bool,
) -> MenuFrame {
    let mut sections: Vec<OwnedSection> = SECTIONS
        .iter()
        .map(|t| OwnedSection {
            title: t,
            items: Vec::new(),
        })
        .collect();
    for (k, (id, label)) in items.iter().enumerate() {
        let (at, icon, key, kind) = place(id);
        sections[at].items.push(OwnedItem {
            id: (*id).to_string(),
            icon,
            // (English: the widgets translate what they draw, a second time would garble it)
            label: (*label).to_string(),
            key,
            kind,
            disabled: off(k).then(|| "Not available now".to_string()),
        });
    }
    sections.retain(|s| !s.items.is_empty());
    MenuFrame {
        sections,
        title: "Game menu".to_string(),
        subtitle: String::new(),
        clock,
        state: if paused { "paused" } else { "menu open" },
        selected: selected.and_then(|k| items.get(k)).map(|m| m.0.to_string()),
    }
}

/// The game menu's lines in the order the column shows them (section by section, each in the
/// game's own order), as indices into `items`.
pub fn display_order(items: &[(&str, &str)]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by_key(|&k| place(items[k].0).0);
    order
}

/// The line after (or before) `from` in the displayed order, skipping greyed-out ones.
pub fn step_displayed(
    items: &[(&str, &str)],
    from: usize,
    down: bool,
    off: &dyn Fn(usize) -> bool,
) -> usize {
    let order = display_order(items);
    let n = order.len();
    let Some(at) = order.iter().position(|&k| k == from) else {
        return from;
    };
    for i in 1..=n {
        let next = if down {
            (at + i) % n
        } else {
            (at + n - i % n) % n
        };
        if !off(order[next]) {
            return order[next];
        }
    }
    from
}
