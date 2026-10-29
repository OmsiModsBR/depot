//! The start, on the Maps page's right: the date and the time (typed, or from the calendar and
//! the clock), the kind of session (new, a saved situation, or without a bus), where to start
//! and the weather - those two in windows over the page - and the button that starts. The
//! line and the bus are chosen inside the game, as in OMSI 2; a bus picked on the Vehicles
//! page is put down at the start (it can be cleared here).
//!
//! The windows: the start points (a list and the map, whose pins are clicked too), the weather
//! (natural, or one chosen), and the map to explore (its radar view, read-only).

use super::theme::*;
use super::ui::{id_of, ButtonKind, Key};
use super::Launcher;
use glam::Vec2;
use omsi_ui::paint::Align;
use omsi_ui::{Rect, Weight};

/// A window over the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Popup {
    StartPoint,
    Weather,
    Explore,
}

/// The kinds of session, as `Choice::session` names them, and their labels.
const SESSIONS: [(&str, &str); 3] = [
    ("new", "New"),
    ("saved", "Saved"),
    ("without_bus", "Without bus"),
];

#[derive(Default)]
pub struct StartView {
    pub popup: Option<Popup>,
    /// What the window changes, as it was when it opened (Cancel puts it back): the start
    /// point and the weather.
    undo: Option<(i32, String)>,
    /// The start points' search.
    search: String,
    /// The weather window shows the choice (a weather chosen, or "Choose" clicked).
    choose: bool,
    /// The date and the time as typed (put back to the choice's when it changes elsewhere).
    date_text: String,
    time_text: String,
}

/// `2024-06-14` as `14/06/2024`.
pub fn date_shown(iso: &str) -> String {
    let (y, m, d) = omsi_ui::widgets::parse_date(iso);
    format!("{d:02}/{m:02}/{y:04}")
}

/// A typed date (`14/06/2024`, `14.6.2024`, or `2024-06-14`) as `2024-06-14`; None when it is
/// no date.
pub fn date_typed(text: &str) -> Option<String> {
    let t = text.trim();
    let parts: Vec<&str> = t.split(['/', '.', '-']).map(str::trim).collect();
    if parts.len() != 3 {
        return None;
    }
    let n: Vec<i32> = parts
        .iter()
        .map(|p| p.parse().ok())
        .collect::<Option<_>>()?;
    let (y, m, d) = if parts[0].len() == 4 {
        (n[0], n[1], n[2])
    } else {
        (n[2], n[1], n[0])
    };
    let days = match m {
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        1..=12 => 31,
        _ => return None,
    };
    ((1000..=9999).contains(&y) && (1..=days).contains(&d)).then(|| format!("{y:04}-{m:02}-{d:02}"))
}

/// A typed time (`08:30`, `8.30`, `830`, `8`) as minutes of the day; None when it is no time.
pub fn time_typed(text: &str) -> Option<i32> {
    let t = text.trim();
    let (h, m) = match t.split_once([':', '.', 'h']) {
        Some((h, m)) => (
            h.trim().parse::<i32>().ok()?,
            if m.trim().is_empty() {
                0
            } else {
                m.trim().parse::<i32>().ok()?
            },
        ),
        None if t.len() >= 3 && t.chars().all(|c| c.is_ascii_digit()) => (
            t[..t.len() - 2].parse().ok()?,
            t[t.len() - 2..].parse().ok()?,
        ),
        None => (t.parse().ok()?, 0),
    };
    ((0..24).contains(&h) && (0..60).contains(&m)).then_some(h * 60 + m)
}

/// The weather chosen, in words.
fn weather_label(l: &Launcher) -> String {
    let w = &l.state.choice.weather;
    if w.is_empty() {
        return "Natural weather".into();
    }
    if let Some(code) = w.strip_prefix("metar:") {
        return if code.is_empty() {
            "Current weather".into()
        } else {
            omsi_ui::tr("Current weather (%{code})").replace("%{code}", &code)
        };
    }
    if w == "cycle" {
        return "Weather cycle".into();
    }
    if crate::weather_setup::custom_weather(Some(w)).is_some() {
        return "Custom weather".into();
    }
    l.state
        .weathers
        .iter()
        .find(|x| x.file == *w)
        .map(|x| x.name.clone())
        .unwrap_or_else(|| super::state::short_map(w))
}

