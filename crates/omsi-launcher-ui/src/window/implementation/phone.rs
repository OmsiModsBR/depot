//! The launcher made for a phone (or a tablet), not the desktop's pages squeezed onto it:
//! a tab bar at the foot (Play, Online, Mods, More), a Play screen with the bus large and
//! the duty as four big cards over one Start button, and every choice made on a sheet of
//! its own that fills the screen - a list of big rows, a search field, a way back.
//! The pages a phone needs less often (Settings, Controls, Profile, …) open from More,
//! full width, under a bar with a way back.
//!
//! The desktop pages it shows (Mods, and what More opens) are the game's to draw: `phone` says
//! which and where (`PhoneFrame`), `foot` draws over them and the tab bar after.

use super::super::domain::frame::Frame;
use super::super::domain::phone::*;
use super::super::domain::showroom::Showroom;
use super::super::interface::output::{core, days_text, hhmm, Page, State};
use glam::Vec2;
use omsi_content::weather::psychro::dew_point_of_relative;
use omsi_ui::paint::Align;
use omsi_ui::theme::*;
use omsi_ui::widgets::{id_of, parse_date, ButtonKind, Ui, MONTHS};
use omsi_ui::{Color, Rect, Weight};

/// Height of the tab bar.
const TAB_H: f32 = 58.0;
/// Height of a sheet's rows.
const ROW_H: f32 = 62.0;
/// Height of a sheet's bar.
const BAR_H: f32 = 54.0;

// (the Online tab is hidden: multiplayer is disabled in this version)
const TABS: [(Tab, &str, &str); 3] = [
    (Tab::Play, "Play", "directions_bus"),
    (Tab::Mods, "Mods", "extension"),
    (Tab::More, "More", "menu"),
];

/// The pages More opens.
const MORE: [(Page, &str, &str, &str); 6] = [
    (
        Page::Profile,
        "Profile",
        "badge",
        "Your driver, level and records",
    ),
    (
        Page::Settings,
        "Settings",
        "tune",
        "Graphics, sound, gameplay",
    ),
    (
        Page::Controls,
        "Controls",
        "sports_esports",
        "Touch, wheels and gamepads",
    ),
    (
        Page::Tutorials,
        "Tutorials",
        "help",
        "Learn to drive the buses",
    ),
    (
        Page::Timetable,
        "Timetable",
        "schedule",
        "The map's lines and trips",
    ),
    (
        Page::Setup,
        "Setup",
        "folder_open",
        "The OMSI 2 folder and content",
    ),
];

/// The height a desktop page is given on a phone, at least (it scrolls as a whole).
pub const PAGE_H: f32 = 700.0;

/// The whole launcher on a phone (instead of the rail and the desktop pages), up to the desktop
/// page it shows, which the game draws (`PhoneFrame::page`); `scroll` and `max` are that page's.
pub fn phone(
    f: &mut Frame,
    pv: &mut PhoneView,
    room: &mut dyn Showroom,
    scroll: &mut f32,
    max: &mut f32,
) -> PhoneFrame {
    let size = f.ui.size;
    let full = Rect::new(0.0, 0.0, size.x, size.y);
    f.ui.p().rect(full, RAIL);
    let mut out = PhoneFrame {
        body: full,
        sheet: false,
        page: None,
        back: false,
        open: None,
    };
    if let Some(s) = pv.sheet {
        sheet(f, pv, s, full);
        out.sheet = true;
        return out;
    }
    let body = Rect::new(0.0, 0.0, size.x, size.y - TAB_H);
    out.body = body;
    match (pv.tab, pv.page) {
        (Tab::More, Some(p)) => {
            out.page = Some(embedded(f, p, body, true, scroll, max));
            out.back = true;
        }
        (Tab::More, None) => out.open = more(f, pv, body),
        (Tab::Mods, _) => out.page = Some(embedded(f, Page::Mods, body, false, scroll, max)),
        // (the Online tab is hidden without LAN play)
        (Tab::Online, _) => {}
        (Tab::Play, _) => play(f, pv, room, body),
    }
    out
}

/// After the desktop page the game drew: its bar with the way back (a page of More), then the
/// tab bar. The tab chosen, whose page the window goes to.
pub fn foot(
    f: &mut Frame,
    pv: &mut PhoneView,
    scroll: &mut f32,
    shown: &PhoneFrame,
) -> Option<Tab> {
    if shown.sheet {
        return None;
    }
    if let Some((page, _)) = shown.page {
        f.ui.pop_clip();
        if shown.back {
            let name = MORE.iter().find(|m| m.0 == page).map(|m| m.1).unwrap_or("");
            let body = shown.body;
            if bar(f, pv, Rect::new(body.x, body.y, body.w, BAR_H), name, false) {
                pv.page = None;
                *scroll = 0.0;
            }
        }
    }
    let size = f.ui.size;
    tab_bar(f, pv, scroll, Rect::new(0.0, size.y - TAB_H, size.x, TAB_H))
}

/// The status line over the foot of the screen (`PhoneFrame::body`).
pub fn status(f: &mut Frame, shown: &PhoneFrame) {
    if !shown.sheet {
        toast(f, shown.body);
    }
}

/// One of the desktop's pages, full width, scrolled as a whole (under a bar with a way back
/// when `back`): where the game draws it, the screen clipped to it.
fn embedded(
    f: &mut Frame,
    page: Page,
    body: Rect,
    back: bool,
    scroll: &mut f32,
    max: &mut f32,
) -> (Page, Rect) {
    let top = if back { BAR_H } else { 0.0 };
    let seen = body.h - top - 12.0;
    let h = seen.max(PAGE_H);
    *max = (h - seen).max(0.0);
    *scroll = scroll.clamp(0.0, *max);
    let content = Rect::new(
        body.x + 16.0,
        body.y + top + 10.0 - *scroll,
        body.w - 32.0,
        h,
    );
    f.ui.push_clip(Rect::new(body.x, body.y + top, body.w, body.h - top), 0.0);
    (page, content)
}

fn tab_bar(f: &mut Frame, pv: &mut PhoneView, scroll: &mut f32, r: Rect) -> Option<Tab> {
    let mut chosen = None;
    f.ui.solid(r);
    f.ui.p().rect(r, PANEL);
    f.ui.p().rect(Rect::new(r.x, r.y, r.w, 1.0), EDGE);
    let w = r.w / TABS.len() as f32;
    let running = f.state.instances.iter().filter(|i| i.running).count();
    for (k, (tab, name, icon)) in TABS.iter().enumerate() {
        let cell = Rect::new(r.x + w * k as f32, r.y, w, r.h);
        let (_, down, clicked) = f.ui.interact(id_of(&format!("ptab-{name}")), cell);
        if clicked {
            pv.tab = *tab;
            pv.page = None;
            *scroll = 0.0;
            chosen = Some(*tab);
        }
        let on = pv.tab == *tab;
        if down {
            f.ui.p().rounded(cell.pad(10.0, 6.0), 12.0, HOVER);
        }
        let c = if on { accent() } else { TEXT_DIM };
        if on {
            f.ui.p().rounded(
                Rect::new(cell.center().x - 28.0, cell.y + 7.0, 56.0, 28.0),
                14.0,
                accent().alpha(0.16),
            );
        }
        f.ui.icon(icon, Vec2::new(cell.center().x, cell.y + 21.0), 22.0, c);
        f.ui.text_in(
            name,
            Rect::new(cell.x, cell.y + 36.0, cell.w, 16.0),
            11.5,
            if on { Weight::Bold } else { Weight::Medium },
            c,
            Align::Center,
        );
        if *tab == Tab::More && running > 0 {
            f.ui.p()
                .circle(Vec2::new(cell.center().x + 14.0, cell.y + 12.0), 5.0, OK);
        }
    }
    chosen
}

