//! The pages behind the Esc menu's lines in the new interface: the settings windows
//! (Options, Controls, Vehicle options, World options, the keyboard's and the game
//! controllers'), the lists (line and tour, drivers, fleet numbers, destinations, depot files,
//! start points, placing a vehicle, the administration) and their drop-downs.
//!
//! The pages stay the HUD's own: what they show is the HUD's frame (`ui::Frame`, made from the
//! game's lists as before), and what a key or a click does is the game's code as it was. This
//! module only draws them, with `omsi_ui::menu_page`, in the room right of the Esc menu's
//! column (still drawn, its line of the page marked), and hands the HUD the rectangles the
//! page was drawn in: the HUD's mouse code hit-tests those as it did its own (`hud::Hits`:
//! `menu_rects`, `menu_ctl`, `menu_side`, `dd_rects` ...), so nothing of how the pages work
//! changes. On a triple screen the page stands on the centre screen with the column, and the
//! rectangles are where it is drawn there; the headset shows the window's picture of it (see
//! `new_ui`).

use crate::game_lists::ListKind;
use crate::hud::{Frame, Hits, MenuKind};
use crate::new_ui::{Area, MenuFrame};
use glam::Vec2;
use omsi_ui::menu_page::{self, Back, Control, Dropdown, Layout, Lead, Page, Pane, Row};
use omsi_ui::pause_menu;
use omsi_ui::widgets::Ui;
use omsi_ui::{Color, Rect};

/// What a row has at its right, owned (see `menu_page::Control`).
#[derive(Clone, Debug, PartialEq)]
pub enum Ctl {
    None,
    Switch(bool),
    Slider(f32, String),
    Stepper(String),
    Opens(String),
    Info(String),
    Editing(String),
    Button(String),
}

/// What stands before a list line's text, owned (see `menu_page::Lead`).
#[derive(Clone, Debug, PartialEq)]
pub enum Before {
    None,
    Sign(String),
    Code(String),
}

