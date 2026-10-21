//! The pages behind the Esc menu's lines in the new interface: the settings windows
//! (Options, Controls, Vehicle options, World options, the keyboard's and the game
//! controllers'), the lists (line and tour, drivers, fleet numbers, destinations, depot files,
//! start points, placing a vehicle, the administration) and their drop-downs.
//!
//! The pages stay the HUD's own: what they show is the HUD's frame (`ui::Frame`, made from the
//! game's lists as before), and what a key or a click does is the game's code as it was. This
//! module only draws them, with `omsi_ui::menu_page`, in the room right of the Esc menu's
//! column (still drawn, its line of the page marked), and hands the HUD the rectangles the
//! page was drawn in: the HUD's mouse code hit-tests those as it did its own (`ui::Ui`'s
//! `menu_rects`, `menu_ctl`, `menu_side`, `dd_rects` ...), so nothing of how the pages work
//! changes. On a triple screen the page stands on the centre screen with the column, and the
//! rectangles are where it is drawn there; the headset shows the window's picture of it (see
//! `new_ui`).

use crate::game_lists::ListKind;
use crate::new_ui::{Area, MenuFrame};
use crate::ui::{Frame, MenuKind};
use glam::Vec2;
use omsi_ui::menu_page::{self, Back, Control, Dropdown, Layout, Lead, Page, Pane, Row};
use omsi_ui::pause_menu;
use omsi_ui::widgets::Ui;
use omsi_ui::{Color, Rect};

/// What a row has at its right, owned (see `menu_page::Control`).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Ctl {
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
pub(crate) enum Before {
    None,
    Sign(String),
    Code(String),
}