/// The status line as a toast over the page's foot.
fn toast(f: &mut Frame, body: Rect) {
    let (text, err, at) = f.state.status.clone();
    if text.is_empty() {
        return;
    }
    let fade = if err {
        (1.0 - (at.elapsed().as_secs_f32() - 10.0) / 1.5).clamp(0.0, 1.0)
    } else {
        (1.0 - (at.elapsed().as_secs_f32() - 4.0) / 1.0).clamp(0.0, 1.0)
    };
    if fade <= 0.0 {
        return;
    }
    let first = text.lines().next().unwrap_or("").to_string();
    let w = (f.ui.width(&first, 13.0, Weight::Medium) + 44.0).min(body.w - 32.0);
    let r = Rect::new(body.center().x - w * 0.5, body.bottom() - 46.0, w, 36.0);
    f.ui.p()
        .rounded(r, 18.0, Color::rgba(40, 40, 40, 0.96 * fade));
    f.ui.icon(
        if err { "error" } else { "info" },
        Vec2::new(r.x + 18.0, r.center().y),
        16.0,
        (if err { DANGER } else { TEXT_DIM }).alpha(fade),
    );
    f.ui.text_in(
        &first,
        Rect::new(r.x + 32.0, r.y, r.w - 42.0, r.h),
        13.0,
        Weight::Medium,
        TEXT.alpha(fade),
        Align::Left,
    );
}

/// A card of the Play screen: what is chosen, and the way to its sheet.
fn card(
    f: &mut Frame,
    name: &str,
    r: Rect,
    icon: &str,
    label: &str,
    value: &str,
    warn: bool,
) -> bool {
    let (h, down, clicked) = f.ui.interact(id_of(name), r);
    f.ui.p().rounded(
        r,
        14.0,
        if down {
            SELECTED
        } else if h {
            HOVER
        } else {
            FIELD
        },
    );
    let ic = Vec2::new(r.x + 26.0, r.center().y);
    f.ui.p().circle(ic, 17.0, Color::rgba(255, 255, 255, 0.05));
    f.ui.icon(icon, ic, 19.0, if warn { WARN } else { accent() });
    f.ui.text_in(
        label,
        Rect::new(r.x + 54.0, r.y + 8.0, r.w - 84.0, 15.0),
        11.0,
        Weight::Medium,
        TEXT_DIM,
        Align::Left,
    );
    f.ui.text_in(
        value,
        Rect::new(r.x + 54.0, r.y + 24.0, r.w - 84.0, r.h - 30.0),
        14.5,
        Weight::Bold,
        TEXT,
        Align::Left,
    );
    f.ui.icon(
        "chevron_right",
        Vec2::new(r.right() - 18.0, r.center().y),
        20.0,
        TEXT_FAINT,
    );
    clicked
}

fn duty_text(state: &State) -> String {
    match (&state.choice.line, &state.choice.tour, state.choice.free) {
        (_, _, true) | (None, _, _) => "Free drive".into(),
        (Some(line), Some(t), _) => format!("Line {line} · tour {t}"),
        (Some(line), None, _) => format!("Line {line} · choose a tour"),
    }
}

fn time_text(state: &State) -> String {
    let (y, m, d) = parse_date(&state.choice.date);
    let weather = match state.choice.weather.strip_prefix("metar:") {
        Some(c) => format!("live {c}"),
        None if state.choice.weather == "cycle" => "weather cycle".into(),
        None if core::weather::custom_weather(Some(&state.choice.weather)).is_some() => {
            let c = core::weather::custom_weather(Some(&state.choice.weather)).unwrap();
            format!("custom · {:.0}°C · {:.0}% RH", c.temp_c, c.humidity)
        }
        None => state
            .weathers
            .iter()
            .find(|w| w.file == state.choice.weather)
            .map(|w| w.name.clone())
            .unwrap_or_else(|| "map weather".into()),
    };
    format!(
        "{:02}:{:02} · {d} {} {y} · {weather}",
        state.choice.time / 60,
        state.choice.time % 60,
        &MONTHS[(m as usize).clamp(1, 12) - 1][..3]
    )
}

fn start_text(state: &State) -> String {
    let where_ = if state.choice.entry < 0 {
        "Automatic".to_string()
    } else {
        state
            .map()
            .and_then(|m| m.entry_points.get(state.choice.entry as usize))
            .map(|e| {
                if e.name.is_empty() {
                    format!("entry {}", e.index + 1)
                } else {
                    e.name.clone()
                }
            })
            .unwrap_or_else(|| "Automatic".into())
    };
    format!("{where_} · {:.0} cars", state.choice.traffic)
}

