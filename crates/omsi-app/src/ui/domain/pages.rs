//! The menu pages' state: what a page behind an Esc menu line shows in a frame (its rows and
//! their controls, the tabs, the timetable pane, the open drop-down), made from the HUD's
//! frame of the open list, and what the new interface draws of the game menu.

use super::super::interface::output::{Frame, ListKind, MenuKind};
use super::menu::MenuFrame;
use omsi_ui::menu_page::Back;

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
pub(in crate::ui) struct PagePane {
    pub(in crate::ui) title: String,
    pub(in crate::ui) meta: String,
    pub(in crate::ui) rows: Vec<(String, String)>,
    pub(in crate::ui) chosen: Option<usize>,
    pub(in crate::ui) button: Option<String>,
    pub(in crate::ui) time: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::ui) struct PageDropdown {
    pub(in crate::ui) row: usize,
    pub(in crate::ui) items: Vec<String>,
    pub(in crate::ui) sel: usize,
    pub(in crate::ui) top: usize,
    pub(in crate::ui) current: Option<usize>,
}

/// What one frame of an open page shows, owned so that it can cross to the renderer.
#[derive(Clone, Debug, PartialEq)]
pub struct PageFrame {
    /// The Esc menu's column beside the page, and the bar over it.
    pub column: MenuFrame,
    pub title: String,
    pub note: String,
    pub(in crate::ui) tabs_title: String,
    pub(in crate::ui) tabs: Vec<String>,
    pub(in crate::ui) tab: usize,
    pub rows: Vec<PageRow>,
    pub fields: bool,
    pub selected: usize,
    pub(in crate::ui) kbd: bool,
    pub active: Option<usize>,
    pub(in crate::ui) top: Option<f32>,
    pub back: Back,
    pub(in crate::ui) pane: Option<PagePane>,
    pub(in crate::ui) pane_first: Option<usize>,
    pub(in crate::ui) dropdown: Option<PageDropdown>,
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