/// The start point chosen, in words.
fn entry_label(l: &Launcher) -> String {
    let e = l.state.choice.entry;
    let Some(m) = l.state.map() else {
        return "Automatic".into();
    };
    match usize::try_from(e).ok().and_then(|i| m.entry_points.get(i)) {
        Some(p) if !p.name.is_empty() => p.name.clone(),
        Some(p) => omsi_ui::tr("Start point %{n}").replace("%{n}", &(p.index + 1).to_string()),
        None => "Automatic (the map's first)".into(),
    }
}

/// A small label over a field.
fn label(l: &mut Launcher, text: &str, x: f32, y: f32, w: f32) {
    l.ui.text_in(
        text,
        Rect::new(x, y, w, 18.0),
        12.5,
        Weight::Regular,
        TEXT_DIM,
        Align::Left,
    );
}

/// A field that follows the computer's clock (nothing to type): what it shows, quiet.
fn clock_field(l: &mut Launcher, r: Rect, text: &str) {
    l.ui.p().rounded(r, CONTROL_RADIUS, FIELD.alpha(0.5));
    l.ui.p().rounded_border(r, CONTROL_RADIUS, 1.0, EDGE);
    l.ui.icon(
        "o-clock",
        Vec2::new(r.x + 18.0, r.center().y),
        15.0,
        TEXT_DIM,
    );
    l.ui.text_in(
        text,
        Rect::new(r.x + 36.0, r.y, r.w - 44.0, r.h),
        13.0,
        Weight::Regular,
        TEXT_DIM,
        Align::Left,
    );
}

/// A field that opens a window: an icon, what is chosen, an arrow. True when clicked.
fn opener(l: &mut Launcher, id: &str, r: Rect, icon: &str, text: &str) -> bool {
    let (h, _, clicked) = l.ui.interact_key(id_of(id), r);
    let t = l.ui.anim(id_of(id), if h { 1.0 } else { 0.0 }, 0.08);
    l.ui.p().rounded(r, CONTROL_RADIUS, FIELD.mix(HOVER, t));
    l.ui.p().rounded_border(r, CONTROL_RADIUS, 1.0, EDGE);
    l.ui.icon(icon, Vec2::new(r.x + 18.0, r.center().y), 16.0, TEXT_SOFT);
    let room = r.w - 36.0 - 30.0;
    l.ui.text_in_tip(
        text,
        Rect::new(r.x + 36.0, r.y, room, r.h),
        13.5,
        Weight::Regular,
        TEXT,
        Align::Left,
    );
    l.ui.icon(
        "o-chevron",
        Vec2::new(r.right() - 16.0, r.center().y),
        13.0,
        TEXT_DIM,
    );
    clicked
}