fn play(f: &mut Frame, pv: &mut PhoneView, room: &mut dyn Showroom, body: Rect) {
    let pad = 12.0;
    let inner = body.pad(pad, pad);
    let wide = inner.w > 1.35 * inner.h;
    // the bus: the left of a phone held sideways, the top of one held upright
    let (pr, col) = if wide {
        let pw = (inner.w * 0.52).round();
        (
            Rect::new(inner.x, inner.y, pw, inner.h),
            Rect::new(inner.x + pw + pad, inner.y, inner.w - pw - pad, inner.h),
        )
    } else {
        let ph = (inner.h * 0.4).round();
        (
            Rect::new(inner.x, inner.y, inner.w, ph),
            Rect::new(inner.x, inner.y + ph + pad, inner.w, inner.h - ph - pad),
        )
    };
    room.card(f.ui, pr);
    // The phone has its own Play page, so keep the less frequently used desktop choices
    // close to the bus without making the main card stack taller.
    if f.state.bus().is_some() {
        if f.ui.icon_button(
            "p-vehicle-settings",
            Vec2::new(pr.right() - 26.0, pr.y + 26.0),
            18.0,
            "settings",
            "Vehicle settings",
        ) {
            open(pv, Sheet::Vehicle);
        }
    }
    if !f.state.choice.free && f.state.choice.line.is_some() && f.state.choice.tour.is_some() {
        if f.ui.icon_button(
            "p-roadbook",
            Vec2::new(pr.right() - 26.0, pr.y + 72.0),
            18.0,
            "receipt_long",
            "Roadbook and IBIS",
        ) {
            f.state.load_ibis();
            open(pv, Sheet::Roadbook);
        }
    }
    // the bus's name over the foot of its picture, the livery as a chip beside it
    let shade = Rect::new(pr.x, pr.bottom() - 58.0, pr.w, 58.0);
    f.ui.p().rounded(shade, RADIUS, Color::rgba(0, 0, 0, 0.55));
    let (name, maker) = f
        .state
        .bus()
        .map(|b| (b.name.clone(), b.manufacturer.clone()))
        .unwrap_or_else(|| ("Choose a bus".into(), String::new()));
    f.ui.text_in(
        &maker,
        Rect::new(shade.x + 14.0, shade.y + 7.0, shade.w - 150.0, 15.0),
        11.0,
        Weight::Medium,
        TEXT_DIM,
        Align::Left,
    );
    f.ui.text_in(
        &name,
        Rect::new(shade.x + 14.0, shade.y + 22.0, shade.w - 150.0, 26.0),
        17.0,
        Weight::Bold,
        TEXT,
        Align::Left,
    );
    let paints = f.state.bus().map(|b| b.paints.len()).unwrap_or(0);
    if paints > 0 {
        let label = if f.state.choice.paint.is_empty() {
            f.state
                .bus()
                .map(crate::window::default_livery_label)
                .unwrap_or("Default paint")
                .to_string()
        } else {
            f.state.choice.paint.clone()
        };
        let cw = (f.ui.width(&label, 12.0, Weight::Medium) + 40.0).min(170.0);
        let chip = Rect::new(shade.right() - cw - 10.0, shade.y + 13.0, cw, 32.0);
        let (_, down, clicked) = f.ui.interact(id_of("p-livery"), chip);
        f.ui.p().rounded(
            chip,
            16.0,
            if down {
                SELECTED
            } else {
                Color::rgba(255, 255, 255, 0.1)
            },
        );
        f.ui.icon(
            "palette",
            Vec2::new(chip.x + 16.0, chip.center().y),
            15.0,
            accent(),
        );
        f.ui.text_in(
            &label,
            Rect::new(chip.x + 28.0, chip.y, chip.w - 34.0, chip.h),
            12.0,
            Weight::Medium,
            TEXT,
            Align::Left,
        );
        if clicked {
            open(pv, Sheet::Livery);
        }
    }
    // the duty as four cards, the start under them
    let start_h = 54.0;
    let n = 5.0;
    let gap = 8.0;
    let card_h = ((col.h - start_h - gap * n) / n).clamp(42.0, 64.0);
    let map = f
        .state
        .map()
        .map(|m| {
            if m.friendly.is_empty() {
                m.name.clone()
            } else {
                m.friendly.clone()
            }
        })
        .unwrap_or_else(|| "Choose a map".into());
    let incomplete = f.state.bus().is_some_and(|b| !b.missing_packs.is_empty());
    let bus = f
        .state
        .bus()
        .map(|b| b.name.clone())
        .unwrap_or_else(|| "Choose a bus".into());
    let (duty, time, start_opts) = (duty_text(f.state), time_text(f.state), start_text(f.state));
    let mut y = col.y;
    let cards = [
        ("p-map", "map", "Map", map, false, Sheet::Map),
        (
            "p-bus",
            "directions_bus",
            "Bus",
            bus,
            incomplete,
            Sheet::Bus,
        ),
        ("p-duty", "route", "Duty", duty, false, Sheet::Duty),
        (
            "p-time",
            "partly_cloudy_day",
            "Time & weather",
            time,
            false,
            Sheet::Time,
        ),
        (
            "p-start-options",
            "tune",
            "Start options",
            start_opts,
            false,
            Sheet::Start,
        ),
    ];
    for (id, icon, label, value, warn, s) in cards {
        if card(
            f,
            id,
            Rect::new(col.x, y, col.w, card_h),
            icon,
            label,
            &value,
            warn,
        ) {
            if s == Sheet::Time && f.state.joined_server.is_some() {
                f.state
                    .set_status("On a server its clock and weather are the server's", false);
            } else {
                open(pv, s);
            }
        }
        y += card_h + gap;
    }
    // Start, and "continue where you left off" beside it when there is a game to continue
    let btn = Rect::new(col.x, col.bottom() - start_h, col.w, start_h);
    let cont = f.state.joined_server.is_none() && f.state.has_last_situation();
    let (start, rest) = if cont {
        (
            Rect::new(btn.x + btn.w * 0.34 + 8.0, btn.y, btn.w * 0.66 - 8.0, btn.h),
            Some(Rect::new(btn.x, btn.y, btn.w * 0.34, btn.h)),
        )
    } else {
        (btn, None)
    };
    if let Some(c) = rest {
        if f.ui.button(
            "p-continue",
            c,
            "Continue",
            Some("history"),
            ButtonKind::Normal,
        ) {
            f.state.launch_last_situation();
        }
    }
    let free = f.state.choice.free || f.state.choice.line.is_none();
    let label = if f.state.joined_server.is_some() {
        "Join and drive"
    } else if free {
        "Drive"
    } else {
        "Start the duty"
    };
    if f.ui.button(
        "p-start",
        start,
        label,
        Some("play_arrow"),
        ButtonKind::Primary,
    ) {
        crate::window::start_from_phone(f);
    }
}

fn open(pv: &mut PhoneView, s: Sheet) {
    pv.sheet = Some(s);
    pv.filter.clear();
}

/// A sheet's bar: the way back, the title, and a search field when `search`.
fn bar(f: &mut Frame, pv: &mut PhoneView, r: Rect, title: &str, search: bool) -> bool {
    f.ui.solid(r);
    f.ui.p().rect(r, PANEL);
    f.ui.p()
        .rect(Rect::new(r.x, r.bottom() - 1.0, r.w, 1.0), EDGE);
    let back = Rect::new(r.x + 6.0, r.y + 5.0, 44.0, 44.0);
    let (_, down, clicked) = f.ui.interact(id_of("sheet-back"), back);
    if down {
        f.ui.p().circle(back.center(), 20.0, HOVER);
    }
    f.ui.icon("arrow_back", back.center(), 22.0, TEXT);
    let tw = if search {
        (r.w * 0.34).min(260.0)
    } else {
        r.w - 70.0
    };
    f.ui.text_in(
        title,
        Rect::new(r.x + 56.0, r.y, tw, r.h),
        18.0,
        Weight::Bold,
        TEXT,
        Align::Left,
    );
    if search {
        let field = Rect::new(r.x + 60.0 + tw, r.y + 8.0, r.w - tw - 72.0, r.h - 16.0);
        f.ui.text_input(
            "sheet-search",
            field,
            &mut pv.filter,
            "Search…",
            Some("search"),
        );
    }
    clicked
}

/// A big row of a sheet: its title, a line under it, and whether it is the one chosen.
fn big_row(
    ui: &mut Ui,
    id: &str,
    r: Rect,
    title: &str,
    sub: &str,
    chosen: bool,
    badge: Option<(&str, Color)>,
) -> bool {
    let clicked = ui.row(id, r, chosen);
    let tx = r.x + 16.0;
    let mut tw = r.w - 60.0;
    if let Some((b, c)) = badge {
        // (left of the check mark of the chosen row)
        let bw = ui.width(b, 10.0, Weight::Bold) + 10.0;
        ui.badge(Vec2::new(r.right() - 48.0 - bw, r.y + 23.0), b, c);
        tw -= bw + 12.0;
    }
    ui.text_in(
        title,
        Rect::new(tx, r.y + 9.0, tw, 22.0),
        15.5,
        Weight::Bold,
        TEXT,
        Align::Left,
    );
    if !sub.is_empty() {
        ui.text_in(
            sub,
            Rect::new(tx, r.y + 32.0, r.w - 60.0, 18.0),
            12.0,
            Weight::Regular,
            TEXT_DIM,
            Align::Left,
        );
    }
    if chosen {
        ui.icon(
            "check_circle",
            Vec2::new(r.right() - 24.0, r.center().y),
            20.0,
            accent(),
        );
    }
    clicked
}

