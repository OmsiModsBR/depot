//! The start, on the Maps page's right: the date and the time (typed, or from the calendar and
//! the clock), the kind of session (new, a saved situation, or without a bus), where to start
//! and the weather - those two in windows over the page - and the button that starts. The
//! line and the bus are chosen inside the game, as in OMSI 2; a bus picked on the Vehicles
//! page is put down at the start (it can be cleared here).
//!
//! The windows: the start points (a list and the map, whose pins are clicked too), the weather
//! (natural, or one chosen), and the map to explore (its radar view, read-only).

use super::super::domain::frame::Frame;
use super::super::domain::map::Map;
use super::super::domain::start::*;
use super::super::interface::output::{core, State};
use super::dialog::{dialog, dialog_foot};
use super::drive::{map_look, weather_picker};
use super::map::map_pointer;
use crate::state::date_shown;
use glam::Vec2;
use omsi_ui::paint::Align;
use omsi_ui::theme::*;
use omsi_ui::widgets::{id_of, ButtonKind, Key};
use omsi_ui::{Rect, Weight};

/// The kinds of session, as `Choice::session` names them, and their labels.
const SESSIONS: [(&str, &str); 3] = [
    ("new", "New"),
    ("saved", "Saved"),
    ("without_bus", "Without bus"),
];

/// The weather chosen, in words.
fn weather_label(f: &Frame) -> String {
    let w = &f.state.choice.weather;
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
    if core::weather::custom_weather(Some(w)).is_some() {
        return "Custom weather".into();
    }
    f.state
        .weathers
        .iter()
        .find(|x| x.file == *w)
        .map(|x| x.name.clone())
        .unwrap_or_else(|| crate::state::short_map(w))
}

/// The start point chosen, in words.
fn entry_label(f: &Frame) -> String {
    let e = f.state.choice.entry;
    let Some(m) = f.state.map() else {
        return "Automatic".into();
    };
    match usize::try_from(e).ok().and_then(|i| m.entry_points.get(i)) {
        Some(p) if !p.name.is_empty() => p.name.clone(),
        Some(p) => omsi_ui::tr("Start point %{n}").replace("%{n}", &(p.index + 1).to_string()),
        None => "Automatic (the map's first)".into(),
    }
}

/// A small label over a field.
fn label(f: &mut Frame, text: &str, x: f32, y: f32, w: f32) {
    f.ui.text_in(
        text,
        Rect::new(x, y, w, 18.0),
        12.5,
        Weight::Regular,
        TEXT_DIM,
        Align::Left,
    );
}

/// A field that follows the computer's clock (nothing to type): what it shows, quiet.
fn clock_field(f: &mut Frame, r: Rect, text: &str) {
    f.ui.p().rounded(r, CONTROL_RADIUS, FIELD.alpha(0.5));
    f.ui.p().rounded_border(r, CONTROL_RADIUS, 1.0, EDGE);
    f.ui.icon(
        "o-clock",
        Vec2::new(r.x + 18.0, r.center().y),
        15.0,
        TEXT_DIM,
    );
    f.ui.text_in(
        text,
        Rect::new(r.x + 36.0, r.y, r.w - 44.0, r.h),
        13.0,
        Weight::Regular,
        TEXT_DIM,
        Align::Left,
    );
}

/// A field that opens a window: an icon, what is chosen, an arrow. True when clicked.
fn opener(f: &mut Frame, id: &str, r: Rect, icon: &str, text: &str) -> bool {
    let (h, _, clicked) = f.ui.interact_key(id_of(id), r);
    let t = f.ui.anim(id_of(id), if h { 1.0 } else { 0.0 }, 0.08);
    f.ui.p().rounded(r, CONTROL_RADIUS, FIELD.mix(HOVER, t));
    f.ui.p().rounded_border(r, CONTROL_RADIUS, 1.0, EDGE);
    f.ui.icon(icon, Vec2::new(r.x + 18.0, r.center().y), 16.0, TEXT_SOFT);
    let room = r.w - 36.0 - 30.0;
    f.ui.text_in_tip(
        text,
        Rect::new(r.x + 36.0, r.y, room, r.h),
        13.5,
        Weight::Regular,
        TEXT,
        Align::Left,
    );
    f.ui.icon(
        "o-chevron",
        Vec2::new(r.right() - 16.0, r.center().y),
        13.0,
        TEXT_DIM,
    );
    clicked
}