/// One row of a page, made from a line of the HUD's list.
#[derive(Clone, Debug, PartialEq)]
pub struct PageRow {
    pub label: String,
    pub desc: String,
    pub ctl: Ctl,
    pub lead: Before,
    pub heading: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct PagePane {
    title: String,
    meta: String,
    rows: Vec<(String, String)>,
    chosen: Option<usize>,
    button: Option<String>,
    time: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
struct PageDropdown {
    row: usize,
    items: Vec<String>,
    sel: usize,
    top: usize,
    current: Option<usize>,
}

/// What one frame of an open page shows, owned so that it can cross to the renderer.
#[derive(Clone, Debug, PartialEq)]
pub struct PageFrame {
    /// The Esc menu's column beside the page, and the bar over it.
    pub column: MenuFrame,
    pub title: String,
    pub note: String,
    tabs_title: String,
    tabs: Vec<String>,
    tab: usize,
    pub rows: Vec<PageRow>,
    pub fields: bool,
    pub selected: usize,
    kbd: bool,
    pub active: Option<usize>,
    top: Option<f32>,
    pub back: Back,
    pane: Option<PagePane>,
    pane_first: Option<usize>,
    dropdown: Option<PageDropdown>,
}

/// A list line `"<code>  <name>"` (a number, maybe padded in front, two spaces, a name): its
/// code and name (as the HUD's list reads the destinations).
fn code_and_name(label: &str) -> Option<(&str, &str)> {
    let (code, name) = label.trim_start().split_once("  ")?;
    (!code.is_empty() && code.chars().all(|c| c.is_ascii_digit()) && !name.trim().is_empty())
        .then(|| (code, name.trim()))
}

/// The row of the page for the HUD list's line (`id`: what it does, `label`: what it says) in a
/// list of `kind`. A settings line is `name\u{1f}kind\u{1f}value\u{1f}description\u{1f}fraction`
/// (`game_lists::row`): `s` a switch, `v` a slider, `c` a stepper, `o` opens a list or a
/// drop-down, `i` information, `E` a value being typed, anything else a button. A list line is
/// its text, a line ending in dots opens another list.
pub fn row_of(id: &str, label: &str, kind: MenuKind) -> PageRow {
    let mut row = PageRow {
        label: String::new(),
        desc: String::new(),
        ctl: Ctl::None,
        lead: Before::None,
        heading: false,
    };
    if id == crate::game_lists::HEADING {
        row.label = label.split('\u{1f}').next().unwrap_or("").to_string();
        row.heading = true;
        return row;
    }
    if label.contains('\u{1f}') {
        let mut parts = label.split('\u{1f}');
        row.label = parts.next().unwrap_or("").to_string();
        let k = parts.next().unwrap_or("a");
        let value = parts.next().unwrap_or("").to_string();
        row.desc = parts.next().unwrap_or("").to_string();
        let frac: Option<f32> = parts.next().and_then(|p| p.parse().ok());
        row.ctl = match k {
            "s" => Ctl::Switch(value == "on"),
            "v" => Ctl::Slider(frac.unwrap_or(0.0).clamp(0.0, 1.0), value),
            "c" => Ctl::Stepper(value),
            "o" => Ctl::Opens(value),
            "i" => Ctl::Info(value),
            "E" => Ctl::Editing(value),
            _ => Ctl::Button(value),
        };
        return row;
    }
    let line = format!("{} ", omsi_ui::tr("Line"));
    let tour = format!("{} ", omsi_ui::tr("Tour"));
    match kind {
        // a line of the timetable: its number on a sign, how many tours it has after it
        MenuKind::Lines => {
            if let Some((name, info)) = label
                .strip_prefix(line.as_str())
                .and_then(|rest| rest.rsplit_once("  ("))
            {
                row.lead = Before::Sign(name.to_string());
                row.label = info.trim_end_matches(')').to_string();
                row.ctl = Ctl::Opens(String::new());
                return row;
            }
        }
        // a tour: its name (the time is chosen beside the list)
        MenuKind::Tours => {
            if let Some(rest) = label.strip_prefix(tour.as_str()) {
                let num = rest.split_once("  ").map(|(n, _)| n).unwrap_or(rest).trim();
                row.label = format!("{tour}{num}");
                return row;
            }
        }
        MenuKind::List => {
            if let Some((code, name)) = code_and_name(label) {
                row.lead = Before::Code(code.to_string());
                row.label = name.to_string();
                return row;
            }
        }
        _ => {}
    }
    match label
        .strip_suffix("...")
        .or_else(|| label.strip_suffix('…'))
    {
        Some(t) => {
            row.label = t.trim_end().to_string();
            row.ctl = Ctl::Opens(String::new());
        }
        None => row.label = label.to_string(),
    }
    row
}

/// The lines of the Esc menu a list of `kind` is behind (the first the menu has is marked):
/// the page "Camera" of the options has a line of its own.
fn column_lines(kind: Option<&ListKind>, tab: Option<&str>) -> &'static [&'static str] {
    match kind {
        Some(ListKind::Options(_)) if tab == Some("Camera") => &["camera", "options"],
        Some(ListKind::Options(_)) => &["options"],
        Some(
            ListKind::Controls
            | ListKind::Keyboard(_)
            | ListKind::Events
            | ListKind::ControllerDevices(_)
            | ListKind::Controller(..)
            | ListKind::ControllerAxis(..)
            | ListKind::ControllerButtons(_)
            | ListKind::ControllerButtonSettings(..)
            | ListKind::ControllerButton(..)
            | ListKind::ControllerCapture(_),
        ) => &["controls"],
        Some(
            ListKind::Vehicle(_)
            | ListKind::Drivers
            | ListKind::Numbers
            | ListKind::Destinations
            | ListKind::RouteNumbers
            | ListKind::Hofs
            | ListKind::Spots
            | ListKind::PlaceMaker
            | ListKind::PlaceType(_)
            | ListKind::PlaceLivery(_)
            | ListKind::PlaceHof(..),
        ) => &["vehicle"],
        Some(ListKind::World(_)) => &["world"],
        Some(ListKind::Lines | ListKind::Tours(..)) => &["duty"],
        Some(ListKind::Admin) => &["admin"],
        None => &[],
    }
}

/// The page for the HUD's frame `f` while a list of `kind` is open (`None` when no list is),
/// beside the Esc menu's `column`.
pub fn page_frame(f: &Frame, kind: Option<&ListKind>, column: MenuFrame) -> Option<PageFrame> {
    let (sel, items) = f.menu?;
    let back_txt = omsi_ui::tr("Back").into_owned();
    let fields = f.menu_kind == MenuKind::Options;
    let list = matches!(
        f.menu_kind,
        MenuKind::Lines | MenuKind::Tours | MenuKind::List
    );
    // ("Back" of the lists stands alone under them, as on the HUD's card)
    let footer = list
        && items
            .last()
            .is_some_and(|&(id, l)| id == "back" && l == back_txt.as_str());
    let rows: Vec<PageRow> = items
        .iter()
        .map(|&(id, label)| row_of(id, label, f.menu_kind))
        .collect();
    let (head, sub) = f.menu_head.clone().unwrap_or_else(|| {
        let t = if fields {
            "Options"
        } else if f.paused {
            "Paused"
        } else {
            "Menu"
        };
        (t.to_string(), String::new())
    });
    let (tabs, tab) = f.menu_tabs.clone().unwrap_or_default();
    let tab_title = tabs.get(tab).cloned();
    // (the way back of a settings window is at the foot of its pages, as on the HUD's sidebar)
    let back = if footer {
        Back::Footer
    } else if fields && !tabs.is_empty() {
        Back::Side
    } else {
        Back::None
    };
    let title = match (&tab_title, fields) {
        (Some(t), true) => t.clone(),
        _ => head.clone(),
    };
    let subtitle = match &tab_title {
        Some(t) if fields && tabs.len() > 1 => t.clone(),
        _ => sub.clone(),
    };
    let back_row = footer.then(|| items.len() - 1);
    let active = (f.menu_kind == MenuKind::Tours && back_row != Some(sel)).then_some(sel);
    let pane = f.menu_preview.as_ref().map(|p| PagePane {
        title: p.title.clone(),
        meta: p.meta.clone(),
        rows: p.rows.clone(),
        chosen: p.chosen,
        button: p.button.clone(),
        time: p.time.clone(),
    });
    let dropdown = f.dropdown.as_ref().map(|d| PageDropdown {
        row: d.row,
        items: d.items.iter().map(|s| s.to_string()).collect(),
        sel: d.sel,
        top: d.top,
        current: d.current,
    });
    let column = column.for_page(
        column_lines(kind, tab_title.as_deref()),
        head.clone(),
        subtitle,
    );
    Some(PageFrame {
        column,
        note: if fields { String::new() } else { sub },
        title,
        tabs_title: head,
        tabs,
        tab,
        rows,
        fields,
        selected: sel,
        kbd: f.menu_kbd,
        active,
        top: f.menu_top,
        back,
        pane,
        pane_first: f.pane_first,
        dropdown,
    })
}

fn control(c: &Ctl) -> Control<'_> {
    match c {
        Ctl::None => Control::None,
        Ctl::Switch(on) => Control::Switch(*on),
        Ctl::Slider(frac, value) => Control::Slider { frac: *frac, value },
        Ctl::Stepper(v) => Control::Stepper(v),
        Ctl::Opens(v) => Control::Opens(v),
        Ctl::Info(v) => Control::Info(v),
        Ctl::Editing(v) => Control::Editing(v),
        Ctl::Button(v) => Control::Button(v),
    }
}