fn sheet(f: &mut Frame, pv: &mut PhoneView, s: Sheet, full: Rect) {
    let title = match s {
        Sheet::Map => "Choose the map",
        Sheet::Bus => "Choose the bus",
        Sheet::Livery => "Choose the livery",
        Sheet::Vehicle => "Vehicle settings",
        Sheet::Duty => "Line or free drive",
        Sheet::Tour => "Choose the tour",
        Sheet::Time => "Time and weather",
        Sheet::Start => "Start options",
        Sheet::Roadbook => "Roadbook and IBIS",
    };
    let search = matches!(s, Sheet::Map | Sheet::Bus | Sheet::Duty);
    let barr = Rect::new(full.x, full.y, full.w, BAR_H);
    let list = Rect::new(
        full.x + 8.0,
        barr.bottom() + 8.0,
        full.w - 16.0,
        full.h - BAR_H - 16.0,
    );
    let back = match s {
        Sheet::Map => map_sheet(f, pv, list),
        Sheet::Bus => bus_sheet(f, pv, list),
        Sheet::Livery => livery_sheet(f, list),
        Sheet::Vehicle => vehicle_sheet(f, list),
        Sheet::Duty => duty_sheet(f, pv, list),
        Sheet::Tour => tour_sheet(f, list),
        Sheet::Time => time_sheet(f, list),
        Sheet::Start => start_sheet(f, list),
        Sheet::Roadbook => roadbook_sheet(f, list),
    };
    // (the bar drawn last: the list scrolls under it)
    if bar(f, pv, barr, title, search) || back {
        pv.sheet = match (s, back) {
            // a line chosen: its tours next
            (Sheet::Duty, true) if f.state.choice.line.is_some() && !f.state.choice.free => {
                Some(Sheet::Tour)
            }
            // a bus chosen that has liveries: the livery next
            (Sheet::Bus, true) if f.state.bus().is_some_and(|b| !b.paints.is_empty()) => {
                Some(Sheet::Livery)
            }
            _ => None,
        };
        pv.filter.clear();
    }
}

fn matches(q: &str, text: &str) -> bool {
    q.is_empty() || text.to_lowercase().contains(q)
}

fn map_sheet(f: &mut Frame, pv: &mut PhoneView, r: Rect) -> bool {
    let q = pv.filter.to_lowercase();
    let items: Vec<(String, String, String, bool)> = f
        .state
        .maps
        .iter()
        .filter(|m| matches(&q, &format!("{} {} {}", m.friendly, m.name, m.file)))
        .map(|m| {
            (
                m.file.clone(),
                if m.friendly.is_empty() {
                    m.name.clone()
                } else {
                    m.friendly.clone()
                },
                m.description.lines().next().unwrap_or("").to_string(),
                m.installed,
            )
        })
        .collect();
    let chosen = f.state.choice.map.clone();
    let mut pick = None;
    f.ui.scroll_area("ps-maps", r, &mut |ui, v| {
        for (k, (file, name, desc, mod_)) in items.iter().enumerate() {
            let rr = Rect::new(v.x, v.y + k as f32 * (ROW_H + 6.0), v.w - 8.0, ROW_H);
            if big_row(
                ui,
                &format!("pm-{file}"),
                rr,
                name,
                desc,
                *file == chosen,
                mod_.then_some(("MOD", ACCENT_2)),
            ) {
                pick = Some(file.clone());
            }
        }
        items.len() as f32 * (ROW_H + 6.0)
    });
    if let Some(file) = pick {
        f.state.select_map(&file);
        return true;
    }
    false
}

fn bus_sheet(f: &mut Frame, pv: &mut PhoneView, r: Rect) -> bool {
    let q = pv.filter.to_lowercase();
    let norm = |f: &str| f.replace('\\', "/").to_ascii_lowercase();
    let allowed: Option<std::collections::HashSet<String>> = f
        .state
        .host_vehicles()
        .map(|v| v.iter().map(|f| norm(f)).collect());
    let items: Vec<(String, String, String, bool, bool)> = f
        .state
        .vehicles
        .iter()
        .filter(|v| matches(&q, &format!("{} {} {}", v.name, v.manufacturer, v.file)))
        .filter(|v| {
            allowed
                .as_ref()
                .map(|a| a.contains(&norm(&v.file)))
                .unwrap_or(true)
        })
        .map(|v| {
            let mut sub = v.manufacturer.clone();
            sub = format!(
                "{sub}{}{}",
                if sub.is_empty() { "" } else { " · " },
                crate::window::liveries_text(v.paints.len())
            );
            if !v.missing_packs.is_empty() {
                sub = format!("{sub} · parts missing");
            }
            (
                v.file.clone(),
                v.name.clone(),
                sub,
                v.installed,
                !v.missing_packs.is_empty(),
            )
        })
        .collect();
    let chosen = f.state.choice.bus.clone();
    let loading = f.state.loading_content;
    let mut pick = None;
    f.ui.scroll_area("ps-buses", r, &mut |ui, v| {
        if items.is_empty() {
            ui.text_in(
                if loading {
                    "Reading the buses…"
                } else {
                    "No bus matches."
                },
                Rect::new(v.x + 16.0, v.y, v.w, 40.0),
                14.0,
                Weight::Regular,
                TEXT_DIM,
                Align::Left,
            );
        }
        for (k, (file, name, sub, mod_, incomplete)) in items.iter().enumerate() {
            let rr = Rect::new(v.x, v.y + k as f32 * (ROW_H + 6.0), v.w - 8.0, ROW_H);
            if rr.bottom() < r.y - ROW_H * 2.0 || rr.y > r.bottom() + ROW_H * 2.0 {
                continue;
            }
            let badge = if *incomplete {
                Some(("PARTS", WARN))
            } else if *mod_ {
                Some(("MOD", ACCENT_2))
            } else {
                None
            };
            if big_row(
                ui,
                &format!("pb-{file}"),
                rr,
                name,
                sub,
                *file == chosen,
                badge,
            ) {
                pick = Some(file.clone());
            }
        }
        items.len() as f32 * (ROW_H + 6.0)
    });
    if let Some(file) = pick {
        f.state.select_bus(&file);
        return true;
    }
    false
}

fn livery_sheet(f: &mut Frame, r: Rect) -> bool {
    let paints: Vec<String> = f.state.bus().map(|b| b.paints.clone()).unwrap_or_default();
    let default_paint = f
        .state
        .bus()
        .map(crate::window::default_livery_label)
        .unwrap_or("Default paint")
        .to_string();
    let chosen = f.state.choice.paint.clone();
    let mut pick: Option<String> = None;
    f.ui.scroll_area("ps-paints", r, &mut |ui, v| {
        let all: Vec<String> = std::iter::once(String::new())
            .chain(paints.iter().cloned())
            .collect();
        for (k, p) in all.iter().enumerate() {
            let rr = Rect::new(v.x, v.y + k as f32 * (ROW_H + 6.0), v.w - 8.0, ROW_H);
            let name = if p.is_empty() {
                default_paint.as_str()
            } else {
                p.as_str()
            };
            if big_row(ui, &format!("pp-{k}"), rr, name, "", *p == chosen, None) {
                pick = Some(p.clone());
            }
        }
        all.len() as f32 * (ROW_H + 6.0)
    });
    if let Some(p) = pick {
        f.state.choice.paint = p;
        f.state.touched();
        return true;
    }
    false
}