/// The start's card in `r` (the Maps page's right column).
pub fn card(l: &mut Launcher, r: Rect) {
    omsi_launcher_ui::kit::card(&mut l.ui, r, "Start", None);
    let x = r.x + 17.0;
    let w = r.w - 34.0;
    let mut y = r.y + 46.0;
    // the date: typed, or from the calendar beside it
    label(l, "Date", x, y, w);
    y += 20.0;
    let shown = date_shown(&l.state.choice.date);
    let date_id = id_of("start-date");
    if l.ui.focus != Some(date_id)
        && date_typed(&l.start.date_text).as_deref() != Some(l.state.choice.date.as_str())
    {
        l.start.date_text = shown;
    }
    // (Settings can have a session start on today's date, at the real time: the fields then
    // follow the computer's clock, and say so, instead of taking what is typed)
    let follows = |l: &Launcher, k: &str| {
        l.state
            .settings
            .get(k)
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    };
    if follows(l, "use_real_date") {
        clock_field(l, Rect::new(x, y, w, 34.0), "Today's date (Settings)");
    } else {
        // (the calendar inside the field, at its right end, as Windows' date field has it)
        let mut text = std::mem::take(&mut l.start.date_text);
        if l.ui.text_input_end(
            "start-date",
            Rect::new(x, y, w, 34.0),
            &mut text,
            "dd/mm/yyyy",
            None,
            34.0,
        ) {
            if let Some(iso) = date_typed(&text) {
                l.state.choice.date = iso;
                l.state.choice.season = "auto".into();
                l.state.touched();
            }
        }
        l.start.date_text = text;
        let mut d = l.state.choice.date.clone();
        if l.ui.calendar_button(
            "start-calendar",
            Rect::new(x + w - 32.0, y + 3.0, 28.0, 28.0),
            &mut d,
        ) {
            l.state.choice.date = d;
            l.state.choice.season = "auto".into();
            l.state.touched();
        }
    }
    y += 44.0;
    // the time: typed, or the computer's clock
    label(l, "Time", x, y, w);
    y += 20.0;
    let shown = format!(
        "{:02}:{:02}",
        l.state.choice.time / 60,
        l.state.choice.time % 60
    );
    let time_id = id_of("start-time");
    if l.ui.focus != Some(time_id) && time_typed(&l.start.time_text) != Some(l.state.choice.time) {
        l.start.time_text = shown;
    }
    if follows(l, "use_real_time") {
        clock_field(l, Rect::new(x, y, w, 34.0), "The real time (Settings)");
    } else {
        let mut text = std::mem::take(&mut l.start.time_text);
        if l.ui.text_input(
            "start-time",
            Rect::new(x, y, w - 78.0, 34.0),
            &mut text,
            "hh:mm",
            None,
        ) {
            if let Some(t) = time_typed(&text) {
                l.state.choice.time = t;
                l.state.touched();
            }
        }
        l.start.time_text = text;
        if l.ui.button(
            "start-now",
            Rect::new(x + w - 70.0, y, 70.0, 34.0),
            "Now",
            None,
            ButtonKind::Normal,
        ) {
            // (the clock's time only: the date chosen stays)
            if let Some((_, _, _, h, m)) = omsi_launcher_lib::local_now() {
                l.state.choice.time = h * 60 + m;
                l.state.touched();
            }
        }
    }
    y += 44.0;
    // the kind of session
    label(l, "Session", x, y, w);
    y += 20.0;
    let mut k = SESSIONS
        .iter()
        .position(|s| s.0 == l.state.choice.session)
        .unwrap_or(0);
    let labels: Vec<&str> = SESSIONS.iter().map(|s| s.1).collect();
    if l.ui
        .segmented("start-session", Rect::new(x, y, w, 34.0), &mut k, &labels)
    {
        l.state.choice.session = SESSIONS[k].0.to_string();
        l.state.touched();
    }
    y += 46.0;
    let session = SESSIONS[k].0;
    if session == "saved" {
        label(l, "Saved situation", x, y, w);
        y += 20.0;
        let list: Vec<String> = l
            .state
            .saved_situations()
            .iter()
            .map(|s| s.name.clone())
            .collect();
        if list.is_empty() {
            l.ui.paragraph("Nothing saved on this map yet. The game keeps where you left off when a session ends, and its save slots.", Vec2::new(x, y + 12.0), w, 12.5, Weight::Regular, TEXT_DIM);
        } else {
            let mut pick = l.state.save_pick.min(list.len() - 1);
            if l.ui
                .select("start-saved", Rect::new(x, y, w, 34.0), &mut pick, &list)
            {
                l.state.save_pick = pick;
            }
        }
    } else {
        label(l, "Start point", x, y, w);
        y += 20.0;
        let e = entry_label(l);
        if opener(l, "start-entry", Rect::new(x, y, w, 34.0), "o-pin", &e) {
            open(l, Popup::StartPoint);
        }
        y += 44.0;
        label(l, "Weather", x, y, w);
        y += 20.0;
        let wl = weather_label(l);
        if opener(l, "start-weather", Rect::new(x, y, w, 34.0), "o-cloud", &wl) {
            open(l, Popup::Weather);
        }
    }
    // at the foot: the bus picked on the Vehicles page (a new session puts it down), and the
    // button that starts
    let go = Rect::new(x, r.bottom() - 17.0 - 44.0, w, 44.0);
    let mut foot = go.y - 12.0;
    if session != "saved" {
        let bus = l
            .state
            .bus()
            .map(|b| omsi_launcher_lib::display_bus_name(&b.name))
            .filter(|_| !l.state.choice.bus.is_empty());
        if bus.is_none() || session != "new" {
            let (hint, icon) = if session == "new" {
                (
                    "No vehicle selected. Add or change vehicles in the game with the ALT menu.",
                    "o-bus",
                )
            } else {
                (
                    "You start on foot at the start point; place a vehicle in the game with the ALT menu.",
                    "o-user",
                )
            };
            // (a quiet note over the button, in a box of its own, with room between them -
            // the lines used to touch the button)
            let pad = 10.0;
            let tx = x + pad + 26.0;
            let tw = w - (tx - x) - pad;
            let hh = l.ui.paragraph_height(hint, tw, 12.5, Weight::Regular);
            // (`paragraph` leaves a line's spacing under the last line: less padding there)
            let note_h = hh + pad * 2.0 - 4.0;
            let note = Rect::new(x, foot - note_h, w, note_h);
            foot = note.y;
            l.ui.p()
                .rounded(note, CONTROL_RADIUS, omsi_ui::theme::surface(FIELD));
            l.ui.p().rounded_border(note, CONTROL_RADIUS, 1.0, EDGE);
            l.ui.icon(
                icon,
                Vec2::new(x + pad + 9.0, note.y + pad + 8.0),
                16.0,
                TEXT_DIM,
            );
            l.ui.paragraph(
                hint,
                Vec2::new(tx, note.y + pad - 2.0),
                tw,
                12.5,
                Weight::Regular,
                TEXT_DIM,
            );
            foot -= 10.0;
        }
        if let (Some(name), "new") = (bus, session) {
            foot -= 26.0;
            l.ui.icon("o-bus", Vec2::new(x + 8.0, foot + 9.0), 15.0, TEXT_DIM);
            let clear_w = 44.0;
            let room = w - 22.0 - clear_w - 8.0;
            let line = omsi_launcher_ui::kit::fit_text(
                &mut l.ui,
                &omsi_ui::tr("Bus preselected in Vehicles: %{name}").replace("%{name}", &name),
                12.5,
                room,
            );
            l.ui.text_in(
                &line,
                Rect::new(x + 22.0, foot, room, 18.0),
                12.5,
                Weight::Regular,
                TEXT_SOFT,
                Align::Left,
            );
            let clear = Rect::new(x + w - clear_w, foot - 4.0, clear_w, 26.0);
            let (h, _, clicked) = l.ui.interact_key(id_of("start-clear-bus"), clear);
            l.ui.text_in(
                "Clear",
                clear,
                12.5,
                Weight::Medium,
                if h { accent().lighten(0.15) } else { accent() },
                Align::Right,
            );
            l.ui.tooltip(clear, "Clear");
            if clicked {
                // (the whole preselection goes: no depot file, number or plate picked for it
                // stays behind for the game)
                l.state.clear_bus();
            }
        }
    }
    let _ = foot;
    // (without a usable OMSI 2 folder, or with a game running - one at a time - the button
    // is dimmed and does nothing; the InfoBar at the foot of the page says why)
    let games = l.state.instances.iter().any(|i| i.running);
    if !l.root_ok() || games {
        l.ui.p()
            .rounded(go, CONTROL_RADIUS, omsi_ui::theme::surface(FIELD));
        l.ui.text_in(
            "Start driving",
            go,
            14.5,
            Weight::Bold,
            TEXT_FAINT,
            Align::Center,
        );
        let tip = if games {
            "Game: running"
        } else {
            "OMSI 2 folder: not chosen"
        };
        l.ui.tooltip(go, tip);
    } else if l.ui.button(
        "start-go",
        go,
        "Start driving",
        Some("o-play"),
        ButtonKind::Primary,
    ) {
        start(l);
    }
}