/// One row of a page, made from a line of the HUD's list.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PageRow {
    pub(crate) label: String,
    pub(crate) desc: String,
    pub(crate) ctl: Ctl,
    pub(crate) lead: Before,
    pub(crate) heading: bool,
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
pub(crate) struct PageFrame {
    /// The Esc menu's column beside the page, and the bar over it.
    pub(crate) column: MenuFrame,
    title: String,
    note: String,
    tabs_title: String,
    tabs: Vec<String>,
    tab: usize,
    rows: Vec<PageRow>,
    fields: bool,
    selected: usize,
    kbd: bool,
    active: Option<usize>,
    top: Option<f32>,
    back: Back,
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
pub(crate) fn row_of(id: &str, label: &str, kind: MenuKind) -> PageRow {
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
pub(crate) fn page_frame(
    f: &Frame,
    kind: Option<&ListKind>,
    column: MenuFrame,
) -> Option<PageFrame> {
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
fn px(r: Rect, s: f32, o: Vec2) -> [f32; 4] {
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
    pub(crate) fn layout(&self, size: (u32, u32)) -> (f32, Layout) {
        let s = self.scale(size);
        let pts = Vec2::new(size.0 as f32, size.1 as f32) / s;
        (s, self.with_page(|p| menu_page::layout(p, Self::area(pts))))
    }

    /// Hand the HUD the rectangles the page is drawn in, as its own list would have set them,
    /// for its mouse code (a row, a slider's track, a page, the way back, a drop-down's entry,
    /// the timetable's stops, its arrows and its button, the scroll bars): in window pixels,
    /// the page laid out in `area` (the centre screen of a triple screen, else the window).
    pub(crate) fn write_hits(&self, ui: &mut crate::ui::Ui, area: Area) {
        let (s, l) = self.layout(area.size);
        let o = area.origin;
        let px = |r: Rect| px(r, s, o);
        ui.menu_rects = l.rows.iter().map(|r| px(*r)).collect();
        ui.menu_ctl = l.controls.iter().map(|c| c.map(px)).collect();
        ui.menu_arrows.clear();
        ui.menu_side = l.tabs.iter().map(|r| px(*r)).collect();
        ui.menu_start = l.start;
        ui.menu_rows = l.shown;
        ui.menu_row_h = l.row_h * s;
        ui.menu_scroll_track = l.scroll.map(|(t, _)| px(t));
        ui.menu_scroll_thumb = l.scroll.map(|(_, g)| px(g));
        ui.menu_pane = l.pane_rows.iter().map(|r| px(*r)).collect();
        ui.menu_pane_start = l.pane_first;
        ui.menu_pane_go = l.pane_go.map(px);
        ui.menu_pane_box = l.pane_card.filter(|_| l.pane_go.is_some()).map(px);
        ui.menu_pane_scroll = l.pane_scroll.map(|(t, g, n, fit)| (px(t), px(g), n, fit));
        ui.menu_time = l.time.iter().map(|r| px(*r)).collect();
        ui.dd_rects = l.dd.iter().map(|r| px(*r)).collect();
        ui.dd_top = l.dd_top;
        if l.dd_panel.is_some() {
            ui.dd_rows = l.dd_rows;
        }
        ui.dd_scroll = l.dd_scroll.map(|(t, g)| (px(t), px(g)));
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
pub(crate) enum Showing {
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
pub(crate) fn showing(menu: bool, chooser: bool) -> Showing {
    match (menu, chooser) {
        (true, true) => Showing::Page,
        (true, false) => Showing::Top,
        (false, _) => Showing::None,
    }
}

/// The Esc menu's column of a session with every line (tests).
#[cfg(test)]
pub(crate) fn test_column(paused: bool) -> MenuFrame {
    crate::new_ui::menu_frame(
        &crate::input::menus::GAME_MENU,
        Some(0),
        &|_| false,
        "08:42".into(),
        paused,
    )
}

/// The HUD's frame with the list `items` of `kind` open on its first line (tests).
#[cfg(test)]
pub(crate) fn test_frame<'a>(items: &'a [(&'a str, &'a str)], kind: MenuKind) -> Frame<'a> {
    Frame {
        scale: 1.0,
        ui_scale: 1.0,
        opacity: 1.0,
        width: 1672.0,
        height: 941.0,
        cursor: (0.0, 0.0),
        vr: false,
        tooltip: None,
        chat: None,
        chat_size: 1.0,
        notes: &[],
        fps: None,
        paused: true,
        menu: Some((0, items)),
        menu_top: None,
        timetable: None,
        info: None,
        info_room: None,
        tutorial: None,
        tags: Vec::new(),
        notices: &[],
        notice_anchor: None,
        menu_kind: kind,
        menu_head: Some(("Options".into(), String::new())),
        menu_preview: None,
        pane_first: None,
        menu_tabs: None,
        menu_kbd: true,
        dropdown: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_lists::row;
    use crate::ui::{DropdownView, Preview};

    #[test]
    fn every_kind_of_settings_row_maps_to_its_control() {
        let o = MenuKind::Options;
        let r = row_of(
            "fullscreen",
            &row("Fullscreen", 's', "on", "Window or screen", None),
            o,
        );
        assert_eq!(
            (r.label.as_str(), r.desc.as_str()),
            ("Fullscreen", "Window or screen")
        );
        assert_eq!(r.ctl, Ctl::Switch(true));
        assert_eq!(
            row_of("x", &row("A", 's', "off", "", None), o).ctl,
            Ctl::Switch(false)
        );
        assert_eq!(
            row_of("volume", &row("Volume", 'v', "60 %", "", Some(0.6)), o).ctl,
            Ctl::Slider(0.6, "60 %".into())
        );
        assert_eq!(
            row_of("x", &row("A", 'c', "Medium", "", None), o).ctl,
            Ctl::Stepper("Medium".into())
        );
        assert_eq!(
            row_of("preset", &row("Preset", 'o', "High", "", None), o).ctl,
            Ctl::Opens("High".into())
        );
        assert_eq!(
            row_of("keyboard", &row("Keyboard", 'o', "", "", None), o).ctl,
            Ctl::Opens(String::new())
        );
        assert_eq!(
            row_of("noop", &row("Connection", 'i', "Connected", "", None), o).ctl,
            Ctl::Info("Connected".into())
        );
        assert_eq!(
            row_of("time_edit", &row("Exact time", 'E', "0842", "", None), o).ctl,
            Ctl::Editing("0842".into())
        );
        assert_eq!(
            row_of(
                "time_edit",
                &row("Exact time", 'e', "08:42:00", "", None),
                o
            )
            .ctl,
            Ctl::Button("08:42:00".into())
        );
        assert_eq!(
            row_of("keybind v 0", &row("Horn", 'a', "H", "", None), o).ctl,
            Ctl::Button("H".into())
        );
        // a heading: never chosen, its label alone
        let h = row_of(crate::game_lists::HEADING, "Steering", o);
        assert!(h.heading);
        assert_eq!(h.label, "Steering");
        assert!(!r.heading);
    }

    #[test]
    fn list_lines_keep_their_text_and_get_their_sign_code_or_arrow() {
        let l = row_of("line 50", "Line 50  (12 tours)", MenuKind::Lines);
        assert_eq!(l.lead, Before::Sign("50".into()));
        assert_eq!(l.label, "12 tours");
        assert_eq!(l.ctl, Ctl::Opens(String::new()));
        let t = row_of("tour 50\u{1}3", "Tour 3", MenuKind::Tours);
        assert_eq!(t.label, "Tour 3");
        let d = row_of("dest 4", " 12  Central Station", MenuKind::List);
        assert_eq!(d.lead, Before::Code("12".into()));
        assert_eq!(d.label, "Central Station");
        let m = row_of("routes", "Route number: 50...", MenuKind::List);
        assert_eq!(m.label, "Route number: 50");
        assert_eq!(m.ctl, Ctl::Opens(String::new()));
        let p = row_of("driver Ana", "Ana  (now)", MenuKind::List);
        assert_eq!(
            (p.label.as_str(), &p.ctl, &p.lead),
            ("Ana  (now)", &Ctl::None, &Before::None)
        );
    }

    #[test]
    fn a_settings_window_has_its_pages_its_rows_and_the_way_back_at_their_foot() {
        let a = row("Fullscreen", 's', "on", "", None);
        let b = row("Volume", 'v', "60 %", "", Some(0.6));
        let items = [("fullscreen", a.as_str()), ("volume", b.as_str())];
        let mut f = test_frame(&items, MenuKind::Options);
        f.menu = Some((1, &items));
        f.menu_tabs = Some((
            vec!["General".into(), "Graphics".into(), "Camera".into()],
            2,
        ));
        let p = page_frame(&f, Some(&ListKind::Options(2)), test_column(true)).unwrap();
        assert!(p.fields);
        assert_eq!(p.rows.len(), 2);
        assert_eq!(p.selected, 1);
        assert_eq!(p.back, Back::Side);
        assert_eq!(p.title, "Camera");
        // the column marks the line the page is behind: "Camera...", which has its own
        assert_eq!(p.column.current(), Some("camera"));
        assert_eq!(p.column.bar(), ("Options", "Camera", "paused"));
        // the rectangles the HUD hit-tests: a row per line, the pages and the way back last
        let (s, l) = p.layout((1672, 941));
        assert_eq!(s, 1.0);
        assert_eq!(l.rows.len(), 2);
        assert_eq!(l.tabs.len(), 4);
        assert!(l.controls[0].is_none() && l.controls[1].is_some());
        // another page of the options: "Options..." is the line marked
        let p = page_frame(&f, Some(&ListKind::Options(0)), test_column(true));
        let mut f2 = test_frame(&items, MenuKind::Options);
        f2.menu_tabs = Some((vec!["General".into()], 0));
        let p2 = page_frame(&f2, Some(&ListKind::Options(0)), test_column(true)).unwrap();
        assert!(p.is_some());
        assert_eq!(p2.column.current(), Some("options"));
    }

    #[test]
    fn a_list_has_the_way_back_under_it_and_its_tour_and_timetable_marked() {
        let items = [
            ("tour 50\u{1}1", "Tour 1"),
            ("tour 50\u{1}2", "Tour 2"),
            ("back", "Back"),
        ];
        let mut f = test_frame(&items, MenuKind::Tours);
        f.menu = Some((1, &items));
        f.menu_head = Some(("Line and tour".into(), "Line 50".into()));
        f.menu_preview = Some(Preview {
            title: "Tour 2".into(),
            meta: "Line 50".into(),
            rows: vec![
                ("Depot".into(), "08:00".into()),
                ("Market".into(), "08:10".into()),
            ],
            chosen: Some(1),
            button: Some("Start trip".into()),
            time: Some("08:00".into()),
        });
        let p = page_frame(
            &f,
            Some(&ListKind::Tours("50".into(), None)),
            test_column(true),
        )
        .unwrap();
        assert_eq!(p.back, Back::Footer);
        assert_eq!(p.active, Some(1));
        assert_eq!(p.note, "Line 50");
        assert_eq!(p.column.current(), Some("duty"));
        let mut ui = crate::ui::Ui::new().unwrap();
        p.write_hits(&mut ui, Area::whole((1672, 941)));
        // the way back at its row's index, the stops, their button and the arrows of the time
        assert_eq!(ui.menu_rects.len(), 3);
        assert_eq!(ui.menu_pane.len(), 2);
        assert!(ui.menu_pane_go.is_some() && ui.menu_pane_box.is_some());
        assert_eq!(ui.menu_time.len(), 2);
        assert!(ui.menu_side.is_empty());
        // the way back chosen is not a tour marked
        f.menu = Some((2, &items));
        let p = page_frame(
            &f,
            Some(&ListKind::Tours("50".into(), None)),
            test_column(true),
        )
        .unwrap();
        assert_eq!(p.active, None);
    }

    #[test]
    fn the_hud_gets_the_rectangles_in_window_pixels() {
        let a = row("Preset", 'o', "High", "", None);
        let items = [("preset", a.as_str())];
        let mut f = test_frame(&items, MenuKind::Options);
        f.menu_tabs = Some((vec!["Graphics".into()], 0));
        let names = ["Low", "Medium", "High"];
        f.dropdown = Some(DropdownView {
            row: 0,
            items: names.to_vec(),
            sel: 2,
            top: 0,
            current: Some(2),
        });
        let p = page_frame(&f, Some(&ListKind::Options(0)), test_column(true)).unwrap();
        for size in [(1672u32, 941u32), (1920, 1080), (3840, 2160)] {
            let mut ui = crate::ui::Ui::new().unwrap();
            p.write_hits(&mut ui, Area::whole(size));
            let (s, l) = p.layout(size);
            assert_eq!(ui.menu_rects[0], px(l.rows[0], s, Vec2::ZERO), "{size:?}");
            assert_eq!(ui.dd_rects.len(), 3, "{size:?}");
            assert_eq!(ui.dd_rows, 3);
            // (all inside the window, right of the column)
            for r in ui
                .menu_rects
                .iter()
                .chain(&ui.menu_side)
                .chain(&ui.dd_rects)
            {
                assert!(
                    r[0] >= pause_menu::COLUMN_W * s
                        && r[2] <= size.0 as f32
                        && r[3] <= size.1 as f32,
                    "{size:?}: {r:?}"
                );
            }
        }
    }

    #[test]
    fn on_a_triple_screen_the_page_is_clicked_where_the_centre_screen_shows_it() {
        let items = [("a", "One"), ("b", "Two"), ("back", "Back")];
        let mut f = test_frame(&items, MenuKind::Options);
        f.menu_tabs = Some((vec!["General".into(), "Graphics".into()], 0));
        let names = ["Low", "Medium", "High"];
        f.dropdown = Some(DropdownView {
            row: 0,
            items: names.to_vec(),
            sel: 1,
            top: 0,
            current: Some(1),
        });
        let p = page_frame(&f, Some(&ListKind::Options(0)), test_column(true)).unwrap();
        // the page of one 1920x1080 screen, and the same on the centre one of three
        let mut one = crate::ui::Ui::new().unwrap();
        p.write_hits(&mut one, Area::whole((1920, 1080)));
        let mut three = crate::ui::Ui::new().unwrap();
        let area = crate::new_ui::menu_area(true, false, (5760, 1080));
        p.write_hits(&mut three, area);
        let moved = |r: &[f32; 4]| [r[0] + 1920.0, r[1], r[2] + 1920.0, r[3]];
        for (a, b) in [
            (&one.menu_rects, &three.menu_rects),
            (&one.menu_side, &three.menu_side),
            (&one.dd_rects, &three.dd_rects),
        ] {
            assert!(!a.is_empty());
            let a: Vec<_> = a.iter().map(moved).collect();
            assert_eq!(&a, b);
            // (inside the centre screen, right of its column)
            for r in b {
                assert!(
                    r[0] >= 1920.0 + pause_menu::COLUMN_W && r[2] <= 3840.0,
                    "{r:?}"
                );
            }
        }
        // in the headset the window's picture is the menu plane's: the window's rectangles
        let mut vr = crate::ui::Ui::new().unwrap();
        p.write_hits(&mut vr, crate::new_ui::menu_area(true, true, (1920, 1080)));
        assert_eq!(vr.menu_rects, one.menu_rects);
    }

    #[test]
    fn opening_a_page_keeps_the_game_paused_and_closing_it_returns_to_the_top_level() {
        // the Esc menu open: its top level
        assert_eq!(showing(true, false), Showing::Top);
        // a line opens a list: the page, the column still beside it saying the game is paused
        assert_eq!(showing(true, true), Showing::Page);
        let items = [("driver Ana", "Ana"), ("back", "Back")];
        let mut f = test_frame(&items, MenuKind::List);
        f.menu_head = Some(("Driver".into(), String::new()));
        let p = page_frame(&f, Some(&ListKind::Drivers), test_column(true)).unwrap();
        assert_eq!(p.column.bar().2, "paused");
        assert_eq!(p.column.current(), Some("vehicle"));
        // closed (Esc, "Back"): the top level again; the menu closed: nothing
        assert_eq!(showing(true, false), Showing::Top);
        assert_eq!(showing(false, false), Showing::None);
        // a list a plugin opened with no Esc menu: the HUD's, whose clicks it has
        assert_eq!(showing(false, true), Showing::None);
        // no list open in the HUD's frame: no page
        let mut f = test_frame(&items, MenuKind::List);
        f.menu = None;
        assert!(page_frame(&f, None, test_column(true)).is_none());
    }
}