fn vehicle_sheet(f: &mut Frame, r: Rect) -> bool {
    let Some(vehicle) = f.state.bus().cloned() else {
        f.ui.text_in(
            "Choose a bus first.",
            Rect::new(r.x + 16.0, r.y, r.w - 32.0, 40.0),
            14.0,
            Weight::Regular,
            TEXT_DIM,
            Align::Left,
        );
        return false;
    };
    let inner = r.pad(10.0, 8.0);
    let field = (inner.w * 0.62).max(190.0);
    let label_w = (inner.w - field - 10.0).max(100.0);
    let mut y = inner.y;

    let auto = f.state.default_hof();
    let mut hof_options = vec![format!("Automatic ({auto})")];
    hof_options.extend(vehicle.hofs.iter().cloned());
    let mut hof_sel = if f.state.choice.hof_manual {
        vehicle
            .hofs
            .iter()
            .position(|h| h.eq_ignore_ascii_case(&f.state.choice.hof))
            .map(|i| i + 1)
            .unwrap_or(0)
    } else {
        0
    };
    f.ui.label(Rect::new(inner.x, y, label_w, ROW), "Depot file");
    if f.ui.select(
        "pv-hof",
        Rect::new(inner.x + label_w + 10.0, y, field, ROW),
        &mut hof_sel,
        &hof_options,
    ) {
        f.state.choice.hof_manual = hof_sel != 0;
        f.state.choice.hof = if hof_sel == 0 {
            auto.clone()
        } else {
            vehicle.hofs[hof_sel - 1].clone()
        };
        f.state.load_ibis();
        f.state.touched();
    }
    y += ROW + 12.0;

    if !vehicle.numbers.is_empty() {
        let number_options: Vec<String> = vehicle
            .numbers
            .iter()
            .map(|(number, plate)| {
                if plate.trim().is_empty() {
                    number.clone()
                } else {
                    format!("{number}  ({})", plate.trim())
                }
            })
            .collect();
        let mut sel = vehicle
            .numbers
            .iter()
            .position(|(number, _)| *number == f.state.choice.number)
            .unwrap_or(0);
        f.ui.label(Rect::new(inner.x, y, label_w, ROW), "Fleet number");
        if f.ui.select(
            "pv-number",
            Rect::new(inner.x + label_w + 10.0, y, field, ROW),
            &mut sel,
            &number_options,
        ) {
            f.state.choice.number = vehicle.numbers[sel].0.clone();
            f.state.touched();
        }
        y += ROW + 12.0;
    }

    let mut plate = f.state.choice.plate.clone();
    f.ui.label(Rect::new(inner.x, y, label_w, ROW), "Number plate");
    if f.ui.text_input(
        "pv-plate",
        Rect::new(inner.x + label_w + 10.0, y, field, ROW),
        &mut plate,
        "Automatic",
        Some("badge"),
    ) {
        f.state.choice.plate = plate;
        f.state.touched();
    }
    y += ROW + 18.0;

    if !vehicle.missing_packs.is_empty() {
        y += f.ui.paragraph(
            &omsi_ui::tr("Parts missing: needs %{packs}")
                .replace("%{packs}", &vehicle.missing_packs.join(", ")),
            Vec2::new(inner.x, y),
            inner.w,
            12.5,
            Weight::Regular,
            WARN,
        ) + 12.0;
    }
    let description = vehicle
        .description
        .replace('\t', " ")
        .lines()
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    if !description.is_empty() {
        y += f.ui.paragraph(
            &description,
            Vec2::new(inner.x, y),
            inner.w,
            12.0,
            Weight::Regular,
            TEXT_DIM,
        ) + 12.0;
    }
    f.ui.text_in(
        &vehicle.file,
        Rect::new(inner.x, y, inner.w, 22.0),
        10.5,
        Weight::Regular,
        TEXT_FAINT,
        Align::Left,
    );
    false
}

fn start_sheet(f: &mut Frame, r: Rect) -> bool {
    let inner = r.pad(10.0, 8.0);
    let mut y = inner.y;
    if let Some(m) = f.state.map().cloned() {
        let mut labels = vec![if f.state.choice.free {
            "Automatic (the map's first)".to_string()
        } else {
            "Automatic (nearest to the first stop)".to_string()
        }];
        labels.extend(m.entry_points.iter().map(|e| {
            if e.name.is_empty() {
                format!("entry {}", e.index + 1)
            } else {
                e.name.clone()
            }
        }));
        let mut es = if f.state.choice.entry < 0 {
            0
        } else {
            (f.state.choice.entry as usize + 1).min(labels.len().saturating_sub(1))
        };
        f.ui.label(Rect::new(inner.x, y, 112.0, ROW), "Start at");
        if labels.len() > 1
            && f.ui.select(
                "ps-start-at",
                Rect::new(inner.x + 112.0, y, inner.w - 112.0, ROW),
                &mut es,
                &labels,
            )
        {
            f.state.choice.entry = es as i32 - 1;
            f.state.touched();
        }
        y += ROW + 14.0;
    }

    // A joined server owns the world traffic/weather. The spawn point is still the local
    // bus's choice, just as on the desktop Drive page.
    if f.state.joined_server.is_some() {
        f.ui.paragraph(
            "Traffic, passengers and the world's clock are set by the server.",
            Vec2::new(inner.x, y),
            inner.w,
            13.0,
            Weight::Regular,
            TEXT_DIM,
        );
        return false;
    }

    let seasons = ["auto", "spring", "summer", "autumn", "winter"];
    let labels: Vec<String> = ["By date", "Spring", "Summer", "Autumn", "Winter"]
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    let mut season = seasons
        .iter()
        .position(|s| *s == f.state.choice.season)
        .unwrap_or(0);
    f.ui.label(Rect::new(inner.x, y, 112.0, ROW), "Season");
    if f.ui.select(
        "ps-season",
        Rect::new(inner.x + 112.0, y, inner.w - 112.0, ROW),
        &mut season,
        &labels,
    ) {
        f.state.choice.season = seasons[season].to_string();
        if season > 0 {
            let month = ["", "04", "07", "10", "01"][season];
            let date = f.state.choice.date.clone();
            let (yy, dd) = (
                date.get(0..4).unwrap_or("1989").to_string(),
                date.get(8..10).unwrap_or("15").to_string(),
            );
            f.state.choice.date = format!("{yy}-{month}-{dd}");
            f.state.load_lines();
        }
        let weather = f.state.choice.weather.clone();
        if let Some(w) = f.state.weathers.iter().find(|x| x.file == weather).cloned() {
            if !f.state.weather_fits(&w) {
                f.state.choice.weather.clear();
            }
        }
        f.state.touched();
    }
    y += ROW + 14.0;

    let mut traffic = f.state.choice.traffic;
    if f.ui.slider(
        "ps-traffic",
        Rect::new(inner.x, y, inner.w, 36.0),
        &mut traffic,
        0.0,
        120.0,
        1.0,
        "Cars around",
        &|v| format!("{v:.0}"),
    ) {
        f.state.choice.traffic = traffic;
        f.state.touched();
    }
    y += 44.0;

    let half = (inner.w - 12.0) * 0.5;
    let mut passengers = f.state.choice.passengers;
    if f.ui.toggle(
        "ps-passengers",
        Rect::new(inner.x, y, half, 34.0),
        &mut passengers,
        "Passengers",
    ) {
        f.state.choice.passengers = passengers;
        f.state.touched();
    }
    let mut schedule = f.state.choice.schedule;
    if f.ui.toggle(
        "ps-schedule",
        Rect::new(inner.x + half + 12.0, y, half, 34.0),
        &mut schedule,
        "Timetable buses",
    ) {
        f.state.choice.schedule = schedule;
        f.state.touched();
    }
    y += 42.0;

    let mut autostart = f.state.choice.autostart;
    if f.ui.toggle(
        "ps-autostart",
        Rect::new(inner.x, y, inner.w, 34.0),
        &mut autostart,
        "Put the bus into service on start (Shift+U)",
    ) {
        f.state.choice.autostart = autostart;
        f.state.touched();
    }
    y += 42.0;

    let mut on_foot = f.state.choice.on_foot;
    if f.ui.toggle(
        "ps-on-foot",
        Rect::new(inner.x, y, inner.w, 34.0),
        &mut on_foot,
        "Start on foot (place a bus from the game menu)",
    ) {
        f.state.choice.on_foot = on_foot;
        f.state.touched();
    }
    false
}