pub(super) fn open(l: &mut Launcher, p: Popup) {
    l.start.undo = Some((l.state.choice.entry, l.state.choice.weather.clone()));
    l.start.search.clear();
    l.start.choose = !l.state.choice.weather.is_empty();
    l.start.popup = Some(p);
}

fn close(l: &mut Launcher, keep: bool) {
    if let (false, Some((entry, weather))) = (keep, l.start.undo.take()) {
        if (entry, &weather) != (l.state.choice.entry, &l.state.choice.weather) {
            l.state.choice.entry = entry;
            l.state.choice.weather = weather;
            l.state.touched();
        }
    }
    l.start.undo = None;
    l.start.popup = None;
}

/// Start the session the card describes.
fn start(l: &mut Launcher) {
    if l.state.map().is_none() {
        l.state.set_status("Choose a map first.", true);
        return;
    }
    match l.state.choice.session.as_str() {
        "saved" => {
            l.state.choice.save();
            l.state.launch_last_situation();
        }
        kind => {
            // (the line is chosen in the game; without a bus picked, or without one at all,
            // the player starts on foot at the start point and places one with the ALT menu -
            // the bus picked on Vehicles stays for the next start)
            l.state.choice.free = true;
            let bus = kind == "new" && !l.state.choice.bus.is_empty() && l.state.bus().is_some();
            l.state.choice.on_foot = !bus;
            l.state.choice.save();
            if bus {
                l.state.launch();
            } else {
                let hof = l.state.default_hof();
                l.state
                    .launch_with(|d| super::state::without_bus(d, Some(hof)));
            }
        }
    }
}