/// `r` (points, at `s` pixels each, in an area whose corner is at `o`) as the HUD's
/// rectangles are: `[x0, y0, x1, y1]` in window pixels.
pub fn px(r: Rect, s: f32, o: Vec2) -> [f32; 4] {
    [
        o.x + r.x * s,
        o.y + r.y * s,
        o.x + r.right() * s,
        o.y + r.bottom() * s,
    ]
}

impl PageFrame {
    pub(crate) fn with_page<R>(&self, f: impl FnOnce(&Page) -> R) -> R {
        let rows: Vec<Row> = self
            .rows
            .iter()
            .map(|r| Row {
                label: &r.label,
                desc: &r.desc,
                control: control(&r.ctl),
                lead: match &r.lead {
                    Before::None => Lead::None,
                    Before::Sign(s) => Lead::Sign(s),
                    Before::Code(s) => Lead::Code(s),
                },
                heading: r.heading,
            })
            .collect();
        let tabs: Vec<&str> = self.tabs.iter().map(String::as_str).collect();
        let pane_rows: Vec<(&str, &str)> = self
            .pane
            .iter()
            .flat_map(|p| p.rows.iter().map(|(a, b)| (a.as_str(), b.as_str())))
            .collect();
        let dd_items: Vec<&str> = self
            .dropdown
            .iter()
            .flat_map(|d| d.items.iter().map(String::as_str))
            .collect();
        let page = Page {
            title: &self.title,
            note: &self.note,
            tabs_title: &self.tabs_title,
            tabs: &tabs,
            tab: self.tab,
            rows: &rows,
            fields: self.fields,
            selected: self.selected,
            kbd: self.kbd,
            active: self.active,
            top: self.top,
            back: self.back,
            pane: self.pane.as_ref().map(|p| Pane {
                title: &p.title,
                meta: &p.meta,
                rows: &pane_rows,
                chosen: p.chosen,
                button: p.button.as_deref(),
                time: p.time.as_deref(),
            }),
            pane_first: self.pane_first,
            dropdown: self.dropdown.as_ref().map(|d| Dropdown {
                row: d.row,
                items: &dd_items,
                sel: d.sel,
                top: d.top,
                current: d.current,
            }),
        };
        f(&page)
    }