fn roadbook_sheet(f: &mut Frame, r: Rect) -> bool {
    let ibis_h = 154.0;
    let (Some(line), Some(tour), false) = (
        f.state.line().cloned(),
        f.state.tour().cloned(),
        f.state.choice.free,
    ) else {
        f.ui.paragraph(
            "Choose a line and a tour to see the roadbook and the IBIS codes.",
            Vec2::new(r.x + 12.0, r.y + 8.0),
            r.w - 24.0,
            13.0,
            Weight::Regular,
            TEXT_DIM,
        );
        crate::window::ibis_box(
            f,
            Rect::new(r.x + 8.0, r.bottom() - ibis_h, r.w - 16.0, ibis_h),
        );
        return false;
    };
    let from = f.state.first_trip().unwrap_or(0);
    let trips: Vec<omsi_launcher_lib::TripInfo> = tour.trips.iter().skip(from).cloned().collect();
    let list = Rect::new(
        r.x + 4.0,
        r.y + 4.0,
        r.w - 8.0,
        (r.h - ibis_h - 14.0).max(80.0),
    );
    f.ui.scroll_area("phone-roadbook", list, &mut |ui, v| {
        let mut y = v.y;
        for (k, trip) in trips.iter().enumerate() {
            let head = Rect::new(v.x, y, v.w - 8.0, 54.0);
            ui.p()
                .rounded(head, 8.0, if k == 0 { SELECTED } else { FIELD });
            ui.text_in(
                &format!(
                    "{} · {} → {}",
                    if k == 0 { "Your first trip" } else { "Then" },
                    if trip.from.is_empty() {
                        "?"
                    } else {
                        &trip.from
                    },
                    trip.terminus
                ),
                Rect::new(head.x + 12.0, head.y + 7.0, head.w - 24.0, 20.0),
                13.0,
                Weight::Bold,
                TEXT,
                Align::Left,
            );
            ui.text_in(
                &format!(
                    "{} - {} · {:.1} km · {}",
                    hhmm(trip.departure),
                    hhmm(trip.arrival),
                    trip.km,
                    if trip.line.is_empty() {
                        "depot run"
                    } else {
                        trip.line.as_str()
                    }
                ),
                Rect::new(head.x + 12.0, head.y + 30.0, head.w - 24.0, 18.0),
                11.5,
                Weight::Regular,
                TEXT_DIM,
                Align::Left,
            );
            y += 62.0;
            for stop in &trip.stops {
                let row = Rect::new(v.x + 6.0, y, v.w - 20.0, 30.0);
                ui.text_in(
                    &hhmm(stop.arr),
                    Rect::new(row.x, row.y, 52.0, row.h),
                    11.5,
                    Weight::Condensed,
                    TEXT_SOFT,
                    Align::Left,
                );
                ui.text_in(
                    &stop.name,
                    Rect::new(row.x + 58.0, row.y, row.w - 62.0, row.h),
                    12.5,
                    Weight::Regular,
                    TEXT,
                    Align::Left,
                );
                y += 30.0;
            }
            y += 10.0;
        }
        y - v.y + 4.0
    });
    let _ = line;
    crate::window::ibis_box(
        f,
        Rect::new(r.x + 8.0, r.bottom() - ibis_h, r.w - 16.0, ibis_h),
    );
    false
}

fn duty_sheet(f: &mut Frame, pv: &mut PhoneView, r: Rect) -> bool {
    let q = pv.filter.to_lowercase();
    let lines: Vec<(String, String, usize)> = f
        .state
        .lines
        .iter()
        .filter(|x| x.user_allowed)
        .filter(|x| matches(&q, &format!("{} {}", x.name, x.termini.join(" "))))
        .map(|x| (x.name.clone(), x.termini.join(" – "), x.tours.len()))
        .collect();
    let free = f.state.choice.free || f.state.choice.line.is_none();
    let chosen = f.state.choice.line.clone();
    let loading = f.state.loading_lines;
    let mut pick: Option<Option<String>> = None;
    f.ui.scroll_area("ps-lines", r, &mut |ui, v| {
        let free_r = Rect::new(v.x, v.y, v.w - 8.0, ROW_H);
        if big_row(
            ui,
            "pl-free",
            free_r,
            "Free drive",
            "No timetable: drive where you like, the buses and traffic about you",
            free,
            None,
        ) {
            pick = Some(None);
        }
        let mut y = v.y + ROW_H + 14.0;
        ui.text_in(
            if loading {
                "Reading the timetable…"
            } else if lines.is_empty() {
                "The map has no lines to drive."
            } else {
                "Lines of the timetable"
            },
            Rect::new(v.x + 8.0, y, v.w, 18.0),
            12.0,
            Weight::Medium,
            TEXT_DIM,
            Align::Left,
        );
        y += 24.0;
        for (name, termini, tours) in &lines {
            let rr = Rect::new(v.x, y, v.w - 8.0, ROW_H);
            if big_row(
                ui,
                &format!("pl-{name}"),
                rr,
                &format!("Line {name}"),
                &format!("{termini} · {tours} tours"),
                !free && chosen.as_deref() == Some(name.as_str()),
                None,
            ) {
                pick = Some(Some(name.clone()));
            }
            y += ROW_H + 6.0;
        }
        y - v.y
    });
    match pick {
        Some(None) => {
            f.state.choice.free = true;
            f.state.touched();
            true
        }
        Some(Some(n)) => {
            f.state.choice.free = false;
            if f.state.choice.line.as_deref() != Some(n.as_str()) {
                f.state.choice.line = Some(n);
                f.state.choice.tour = None;
            }
            f.state.touched();
            true
        }
        None => false,
    }
}

fn tour_sheet(f: &mut Frame, r: Rect) -> bool {
    let Some(line) = f.state.line().cloned() else {
        f.ui.text_in(
            "Choose a line first.",
            Rect::new(r.x + 16.0, r.y, r.w, 40.0),
            14.0,
            Weight::Regular,
            TEXT_DIM,
            Align::Left,
        );
        return false;
    };
    let mut tours: Vec<&omsi_launcher_lib::TourInfo> = line.tours.iter().collect();
    tours.sort_by(|a, b| {
        b.runs
            .cmp(&a.runs)
            .then_with(|| crate::window::natural(&a.number).cmp(&crate::window::natural(&b.number)))
    });
    let items: Vec<(String, String, bool, Option<String>)> = tours
        .iter()
        .map(|t| {
            let when = format!("{} – {}", hhmm(t.first), hhmm(t.last));
            let sub = if t.runs {
                format!(
                    "{when} · {}",
                    omsi_ui::tr("%{trips} trips · %{days}")
                        .replace("%{trips}", &t.trips.len().to_string())
                        .replace("%{days}", &days_text(&t.days))
                )
            } else {
                format!(
                    "{when} · {} · {}",
                    days_text(&t.days),
                    omsi_ui::tr("runs %{date}").replace(
                        "%{date}",
                        &t.next_run
                            .clone()
                            .unwrap_or_else(|| omsi_ui::tr("never").into_owned())
                    )
                )
            };
            (t.number.clone(), sub, t.runs, t.next_run.clone())
        })
        .collect();
    let chosen = f.state.choice.tour.clone();
    let mut pick = None;
    f.ui.scroll_area("ps-tours", r, &mut |ui, v| {
        for (k, (num, sub, runs, next)) in items.iter().enumerate() {
            let rr = Rect::new(v.x, v.y + k as f32 * (ROW_H + 6.0), v.w - 8.0, ROW_H);
            if big_row(
                ui,
                &format!("pt-{num}"),
                rr,
                &format!("Tour {num}"),
                sub,
                chosen.as_deref() == Some(num.as_str()),
                (!runs).then_some(("OTHER DAY", TEXT_FAINT)),
            ) {
                pick = Some((num.clone(), *runs, next.clone()));
            }
        }
        items.len() as f32 * (ROW_H + 6.0)
    });
    if let Some((num, runs, next)) = pick {
        // a tour of another day moves the date to the next day it runs, as the desktop does
        if !runs {
            if let Some(n) = next {
                f.state.choice.date = n;
                f.state.load_lines();
            }
        }
        f.state.choice.tour = Some(num);
        f.state.touched();
        return true;
    }
    false
}