/// How a dialog darkens the page under it: as Windows 11's, enough to set the dialog apart,
/// the page under it still seen (at 0.62 it was almost hidden).
pub(super) const SCRIM: omsi_ui::Color = omsi_ui::Color::rgba(0, 0, 0, 0.45);

/// The dialog's frame, `w` x `h` in the middle of the window, over the page darkened: its
/// title and a note at the right of it. Returns its inside, under the title.
pub(super) fn dialog(l: &mut Launcher, w: f32, h: f32, title: &str, note: &str) -> Rect {
    let size = l.ui.size;
    let full = Rect::new(0.0, 0.0, size.x, size.y);
    l.ui.solid(full);
    l.ui.p().rect(full, SCRIM);
    let (w, h) = (w.min(size.x - 48.0), h.min(size.y - 48.0));
    let r = Rect::new((size.x - w) * 0.5, (size.y - h) * 0.5, w, h);
    l.ui.p().rounded(r, RADIUS, PANEL);
    l.ui.p().rounded_border(r, RADIUS, 1.0, EDGE);
    l.ui.text_in(
        title,
        Rect::new(r.x + 20.0, r.y + 14.0, r.w - 40.0, 30.0),
        18.0,
        Weight::Bold,
        TEXT,
        Align::Left,
    );
    l.ui.text_in(
        note,
        Rect::new(r.x + 20.0, r.y + 14.0, r.w - 40.0, 30.0),
        12.5,
        Weight::Regular,
        TEXT_DIM,
        Align::Right,
    );
    Rect::new(r.x + 20.0, r.y + 56.0, r.w - 40.0, r.h - 56.0)
}

/// The dialog's foot (a line over it) at the bottom of `inside`; returns the row its buttons
/// sit in, and the inside above it.
pub(super) fn dialog_foot(l: &mut Launcher, inside: Rect) -> (Rect, Rect) {
    let foot = Rect::new(inside.x, inside.bottom() - 62.0, inside.w, 62.0);
    l.ui.p().rect(
        Rect::new(inside.x - 20.0, foot.y, inside.w + 40.0, 1.0),
        EDGE,
    );
    let body = Rect::new(
        inside.x,
        inside.y,
        inside.w,
        (foot.y - 14.0 - inside.y).max(0.0),
    );
    (Rect::new(foot.x, foot.y + 14.0, foot.w, 34.0), body)
}

/// The window open over the page, if any (the page under it sees no mouse meanwhile).
pub fn popup(l: &mut Launcher) {
    let Some(p) = l.start.popup else { return };
    if l.ui.input.keys.contains(&Key::Escape) {
        close(l, false);
        return;
    }
    match p {
        Popup::StartPoint => start_point(l),
        Popup::Weather => weather(l),
        Popup::Explore => explore(l),
    }
}

/// The hint over a map's lower left corner.
fn map_hint(l: &mut Launcher, map: Rect, text: &str) {
    let w = l.ui.width(text, 12.0, Weight::Regular) + 24.0;
    let chip = Rect::new(map.x + 12.0, map.bottom() - 12.0 - 26.0, w, 26.0);
    l.ui.p().rounded(chip, 13.0, PANEL.alpha(0.92));
    l.ui.p().rounded_border(chip, 13.0, 1.0, EDGE);
    l.ui.text_in(text, chip, 12.0, Weight::Regular, TEXT_SOFT, Align::Center);
}