    /// Pixels per design point in a window of `size` pixels: the column's.
    pub(crate) fn scale(&self, size: (u32, u32)) -> f32 {
        self.column.scale(size)
    }

    /// The room of the page in a window of `pts` points: right of the column, under the bar.
    fn area(pts: Vec2) -> Rect {
        Rect::new(
            pause_menu::COLUMN_W,
            pause_menu::BAR_H,
            (pts.x - pause_menu::COLUMN_W).max(1.0),
            (pts.y - pause_menu::BAR_H).max(1.0),
        )
    }

    /// Where the page's parts are in a window of `size` pixels (in points), and the scale.
    pub fn layout(&self, size: (u32, u32)) -> (f32, Layout) {
        let s = self.scale(size);
        let pts = Vec2::new(size.0 as f32, size.1 as f32) / s;
        (s, self.with_page(|p| menu_page::layout(p, Self::area(pts))))
    }

    /// Hand the HUD the rectangles the page is drawn in, as its own list would have set them,
    /// for its mouse code (a row, a slider's track, a page, the way back, a drop-down's entry,
    /// the timetable's stops, its arrows and its button, the scroll bars): in window pixels,
    /// the page laid out in `area` (the centre screen of a triple screen, else the window).
    pub fn write_hits(&self, hits: &mut Hits, area: Area) {
        let (s, l) = self.layout(area.size);
        let o = area.origin;
        let px = |r: Rect| px(r, s, o);
        hits.menu_rects = l.rows.iter().map(|r| px(*r)).collect();
        hits.menu_ctl = l.controls.iter().map(|c| c.map(px)).collect();
        hits.menu_arrows.clear();
        hits.menu_side = l.tabs.iter().map(|r| px(*r)).collect();
        hits.menu_start = l.start;
        hits.menu_rows = l.shown;
        hits.menu_row_h = l.row_h * s;
        hits.menu_scroll_track = l.scroll.map(|(t, _)| px(t));
        hits.menu_scroll_thumb = l.scroll.map(|(_, g)| px(g));
        hits.menu_pane = l.pane_rows.iter().map(|r| px(*r)).collect();
        hits.menu_pane_start = l.pane_first;
        hits.menu_pane_go = l.pane_go.map(px);
        hits.menu_pane_box = l.pane_card.filter(|_| l.pane_go.is_some()).map(px);
        hits.menu_pane_scroll = l.pane_scroll.map(|(t, g, n, fit)| (px(t), px(g), n, fit));
        hits.menu_time = l.time.iter().map(|r| px(*r)).collect();
        hits.dd_rects = l.dd.iter().map(|r| px(*r)).collect();
        hits.dd_top = l.dd_top;
        if l.dd_panel.is_some() {
            hits.dd_rows = l.dd_rows;
        }
        hits.dd_scroll = l.dd_scroll.map(|(t, g)| (px(t), px(g)));
    }

    /// Draw the page over a window of `size` points: the world dimmed, the column and its bar,
    /// the page right of them.
    pub(crate) fn draw(&self, ui: &mut Ui, size: Vec2, logo: Option<(usize, f32)>) {
        ui.p().rect(
            Rect::new(0.0, 0.0, size.x, size.y),
            Color::hex(0x000000).alpha(0.35),
        );
        self.column.with_view(
            |v| {
                pause_menu::draw(ui, v);
            },
            logo,
        );
        let area = Self::area(size);
        // (the room of the page a little darker still: the cards stand out of the world)
        ui.p().rect(area, Color::hex(0x000000).alpha(0.25));
        self.with_page(|p| {
            menu_page::draw(ui, p, area);
        });
    }
}

impl crate::app::App {
    /// Whether the new interface draws the open list (a page behind the Esc menu): a list is
    /// open from the Esc menu.
    pub(crate) fn new_pages_active(&self) -> bool {
        showing(self.game_menu.is_some(), self.chooser.is_some()) == Showing::Page
    }
}

/// What the new interface draws of the game menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Showing {
    /// Nothing: no menu (or a list opened with no menu, the HUD's).
    None,
    /// The Esc menu's top level.
    Top,
    /// A page behind one of its lines.
    Page,
}

/// What shows with the game menu open or not (`menu`) and a list open or not (`chooser`), in
/// the window, on a triple screen and in the headset alike. A list opened with no Esc menu (a
/// plugin's command) stays the HUD's: the page's clicks are the menu's.
pub fn showing(menu: bool, chooser: bool) -> Showing {
    match (menu, chooser) {
        (true, true) => Showing::Page,
        (true, false) => Showing::Top,
        (false, _) => Showing::None,
    }
}