fn time_sheet(f: &mut Frame, r: Rect) -> bool {
    let left = Rect::new(
        r.x + 8.0,
        r.y + 4.0,
        (r.w * 0.42).max(260.0).min(r.w - 16.0),
        r.h - 8.0,
    );
    f.ui.text_in(
        "Start",
        Rect::new(left.x, left.y, left.w, 18.0),
        12.0,
        Weight::Medium,
        TEXT_DIM,
        Align::Left,
    );
    let mut t = f.state.choice.time;
    if f.ui.time_field(
        "p-time",
        Rect::new(left.x, left.y + 22.0, left.w, 48.0),
        &mut t,
    ) {
        f.state.choice.time = t;
        f.state.touched();
    }
    f.ui.text_in(
        "Date",
        Rect::new(left.x, left.y + 84.0, left.w, 18.0),
        12.0,
        Weight::Medium,
        TEXT_DIM,
        Align::Left,
    );
    let mut d = f.state.choice.date.clone();
    if f.ui.date_field(
        "p-date",
        Rect::new(left.x, left.y + 106.0, left.w, 48.0),
        &mut d,
    ) {
        f.state.choice.date = d;
        f.state.choice.season = "auto".into();
        f.state.load_lines();
        f.state.touched();
    }
    let done = f.ui.button(
        "p-time-done",
        Rect::new(left.x, left.bottom() - 52.0, left.w, 52.0),
        "Done",
        Some("check"),
        ButtonKind::Primary,
    );
    // Weather has a phone/tablet layout of its own; keep the same choices and editors as
    // the desktop launcher instead of silently losing Custom weather / METAR on Android.
    let right = Rect::new(
        left.right() + 16.0,
        r.y,
        r.right() - left.right() - 16.0,
        r.h,
    );
    if right.w <= 120.0 {
        return done;
    }

    if let Some(mut custom) = core::weather::custom_weather(Some(&f.state.choice.weather)) {
        let mut changed = false;
        let mut back = false;
        f.ui.scroll_area("ps-custom-weather", right, &mut |ui, v| {
            let mut yy = v.y + 4.0;
            if ui.button(
                "ps-weather-back",
                Rect::new(v.x, yy, v.w - 8.0, 42.0),
                "Choose another weather",
                Some("arrow_back"),
                ButtonKind::Normal,
            ) {
                back = true;
            }
            yy += 52.0;
            changed |= ui.slider(
                "ps-custom-vis",
                Rect::new(v.x, yy, v.w - 8.0, 40.0),
                &mut custom.visibility_m,
                50.0,
                50_000.0,
                50.0,
                "Visibility",
                &|x| {
                    if x >= 49_950.0 {
                        "unlimited".into()
                    } else if x >= 1000.0 {
                        format!("{:.1} km", x / 1000.0)
                    } else {
                        format!("{x:.0} m")
                    }
                },
            );
            yy += 46.0;
            changed |= ui.slider(
                "ps-custom-bright",
                Rect::new(v.x, yy, v.w - 8.0, 40.0),
                &mut custom.brightness,
                0.0,
                1.5,
                0.05,
                "Brightness",
                &|x| format!("{:.0} %", x * 100.0),
            );
            yy += 46.0;
            changed |= ui.slider(
                "ps-custom-wdir",
                Rect::new(v.x, yy, v.w - 8.0, 40.0),
                &mut custom.wind_dir,
                0.0,
                355.0,
                5.0,
                "Wind direction",
                &|x| format!("{x:.0}°"),
            );
            yy += 46.0;
            changed |= ui.slider(
                "ps-custom-wspeed",
                Rect::new(v.x, yy, v.w - 8.0, 40.0),
                &mut custom.wind_speed,
                0.0,
                40.0,
                0.5,
                "Wind speed",
                &|x| format!("{x:.1} m/s"),
            );
            yy += 46.0;
            changed |= ui.slider(
                "ps-custom-temp",
                Rect::new(v.x, yy, v.w - 8.0, 40.0),
                &mut custom.temp_c,
                -30.0,
                45.0,
                1.0,
                "Temperature",
                &|x| format!("{x:.0} °C"),
            );
            yy += 46.0;
            let t = custom.temp_c;
            changed |= ui.slider(
                "ps-custom-hum",
                Rect::new(v.x, yy, v.w - 8.0, 40.0),
                &mut custom.humidity,
                0.0,
                100.0,
                1.0,
                "Humidity",
                &|x| format!("{x:.0} % · dew {:.0} °C", dew_point_of_relative(t, x)),
            );
            yy += 50.0;

            ui.label(Rect::new(v.x, yy, 120.0, 38.0), "Cloud type");
            let cloud_labels: Vec<String> = core::weather::CUSTOM_CLOUDS
                .iter()
                .map(|x| (*x).to_string())
                .collect();
            let mut cloud = custom.cloud;
            if ui.select(
                "ps-custom-cloud",
                Rect::new(v.x + 122.0, yy, v.w - 130.0, 38.0),
                &mut cloud,
                &cloud_labels,
            ) {
                custom.cloud = cloud;
                changed = true;
            }
            yy += 48.0;

            ui.label(Rect::new(v.x, yy, 120.0, 38.0), "Precipitation");
            let precip_labels: Vec<String> = core::weather::CUSTOM_PRECIP
                .iter()
                .map(|x| (*x).to_string())
                .collect();
            let mut precip = custom.precip.clamp(0, 2) as usize;
            if ui.select(
                "ps-custom-precip",
                Rect::new(v.x + 122.0, yy, v.w - 130.0, 38.0),
                &mut precip,
                &precip_labels,
            ) {
                custom.precip = precip as i32;
                changed = true;
            }
            yy += 48.0;

            changed |= ui.slider(
                "ps-custom-intensity",
                Rect::new(v.x, yy, v.w - 8.0, 40.0),
                &mut custom.precip_intensity,
                0.0,
                255.0,
                1.0,
                "Precipitation intensity",
                &|x| format!("{x:.0} / 255"),
            );
            yy += 46.0;
            changed |= ui.slider(
                "ps-custom-wet",
                Rect::new(v.x, yy, v.w - 8.0, 40.0),
                &mut custom.road_wetness,
                0.0,
                1.0,
                0.05,
                "Road wetness",
                &|x| format!("{:.0} %", x * 100.0),
            );
            yy += 46.0;

            let mut snow = custom.snow_cover;
            if ui.toggle(
                "ps-custom-snow",
                Rect::new(v.x, yy, v.w - 8.0, 38.0),
                &mut snow,
                "Snow cover",
            ) {
                custom.snow_cover = snow;
                changed = true;
            }
            yy += 44.0;
            let mut snow_road = custom.snow_on_road;
            if ui.toggle(
                "ps-custom-snowroad",
                Rect::new(v.x, yy, v.w - 8.0, 38.0),
                &mut snow_road,
                "Snow on road",
            ) {
                custom.snow_on_road = snow_road;
                changed = true;
            }
            yy += 48.0;
            yy - v.y
        });
        if back {
            f.state.choice.weather.clear();
            f.state.touched();
        } else if changed {
            custom.normalize();
            f.state.choice.weather = custom.encode();
            f.state.touched();
        }
        return done;
    }

    if let Some(code) = f
        .state
        .choice
        .weather
        .strip_prefix("metar:")
        .map(str::to_string)
    {
        let mut airport = code
            .to_ascii_uppercase()
            .chars()
            .filter(|c| c.is_ascii_alphabetic())
            .take(4)
            .collect::<String>();
        let airport_list =
            core::weather::metar_airports(std::path::Path::new(&f.state.config.root));
        let mut changed = false;
        let mut back = false;
        f.ui.scroll_area("ps-real-weather", right, &mut |ui, v| {
            let mut yy = v.y + 4.0;
            if ui.button(
                "ps-real-back",
                Rect::new(v.x, yy, v.w - 8.0, 42.0),
                "Choose another weather",
                Some("arrow_back"),
                ButtonKind::Normal,
            ) {
                back = true;
            }
            yy += 56.0;
            ui.heading(
                Rect::new(v.x, yy, v.w - 8.0, 28.0),
                "Current weather (METAR)",
                Some("public"),
            );
            yy += 38.0;
            ui.label(Rect::new(v.x, yy, 90.0, 42.0), "ICAO");
            if ui.text_input(
                "ps-metar-icao",
                Rect::new(v.x + 92.0, yy, v.w - 100.0, 42.0),
                &mut airport,
                "ICAO",
                None,
            ) {
                airport = airport
                    .chars()
                    .filter(|c| c.is_ascii_alphabetic())
                    .take(4)
                    .collect::<String>()
                    .to_ascii_uppercase();
                changed = true;
            }
            yy += 52.0;

            let labels: Vec<String> = airport_list.iter().map(|a| a.1.clone()).collect();
            if !labels.is_empty() {
                let mut sel = airport_list
                    .iter()
                    .position(|a| a.0.eq_ignore_ascii_case(&airport))
                    .unwrap_or(0);
                if ui.select(
                    "ps-metar-list",
                    Rect::new(v.x, yy, v.w - 8.0, 42.0),
                    &mut sel,
                    &labels,
                ) {
                    if let Some(a) = airport_list.get(sel) {
                        airport = a.0.clone();
                        changed = true;
                    }
                }
                yy += 52.0;
            }
            ui.paragraph(
                "The METAR is fetched when the game starts.",
                Vec2::new(v.x, yy),
                v.w - 8.0,
                12.0,
                Weight::Regular,
                TEXT_DIM,
            );
            yy += 48.0;
            yy - v.y
        });
        if back {
            f.state.choice.weather.clear();
            f.state.touched();
        } else if changed {
            f.state.choice.weather = format!("metar:{airport}");
            f.state.touched();
        }
        return done;
    }

    if let Some(custom) =
        crate::window::selected_weather_as_custom(&f.state.config.root, &f.state.choice.weather)
    {
        if f.ui.button(
            "ps-edit-current-weather",
            Rect::new(right.x, right.y, right.w, 42.0),
            "Edit selected weather as custom",
            Some("tune"),
            ButtonKind::Normal,
        ) {
            f.state.choice.weather = custom;
            f.state.touched();
            return done;
        }
    }

    let home = omsi_launcher_lib::start::nearest_airport(&f.state.config.root, &f.state.choice.map);
    let custom = core::weather::CustomWeather::default().encode();
    let items: Vec<(String, String, String)> = [
        (
            String::new(),
            "Natural weather".to_string(),
            "Develops by itself through the day and the season".to_string(),
        ),
        (
            custom,
            "Custom weather".to_string(),
            "Visibility, wind, temperature, rain, snow and road state".to_string(),
        ),
        (
            format!("metar:{home}"),
            "Current weather".to_string(),
            format!("Real weather from {home} (ICAO can be changed)"),
        ),
        (
            "cycle".to_string(),
            "Weather cycle".to_string(),
            "Changes every 25-60 minutes, as the month allows".to_string(),
        ),
    ]
    .into_iter()
    .chain(f.state.weathers.iter().map(|w| {
        (
            w.file.clone(),
            w.name.clone(),
            format!(
                "{:.0} °C · {} · {}",
                w.temp,
                if w.clouds.is_empty() {
                    "clear"
                } else {
                    w.clouds.as_str()
                },
                if w.precip.is_empty() {
                    "dry"
                } else {
                    w.precip.as_str()
                }
            ),
        )
    }))
    .collect();

    let chosen = f.state.choice.weather.clone();
    let mut pick = None;
    f.ui.scroll_area("ps-weather", right, &mut |ui, v| {
        for (k, (file, name, sub)) in items.iter().enumerate() {
            let rr = Rect::new(v.x, v.y + k as f32 * (ROW_H + 6.0), v.w - 8.0, ROW_H);
            if big_row(ui, &format!("pw-{k}"), rr, name, sub, *file == chosen, None) {
                pick = Some(file.clone());
            }
        }
        items.len() as f32 * (ROW_H + 6.0)
    });
    if let Some(file) = pick {
        f.state.choice.weather = file;
        f.state.touched();
    }
    done
}