fn start_point(l: &mut Launcher) {
    let name = l
        .state
        .map()
        .map(|m| {
            if m.friendly.trim().is_empty() {
                m.name.clone()
            } else {
                m.friendly.clone()
            }
        })
        .unwrap_or_default();
    let size = l.ui.size;
    let inside = dialog(
        l,
        1040.0,
        (size.y - 96.0).min(680.0),
        "Choose the start point",
        &name,
    );
    let (buttons, body) = dialog_foot(l, inside);
    let list_w = 300.0_f32.min(body.w * 0.4);
    let map = Rect::new(
        body.x + list_w + 16.0,
        body.y,
        body.w - list_w - 16.0,
        body.h,
    );
    // the map first: the list and the buttons lie beside it, the pins are clicked after
    l.mapview.want(super::drive::map_look(l));
    l.map_background(map);
    l.ui.p().rounded_border(map, RADIUS, 1.0, EDGE);
    map_hint(l, map, "Scroll to zoom · drag to move · click a pin");
    let mut search = std::mem::take(&mut l.start.search);
    l.ui.text_input(
        "start-entry-search",
        Rect::new(body.x, body.y, list_w, 34.0),
        &mut search,
        "Search start points...",
        Some("o-search"),
    );
    let entries: Vec<(i32, String)> = l
        .state
        .map()
        .map(|m| {
            m.entry_points
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    (
                        i as i32,
                        if e.name.is_empty() {
                            omsi_ui::tr("Start point %{n}")
                                .replace("%{n}", &(e.index + 1).to_string())
                        } else {
                            e.name.clone()
                        },
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let rows: Vec<(i32, String)> = std::iter::once((-1, "Automatic (the map's first)".to_string()))
        .chain(
            entries
                .into_iter()
                .filter(|(_, n)| entry_matches(n, &search)),
        )
        .collect();
    l.start.search = search;
    let chosen = l.state.choice.entry;
    let mut picked = None;
    let area = Rect::new(body.x - 4.0, body.y + 44.0, list_w + 8.0, body.h - 44.0);
    l.ui.scroll_area("start-entry-list", area, &mut |ui, v| {
        let mut y = v.y;
        for (i, name) in &rows {
            let row = Rect::new(v.x + 4.0, y, v.w - 12.0, 46.0);
            if ui.row(&format!("start-entry-{i}"), row, *i == chosen) {
                picked = Some(*i);
            }
            ui.icon(
                "o-pin",
                Vec2::new(row.x + 18.0, row.center().y),
                16.0,
                if *i == chosen { accent() } else { TEXT_DIM },
            );
            ui.text_in(
                name,
                Rect::new(row.x + 38.0, row.y, row.w - 46.0, row.h),
                13.5,
                Weight::Medium,
                TEXT,
                Align::Left,
            );
            y += 50.0;
        }
        y - v.y
    });
    if let Some(i) = picked {
        l.state.choice.entry = i;
        l.state.touched();
    }
    if l.ui.button(
        "start-entry-cancel",
        Rect::new(buttons.right() - 196.0 - 10.0 - 90.0, buttons.y, 90.0, 34.0),
        "Cancel",
        None,
        ButtonKind::Normal,
    ) {
        close(l, false);
        return;
    }
    if l.ui.button(
        "start-entry-use",
        Rect::new(buttons.right() - 196.0, buttons.y, 196.0, 34.0),
        "Use this start point",
        None,
        ButtonKind::Primary,
    ) {
        close(l, true);
        return;
    }
    l.map_interact_in_window(map, map);
}

fn weather(l: &mut Launcher) {
    let size = l.ui.size;
    let inside = dialog(
        l,
        620.0,
        (size.y - 96.0).min(720.0),
        "Weather and conditions",
        "Applies to this start",
    );
    let (buttons, body) = dialog_foot(l, inside);
    let mut k = usize::from(l.start.choose);
    if l.ui.segmented(
        "start-weather-kind",
        Rect::new(body.x, body.y, body.w, 34.0),
        &mut k,
        &["Natural", "Choose"],
    ) {
        l.start.choose = k == 1;
        if k == 0 {
            l.state.choice.weather.clear();
            l.state.touched();
        }
    }
    let rest = Rect::new(body.x, body.y + 48.0, body.w, (body.h - 48.0).max(0.0));
    if l.start.choose {
        super::drive::weather_picker(l, rest);
    } else {
        l.ui.paragraph(
            "The weather develops by itself through the day and the season: clouds come and go, rain falls and the roads dry again.",
            Vec2::new(rest.x, rest.y + 14.0),
            rest.w,
            13.0,
            Weight::Regular,
            TEXT_DIM,
        );
    }
    if l.ui.button(
        "start-weather-cancel",
        Rect::new(buttons.right() - 100.0 - 10.0 - 90.0, buttons.y, 90.0, 34.0),
        "Cancel",
        None,
        ButtonKind::Normal,
    ) {
        close(l, false);
        return;
    }
    if l.ui.button(
        "start-weather-apply",
        Rect::new(buttons.right() - 100.0, buttons.y, 100.0, 34.0),
        "Apply",
        None,
        ButtonKind::Primary,
    ) {
        close(l, true);
    }
}

fn explore(l: &mut Launcher) {
    let name = l
        .state
        .map()
        .map(|m| {
            if m.friendly.trim().is_empty() {
                m.name.clone()
            } else {
                m.friendly.clone()
            }
        })
        .unwrap_or_default();
    let size = l.ui.size;
    let inside = dialog(
        l,
        (size.x - 120.0).min(1240.0),
        size.y - 96.0,
        "Explore the map",
        &omsi_ui::tr("%{name} · the map's own radar view, read from its files")
            .replace("%{name}", &name),
    );
    let (buttons, body) = dialog_foot(l, inside);
    let side_w = 240.0;
    let map = Rect::new(body.x, body.y, body.w - side_w - 16.0, body.h);
    l.mapview.want(super::drive::map_look(l));
    l.map_background(map);
    l.ui.p().rounded_border(map, RADIUS, 1.0, EDGE);
    map_hint(l, map, "Scroll to zoom · drag to move");
    let sx = map.right() + 16.0;
    let mut y = body.y;
    l.ui.heading(Rect::new(sx, y, side_w, 26.0), "On this map", None);
    y += 34.0;
    let counts = l.mapview.counts();
    let rows = [
        ("Streets", counts.map(|c| c.0.to_string())),
        ("Stops", counts.map(|c| c.1.to_string())),
        // (as the map's list names them: the start point window offers the same)
        (
            "Start points",
            l.state.map().map(|m| m.entry_points.len().to_string()),
        ),
    ];
    for (k, v) in rows {
        l.ui.text_in(
            k,
            Rect::new(sx, y, side_w, 22.0),
            13.0,
            Weight::Regular,
            TEXT_DIM,
            Align::Left,
        );
        l.ui.text_in(
            v.as_deref().unwrap_or("…"),
            Rect::new(sx, y, side_w, 22.0),
            13.0,
            Weight::Medium,
            TEXT,
            Align::Right,
        );
        y += 28.0;
    }
    let note = "Read-only: nothing here changes the map's files or your start. Streets, stops and start points show only what the map provides.";
    let nh = l.ui.paragraph_height(note, side_w, 12.0, Weight::Regular);
    l.ui.paragraph(
        note,
        Vec2::new(sx, body.bottom() - nh),
        side_w,
        12.0,
        Weight::Regular,
        TEXT_DIM,
    );
    if l.ui.button(
        "start-explore-close",
        Rect::new(buttons.right() - 90.0, buttons.y, 90.0, 34.0),
        "Close",
        None,
        ButtonKind::Normal,
    ) {
        close(l, false);
        return;
    }
    // (looking only: a pin clicked here does not change the start)
    let entry = l.state.choice.entry;
    l.map_interact_in_window(map, map);
    if l.state.choice.entry != entry {
        l.state.choice.entry = entry;
    }
}

/// Whether a start point's name shows for the typed search (case does not matter).
pub fn entry_matches(name: &str, search: &str) -> bool {
    let q = search.trim().to_lowercase();
    q.is_empty() || super::ui::matches(name, &q)
}