/// The start's card in `r` (the Maps page's right column).
pub fn card(f: &mut Frame, sv: &mut StartView, r: Rect) {
    crate::kit::card(f.ui, r, "Start", None);
    let x = r.x + 17.0;
    let w = r.w - 34.0;
    let mut y = r.y + 46.0;
    // the date: typed, or from the calendar beside it
    label(f, "Date", x, y, w);
    y += 20.0;
    let shown = date_shown(&f.state.choice.date);
    let date_id = id_of("start-date");
    if f.ui.focus != Some(date_id)
        && date_typed(&sv.date_text).as_deref() != Some(f.state.choice.date.as_str())
    {
        sv.date_text = shown;
    }
    // (Settings can have a session start on today's date, at the real time: the fields then
    // follow the computer's clock, and say so, instead of taking what is typed)
    let follows = |state: &State, k: &str| {
        state
            .settings
            .get(k)
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    };
    if follows(f.state, "use_real_date") {
        clock_field(f, Rect::new(x, y, w, 34.0), "Today's date (Settings)");
    } else {
        // (the calendar inside the field, at its right end, as Windows' date field has it)
        let mut text = std::mem::take(&mut sv.date_text);
        if f.ui.text_input_end(
            "start-date",
            Rect::new(x, y, w, 34.0),
            &mut text,
            "dd/mm/yyyy",
            None,
            34.0,
        ) {
            if let Some(iso) = date_typed(&text) {
                f.state.choice.date = iso;
                f.state.choice.season = "auto".into();
                f.state.touched();
            }
        }
        sv.date_text = text;
        let mut d = f.state.choice.date.clone();
        if f.ui.calendar_button(
            "start-calendar",
            Rect::new(x + w - 32.0, y + 3.0, 28.0, 28.0),
            &mut d,
        ) {
            f.state.choice.date = d;
            f.state.choice.season = "auto".into();
            f.state.touched();
        }
    }
    y += 44.0;
    // the time: typed, or the computer's clock
    label(f, "Time", x, y, w);
    y += 20.0;
    let shown = format!(
        "{:02}:{:02}",
        f.state.choice.time / 60,
        f.state.choice.time % 60
    );
    let time_id = id_of("start-time");
    if f.ui.focus != Some(time_id) && time_typed(&sv.time_text) != Some(f.state.choice.time) {
        sv.time_text = shown;
    }
    if follows(f.state, "use_real_time") {
        clock_field(f, Rect::new(x, y, w, 34.0), "The real time (Settings)");
    } else {
        let mut text = std::mem::take(&mut sv.time_text);
        if f.ui.text_input(
            "start-time",
            Rect::new(x, y, w - 78.0, 34.0),
            &mut text,
            "hh:mm",
            None,
        ) {
            if let Some(t) = time_typed(&text) {
                f.state.choice.time = t;
                f.state.touched();
            }
        }
        sv.time_text = text;
        if f.ui.button(
            "start-now",
            Rect::new(x + w - 70.0, y, 70.0, 34.0),
            "Now",
            None,
            ButtonKind::Normal,
        ) {
            // (the clock's time only: the date chosen stays)
            if let Some((_, _, _, h, m)) = omsi_launcher_lib::local_now() {
                f.state.choice.time = h * 60 + m;
                f.state.touched();
            }
        }
    }
    y += 44.0;
    // the kind of session
    label(f, "Session", x, y, w);
    y += 20.0;
    let mut k = SESSIONS
        .iter()
        .position(|s| s.0 == f.state.choice.session)
        .unwrap_or(0);
    let labels: Vec<&str> = SESSIONS.iter().map(|s| s.1).collect();
    if f.ui
        .segmented("start-session", Rect::new(x, y, w, 34.0), &mut k, &labels)
    {
        f.state.choice.session = SESSIONS[k].0.to_string();
        f.state.touched();
    }
    y += 46.0;
    let session = SESSIONS[k].0;
    if session == "saved" {
        label(f, "Saved situation", x, y, w);
        y += 20.0;
        let list: Vec<String> = f
            .state
            .saved_situations()
            .iter()
            .map(|s| s.name.clone())
            .collect();
        if list.is_empty() {
            f.ui.paragraph("Nothing saved on this map yet. The game keeps where you left off when a session ends, and its save slots.", Vec2::new(x, y + 12.0), w, 12.5, Weight::Regular, TEXT_DIM);
        } else {
            let mut pick = f.state.save_pick.min(list.len() - 1);
            if f.ui
                .select("start-saved", Rect::new(x, y, w, 34.0), &mut pick, &list)
            {
                f.state.save_pick = pick;
            }
        }
    } else {
        label(f, "Start point", x, y, w);
        y += 20.0;
        let e = entry_label(f);
        if opener(f, "start-entry", Rect::new(x, y, w, 34.0), "o-pin", &e) {
            open(f, sv, Popup::StartPoint);
        }
        y += 44.0;
        label(f, "Weather", x, y, w);
        y += 20.0;
        let wl = weather_label(f);
        if opener(f, "start-weather", Rect::new(x, y, w, 34.0), "o-cloud", &wl) {
            open(f, sv, Popup::Weather);
        }
    }
    // at the foot: the bus picked on the Vehicles page (a new session puts it down), and the
    // button that starts
    let go = Rect::new(x, r.bottom() - 17.0 - 44.0, w, 44.0);
    let mut foot = go.y - 12.0;
    if session != "saved" {
        let bus = f
            .state
            .bus()
            .map(|b| omsi_launcher_lib::display_bus_name(&b.name))
            .filter(|_| !f.state.choice.bus.is_empty());
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
            let hh = f.ui.paragraph_height(hint, tw, 12.5, Weight::Regular);
            // (`paragraph` leaves a line's spacing under the last line: less padding there)
            let note_h = hh + pad * 2.0 - 4.0;
            let note = Rect::new(x, foot - note_h, w, note_h);
            foot = note.y;
            f.ui.p()
                .rounded(note, CONTROL_RADIUS, omsi_ui::theme::surface(FIELD));
            f.ui.p().rounded_border(note, CONTROL_RADIUS, 1.0, EDGE);
            f.ui.icon(
                icon,
                Vec2::new(x + pad + 9.0, note.y + pad + 8.0),
                16.0,
                TEXT_DIM,
            );
            f.ui.paragraph(
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
            // (two lines: the phrase, and the model's name under it - a long name no longer
            // pushes the phrase out of the row)
            foot -= 44.0;
            f.ui.icon("o-bus", Vec2::new(x + 8.0, foot + 18.0), 15.0, TEXT_DIM);
            let clear_w = 44.0;
            let room = w - 22.0 - clear_w - 8.0;
            let phrase = omsi_ui::tr("Preselected bus:");
            for (row, text) in [&*phrase, &*name].into_iter().enumerate() {
                let line = crate::kit::fit_text(f.ui, text, 12.5, room);
                f.ui.text_in(
                    &line,
                    Rect::new(x + 22.0, foot + row as f32 * 18.0, room, 18.0),
                    12.5,
                    Weight::Regular,
                    TEXT_SOFT,
                    Align::Left,
                );
            }
            let clear = Rect::new(x + w - clear_w, foot + 5.0, clear_w, 26.0);
            let (h, _, clicked) = f.ui.interact_key(id_of("start-clear-bus"), clear);
            f.ui.text_in(
                "Clear",
                clear,
                12.5,
                Weight::Medium,
                if h { accent().lighten(0.15) } else { accent() },
                Align::Right,
            );
            f.ui.tooltip(clear, "Clear");
            if clicked {
                // (the whole preselection goes: no depot file, number or plate picked for it
                // stays behind for the game)
                f.state.clear_bus();
            }
        }
    }
    let _ = foot;
    // (without a usable OMSI 2 folder, or with a game running - one at a time - the button
    // is dimmed and does nothing; the InfoBar at the foot of the page says why)
    let games = f.state.instances.iter().any(|i| i.running);
    if !f.root.ok(&f.state.config.root) || games {
        f.ui.p()
            .rounded(go, CONTROL_RADIUS, omsi_ui::theme::surface(FIELD));
        f.ui.text_in(
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
        f.ui.tooltip(go, tip);
    } else if f.ui.button(
        "start-go",
        go,
        "Start driving",
        Some("o-play"),
        ButtonKind::Primary,
    ) {
        start(f);
    }
}

pub fn open(f: &mut Frame, sv: &mut StartView, p: Popup) {
    sv.undo = Some((f.state.choice.entry, f.state.choice.weather.clone()));
    sv.search.clear();
    sv.choose = !f.state.choice.weather.is_empty();
    sv.popup = Some(p);
}

fn close(f: &mut Frame, sv: &mut StartView, keep: bool) {
    if let (false, Some((entry, weather))) = (keep, sv.undo.take()) {
        if (entry, &weather) != (f.state.choice.entry, &f.state.choice.weather) {
            f.state.choice.entry = entry;
            f.state.choice.weather = weather;
            f.state.touched();
        }
    }
    sv.undo = None;
    sv.popup = None;
}

/// Start the session the card describes.
fn start(f: &mut Frame) {
    if f.state.map().is_none() {
        f.state.set_status("Choose a map first.", true);
        return;
    }
    match f.state.choice.session.as_str() {
        "saved" => {
            f.state.choice.save();
            f.state.launch_last_situation();
        }
        kind => {
            // (the line is chosen in the game; without a bus picked, or without one at all,
            // the player starts on foot at the start point and places one with the ALT menu -
            // the bus picked on Vehicles stays for the next start)
            f.state.choice.free = true;
            let bus = kind == "new" && !f.state.choice.bus.is_empty() && f.state.bus().is_some();
            f.state.choice.on_foot = !bus;
            f.state.choice.save();
            if bus {
                f.state.launch();
            } else {
                let hof = f.state.default_hof();
                f.state
                    .launch_with(|d| crate::state::without_bus(d, Some(hof)));
            }
        }
    }
}

/// The window open over the page, if any (the page under it sees no mouse meanwhile).
pub fn popup(f: &mut Frame, sv: &mut StartView, mv: &mut dyn Map) {
    let Some(p) = sv.popup else { return };
    if f.ui.input.keys.contains(&Key::Escape) {
        close(f, sv, false);
        return;
    }
    match p {
        Popup::StartPoint => start_point(f, sv, mv),
        Popup::Weather => weather(f, sv),
        Popup::Explore => explore(f, sv, mv),
    }
}

/// The hint over a map's lower left corner.
fn map_hint(f: &mut Frame, map: Rect, text: &str) {
    let w = f.ui.width(text, 12.0, Weight::Regular) + 24.0;
    let chip = Rect::new(map.x + 12.0, map.bottom() - 12.0 - 26.0, w, 26.0);
    f.ui.p().rounded(chip, 13.0, PANEL.alpha(0.92));
    f.ui.p().rounded_border(chip, 13.0, 1.0, EDGE);
    f.ui.text_in(text, chip, 12.0, Weight::Regular, TEXT_SOFT, Align::Center);
}

fn start_point(f: &mut Frame, sv: &mut StartView, mv: &mut dyn Map) {
    let name = f
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
    let size = f.ui.size;
    let inside = dialog(
        f.ui,
        1040.0,
        (size.y - 96.0).min(680.0),
        "Choose the start point",
        &name,
    );
    let (buttons, body) = dialog_foot(f.ui, inside);
    let list_w = 300.0_f32.min(body.w * 0.4);
    let map = Rect::new(
        body.x + list_w + 16.0,
        body.y,
        body.w - list_w - 16.0,
        body.h,
    );
    // the map first: the list and the buttons lie beside it, the pins are clicked after
    mv.want(map_look(f.state));
    mv.background(f.ui, map);
    f.ui.p().rounded_border(map, RADIUS, 1.0, EDGE);
    map_hint(f, map, "Scroll to zoom · drag to move · click a pin");
    let mut search = std::mem::take(&mut sv.search);
    f.ui.text_input(
        "start-entry-search",
        Rect::new(body.x, body.y, list_w, 34.0),
        &mut search,
        "Search start points...",
        Some("o-search"),
    );
    let entries: Vec<(i32, String)> = f
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
    sv.search = search;
    let chosen = f.state.choice.entry;
    let mut picked = None;
    let area = Rect::new(body.x - 4.0, body.y + 44.0, list_w + 8.0, body.h - 44.0);
    f.ui.scroll_area("start-entry-list", area, &mut |ui, v| {
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
        f.state.choice.entry = i;
        f.state.touched();
    }
    if f.ui.button(
        "start-entry-cancel",
        Rect::new(buttons.right() - 196.0 - 10.0 - 90.0, buttons.y, 90.0, 34.0),
        "Cancel",
        None,
        ButtonKind::Normal,
    ) {
        close(f, sv, false);
        return;
    }
    if f.ui.button(
        "start-entry-use",
        Rect::new(buttons.right() - 196.0, buttons.y, 196.0, 34.0),
        "Use this start point",
        None,
        ButtonKind::Primary,
    ) {
        close(f, sv, true);
        return;
    }
    map_pointer(f, mv, map, map, false);
}

fn weather(f: &mut Frame, sv: &mut StartView) {
    let size = f.ui.size;
    let inside = dialog(
        f.ui,
        620.0,
        (size.y - 96.0).min(720.0),
        "Weather and conditions",
        "Applies to this start",
    );
    let (buttons, body) = dialog_foot(f.ui, inside);
    let mut k = usize::from(sv.choose);
    if f.ui.segmented(
        "start-weather-kind",
        Rect::new(body.x, body.y, body.w, 34.0),
        &mut k,
        &["Natural", "Choose"],
    ) {
        sv.choose = k == 1;
        if k == 0 {
            f.state.choice.weather.clear();
            f.state.touched();
        }
    }
    let rest = Rect::new(body.x, body.y + 48.0, body.w, (body.h - 48.0).max(0.0));
    if sv.choose {
        weather_picker(f, rest);
    } else {
        f.ui.paragraph(
            "The weather develops by itself through the day and the season: clouds come and go, rain falls and the roads dry again.",
            Vec2::new(rest.x, rest.y + 14.0),
            rest.w,
            13.0,
            Weight::Regular,
            TEXT_DIM,
        );
    }
    if f.ui.button(
        "start-weather-cancel",
        Rect::new(buttons.right() - 100.0 - 10.0 - 90.0, buttons.y, 90.0, 34.0),
        "Cancel",
        None,
        ButtonKind::Normal,
    ) {
        close(f, sv, false);
        return;
    }
    if f.ui.button(
        "start-weather-apply",
        Rect::new(buttons.right() - 100.0, buttons.y, 100.0, 34.0),
        "Apply",
        None,
        ButtonKind::Primary,
    ) {
        close(f, sv, true);
    }
}

fn explore(f: &mut Frame, sv: &mut StartView, mv: &mut dyn Map) {
    let name = f
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
    let size = f.ui.size;
    let inside = dialog(
        f.ui,
        (size.x - 120.0).min(1240.0),
        size.y - 96.0,
        "Explore the map",
        &omsi_ui::tr("%{name} · the map's own radar view, read from its files")
            .replace("%{name}", &name),
    );
    let (buttons, body) = dialog_foot(f.ui, inside);
    let side_w = 240.0;
    let map = Rect::new(body.x, body.y, body.w - side_w - 16.0, body.h);
    mv.want(map_look(f.state));
    mv.background(f.ui, map);
    f.ui.p().rounded_border(map, RADIUS, 1.0, EDGE);
    map_hint(f, map, "Scroll to zoom · drag to move");
    let sx = map.right() + 16.0;
    let mut y = body.y;
    f.ui.heading(Rect::new(sx, y, side_w, 26.0), "On this map", None);
    y += 34.0;
    let counts = mv.counts();
    let rows = [
        ("Streets", counts.map(|c| c.0.to_string())),
        ("Stops", counts.map(|c| c.1.to_string())),
        // (as the map's list names them: the start point window offers the same)
        (
            "Start points",
            f.state.map().map(|m| m.entry_points.len().to_string()),
        ),
    ];
    for (k, v) in rows {
        f.ui.text_in(
            k,
            Rect::new(sx, y, side_w, 22.0),
            13.0,
            Weight::Regular,
            TEXT_DIM,
            Align::Left,
        );
        f.ui.text_in(
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
    let nh = f.ui.paragraph_height(note, side_w, 12.0, Weight::Regular);
    f.ui.paragraph(
        note,
        Vec2::new(sx, body.bottom() - nh),
        side_w,
        12.0,
        Weight::Regular,
        TEXT_DIM,
    );
    if f.ui.button(
        "start-explore-close",
        Rect::new(buttons.right() - 90.0, buttons.y, 90.0, 34.0),
        "Close",
        None,
        ButtonKind::Normal,
    ) {
        close(f, sv, false);
        return;
    }
    // (looking only: a pin clicked here does not change the start)
    let entry = f.state.choice.entry;
    map_pointer(f, mv, map, map, false);
    if f.state.choice.entry != entry {
        f.state.choice.entry = entry;
    }
}