fn more(f: &mut Frame, pv: &mut PhoneView, body: Rect) -> Option<Page> {
    let mut chosen = None;
    let inner = body.pad(12.0, 12.0);
    let cols = if inner.w > 700.0 { 4 } else { 2 };
    let rows = MORE.len().div_ceil(cols);
    let gap = 10.0;
    let tw = (inner.w - gap * (cols as f32 - 1.0)) / cols as f32;
    let th = ((inner.h - gap * (rows as f32 - 1.0)) / rows as f32).clamp(64.0, 120.0);
    for (k, (page, name, icon, sub)) in MORE.iter().enumerate() {
        let (c, rw) = (k % cols, k / cols);
        let r = Rect::new(
            inner.x + (tw + gap) * c as f32,
            inner.y + (th + gap) * rw as f32,
            tw,
            th,
        );
        let (h, down, clicked) = f.ui.interact(id_of(&format!("pmore-{name}")), r);
        f.ui.p().rounded(
            r,
            14.0,
            if down {
                SELECTED
            } else if h {
                HOVER
            } else {
                FIELD
            },
        );
        f.ui.icon(icon, Vec2::new(r.x + 28.0, r.y + 28.0), 24.0, accent());
        f.ui.text_in(
            name,
            Rect::new(r.x + 16.0, r.bottom() - 46.0, r.w - 24.0, 20.0),
            15.0,
            Weight::Bold,
            TEXT,
            Align::Left,
        );
        f.ui.text_in(
            sub,
            Rect::new(r.x + 16.0, r.bottom() - 26.0, r.w - 24.0, 18.0),
            11.5,
            Weight::Regular,
            TEXT_DIM,
            Align::Left,
        );
        if clicked {
            pv.page = Some(*page);
            chosen = Some(*page);
        }
    }
    chosen
}
