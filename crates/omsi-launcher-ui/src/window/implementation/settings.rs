//! The Settings page (the Windows 11 concept): every option in a section on the left, in cards
//! of a few rows on the right, searchable; each row says what the option does and when a change
//! takes effect, marks a change from the default and takes it back.

use super::super::domain::frame::Frame;
use super::super::domain::settings::*;
use super::super::interface::output::{core, Choice, Flow, Host};
use super::dialog::SCRIM;
use super::heading::page_title_with;
use super::omsi2_modules::module_card;
use super::omsi2_plugins::plugins_card as omsi2_plugins_card;
use crate::Page as UiPage;
use glam::Vec2;
use omsi_launcher_lib::settings::d3d9_fixes_vanilla;
use omsi_ui::paint::Align;
use omsi_ui::theme::*;
use omsi_ui::widgets::{id_of, ButtonKind, Ui};
use omsi_ui::{Color, Rect, Weight};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::HashMap;

const ROW_H: f32 = 50.0;
const HEAD_H: f32 = 44.0;

/// The page's cards: a group of options, or one of the page's own.
enum Card<'a> {
    Group(String, Vec<&'a Opt>),
    Api(&'a Opt),
    /// The accent colour as swatches (a search lists it as a dropdown row).
    Accent(&'a Opt),
    Own(&'static str),
}

impl Card<'_> {
    fn wide(&self) -> bool {
        matches!(
            self,
            Card::Api(_) | Card::Own("plugins" | "performance" | "omsi2-plugins")
        )
    }

    fn height(&self) -> f32 {
        match self {
            Card::Group(_, rows) => HEAD_H + rows.len() as f32 * ROW_H + 6.0,
            Card::Api(_) => 148.0,
            Card::Accent(_) => 312.0,
            Card::Own(name) => OWN_H.with(|h| h.borrow().get(name).copied().unwrap_or(200.0)),
        }
    }
}

thread_local! {
    /// How high each of the page's own cards came out the frame before.
    static OWN_H: RefCell<HashMap<&'static str, f32>> = RefCell::new(HashMap::new());
}

/// The cards of what is listed, in the order of the table (a search: "Section · Group").
fn cards<'a>(opts: &[&'a Opt], sec: usize, mixed: bool, omsi2: bool) -> Vec<Card<'a>> {
    let mut out: Vec<Card> = Vec::new();
    for o in opts {
        if !mixed && matches!(o.kind, Kind::Api) {
            out.push(Card::Api(o));
            continue;
        }
        if !mixed && o.key == "ui_accent" {
            out.push(Card::Accent(o));
            continue;
        }
        let title = if sec == PERFORMANCE && !mixed {
            PERFORMANCE_OPTS
                .iter()
                .find(|(k, _)| *k == o.key)
                .map_or(o.group, |(_, g)| g)
                .to_string()
        } else if mixed {
            format!("{} · {}", SECTIONS[o.sec], o.group)
        } else {
            o.group.to_string()
        };
        match out
            .iter_mut()
            .find(|c| matches!(c, Card::Group(t, _) if *t == title))
        {
            Some(Card::Group(_, rows)) => rows.push(o),
            _ => out.push(Card::Group(title, vec![o])),
        }
    }
    if sec == PERFORMANCE && !mixed {
        out.insert(0, Card::Own("performance"));
    }
    if !mixed {
        let own: &[&str] = match sec {
            SOUND => &["radio"],
            DEVICES => &["controls"],
            // (in OMSI 2 mode: its own plug-ins, each switched on or off)
            PLUGINS if omsi2 => &["omsi2-plugins"],
            PLUGINS => &["plugins"],
            PRESETS => &["oop", "profiles", "files"],
            INTERFACE => &["preview"],
            MODULES if omsi2 => &["omsi-prefetch", "openhud"],
            _ => &[],
        };
        // (the Depot's own radio stations, plug-ins and profiles are not OMSI 2's)
        out.extend(
            own.iter()
                .filter(|n| !omsi2 || !matches!(**n, "radio" | "plugins" | "profiles"))
                .map(|n| Card::Own(n)),
        );
    }
    out
}

pub fn settings(f: &mut Frame, view: &mut SettingsView, area: Rect) -> Outside {
    // (Modules goes with OMSI 2 mode: back to the first section without it)
    if !section_shown(view.sec, &f.state.settings, f.state.omsi2_engine()) {
        view.sec = GENERAL;
    }
    let sec = view.sec.min(SECTIONS.len() - 1);
    let (_, title, line) = UiPage::Settings.heading().unwrap_or_default();
    let searching = !view.query.trim().is_empty();
    let place = if searching {
        "Search"
    } else if view.changed_only {
        "Changed"
    } else {
        SECTIONS[sec]
    };
    let body = page_title_with(f.ui, area, UiPage::Settings.title(), (place, title, line));
    let root = std::path::PathBuf::from(&f.state.config.root);
    let omsi2 = f.state.omsi2_engine();
    let mut start = start_values(&f.state.choice);
    let mut out = Outside::default();
    out.game_runs = f.state.in_game();
    out.modules.idle = !f.state.in_game() && matches!(f.state.original.flow, Flow::Idle);
    let narrow = body.w < 760.0;
    let ui = &mut *f.ui;
    // (in OMSI 2 mode the options OMSI 2 has are shown from, and kept in, its own file: the
    // page works on a copy of the settings with those laid over, taken apart again below)
    let data = omsi_launcher_lib::data_dir();
    let mut shown_settings = f.state.settings.clone();
    if omsi2 {
        let saved = view.omsi2_saved.get_or_insert_with(|| {
            omsi_launcher_lib::original_engine::config::load_settings_in(&data)
        });
        crate::omsi2_map::overlay(&mut shown_settings, saved);
    } else {
        view.omsi2_saved = None;
    }
    let before = shown_settings.clone();
    let mut st = Store {
        s: &mut shown_settings,
        start: &mut start,
        changed: false,
        start_changed: false,
        omsi2,
        adapter_mb: f.host.adapter_texture_mb(),
        ceiling_mb: f.host.texture_ceiling_mb(),
    };
    // the bar: the search, all or the changed ones, how many are changed, the presets
    let bar_h = 34.0;
    // (each section's rows and whether one is changed, once a frame: the list and the chip use them)
    let mut per_sec = [(0usize, false); SECTIONS.len()];
    let mut n_changed = 0;
    if performance_offered(st.s, omsi2) {
        for k in PERFORMANCE_OPTS
            .iter()
            .filter_map(|(k, _)| OPTS.iter().find(|o| o.key == *k))
        {
            if shown(k, &st) != Shown::Hidden {
                per_sec[PERFORMANCE].0 += 1;
                per_sec[PERFORMANCE].1 |= shown(k, &st) == Shown::Row && changed(k, &st);
            }
        }
    }
    for o in OPTS {
        if shown(o, &st) == Shown::Row {
            let c = changed(o, &st);
            per_sec[o.sec].0 += 1;
            per_sec[o.sec].1 |= c;
            n_changed += c as usize;
        } else if matches!(shown(o, &st), Shown::Locked(_)) {
            per_sec[o.sec].0 += 1;
        }
    }
    let sw = if narrow {
        body.w
    } else {
        (body.w * 0.4).min(520.0)
    };
    ui.text_input(
        "set-search",
        Rect::new(body.x, body.y, sw, bar_h),
        &mut view.query,
        "Search every option...",
        Some("search"),
    );
    let mut show = view.changed_only as usize;
    let (seg, mut y) = if narrow {
        (
            Rect::new(body.x, body.y + bar_h + 10.0, 200.0, bar_h),
            body.y + (bar_h + 10.0) * 2.0,
        )
    } else {
        (
            Rect::new(body.x + sw + 12.0, body.y, 200.0, bar_h),
            body.y + bar_h + 14.0,
        )
    };
    if ui.segmented("set-show", seg, &mut show, &["All", "Changed"]) {
        view.changed_only = show == 1;
    }
    if !narrow {
        let pw = ui.width("Presets", 13.0, Weight::Bold) + 48.0;
        if ui.button(
            "set-presets",
            Rect::new(body.right() - pw, body.y, pw, bar_h),
            "Presets",
            Some("inventory_2"),
            ButtonKind::Normal,
        ) {
            view.sec = PRESETS;
            view.query.clear();
            view.changed_only = false;
        }
        let chip = if n_changed == 1 {
            "1 changed from default".to_string()
        } else {
            omsi_ui::tr("%{n} changed from default").replace("%{n}", &n_changed.to_string())
        };
        let cw = ui.width(&chip, 12.0, Weight::Regular) + 24.0;
        let cr = Rect::new(body.right() - pw - 10.0 - cw, body.y + 4.0, cw, 26.0);
        ui.p().rounded(cr, 13.0, Color::WHITE.alpha(0.04));
        ui.p().rounded_border(cr, 13.0, 1.0, EDGE);
        ui.text_in(&chip, cr, 12.0, Weight::Regular, TEXT_DIM, Align::Center);
    }
    let q = view.query.clone();
    let mixed = !q.trim().is_empty() || view.changed_only;
    // the sections: a list on the left, a dropdown on a narrow window
    let content = if narrow {
        let shown: Vec<usize> = (0..SECTIONS.len())
            .filter(|&i| section_shown(i, st.s, omsi2))
            .collect();
        let mut s = shown.iter().position(|&i| i == sec).unwrap_or(0);
        let names: Vec<String> = shown.iter().map(|&i| SECTIONS[i].to_string()).collect();
        if ui.select(
            "set-section",
            Rect::new(
                seg.right() + 10.0,
                seg.y,
                (body.right() - seg.right() - 10.0).max(120.0),
                bar_h,
            ),
            &mut s,
            &names,
        ) {
            view.sec = shown[s];
            view.query.clear();
            view.changed_only = false;
        }
        Rect::new(body.x, y, body.w, (body.bottom() - y).max(0.0))
    } else {
        let lw = crate::kit::fit(230.0, body.w);
        let nav = Rect::new(body.x, y, lw, (body.bottom() - y).max(0.0));
        if let Some(s) = sections_card(ui, nav, sec, mixed, &per_sec, st.s, omsi2) {
            view.sec = s;
            view.query.clear();
            view.changed_only = false;
        }
        y = nav.y;
        Rect::new(
            nav.right() + GAP,
            y,
            body.right() - nav.right() - GAP,
            nav.h,
        )
    };
    let sec = view.sec.min(SECTIONS.len() - 1);
    let opts = listed(sec, &q, view.changed_only, &st);
    section_body(
        ui, content, sec, mixed, narrow, &opts, &mut st, &mut out, &root, f.host,
    );
    // (the launcher speaks the chosen language at once)
    f.host
        .set_language(st.get("language").as_str().unwrap_or("ENG"));
    let (mut changed, start_changed) = (st.changed, st.start_changed);
    if changed && omsi2 {
        let saved = view.omsi2_saved.get_or_insert_with(Default::default);
        let (kept, own) = crate::omsi2_map::store_back(&before, &shown_settings, saved);
        if kept {
            if let Err(e) =
                omsi_launcher_lib::original_engine::config::save_settings_in(&data, saved)
            {
                f.state.set_status(
                    omsi_ui::tr("OMSI 2 mode settings were not saved: %{error}")
                        .replace("%{error}", &format!("{e:#}")),
                    true,
                );
            }
        }
        changed = !own.is_empty();
        for (k, v) in own {
            f.state.settings[k] = v;
        }
    } else if changed {
        f.state.settings = shown_settings;
    }
    if changed {
        f.state.settings_dirty = 0.3;
    }
    if start_changed {
        // (a season chosen moves the date into it, as it always did)
        let season = start["season"].as_str().unwrap_or("auto").to_string();
        let new_season = season != f.state.choice.season;
        apply_start(&mut f.state.choice, &start);
        if new_season {
            f.state.set_season(&season);
        }
        f.state.touched();
    }
    if let Some(id) = out.modules.look {
        f.state.module_look(id);
    }
    if let Some(id) = out.modules.remove {
        f.state.module_ask_remove(id);
    }
    out
}

/// The section `sec` (or, `mixed`, what a search or "changed only" lists: `opts`) in `content`,
/// scrolled: its cards in two columns, one on a narrow window.
#[allow(clippy::too_many_arguments)]
pub fn section_body(
    ui: &mut Ui,
    content: Rect,
    sec: usize,
    mixed: bool,
    narrow: bool,
    opts: &[&Opt],
    st: &mut Store,
    out: &mut Outside,
    root: &std::path::Path,
    host: &dyn Host,
) {
    let id = format!("set-page-{sec}-{mixed}");
    ui.scroll_area(&id, content, &mut |ui, v| {
        let w = v.w - 8.0;
        let list = cards(opts, sec, mixed, st.omsi2);
        if list.is_empty() {
            let text = if mixed {
                "Nothing matches. A search looks through every section."
            } else {
                "Nothing to set here with the options chosen now."
            };
            ui.text_in(
                text,
                Rect::new(v.x, v.y, w, 30.0),
                13.0,
                Weight::Regular,
                TEXT_DIM,
                Align::Left,
            );
            return 30.0;
        }
        // (a card goes under the shorter column; a wide one across both)
        let cols = if narrow { 1 } else { 2 };
        let cw = (w - GAP * (cols - 1) as f32) / cols as f32;
        let mut tops = vec![v.y; cols];
        for c in &list {
            let h = c.height();
            let r = if c.wide() || cols == 1 {
                let top = tops.iter().cloned().fold(v.y, f32::max);
                tops.iter_mut().for_each(|t| *t = top + h + GAP);
                Rect::new(v.x, top, w, h)
            } else {
                let k = if tops[1] < tops[0] { 1 } else { 0 };
                let r = Rect::new(v.x + k as f32 * (cw + GAP), tops[k], cw, h);
                tops[k] += h + GAP;
                r
            };
            match c {
                Card::Group(title, rows) => group_card(ui, r, title, rows, st),
                Card::Api(o) => api_card(ui, r, o, st),
                Card::Accent(o) => accent_card(ui, r, o, st),
                Card::Own(name) => {
                    let used = own_card(ui, r, name, st, out, root, host);
                    OWN_H.with(|m| m.borrow_mut().insert(name, used));
                }
            }
        }
        tops.iter().cloned().fold(v.y, f32::max) - v.y - GAP
    });
}

/// The sections on the left, with how many options each shows (a dot: one of them changed).
/// Returns a section clicked.
fn sections_card(
    ui: &mut Ui,
    r: Rect,
    sec: usize,
    mixed: bool,
    per_sec: &[(usize, bool)],
    s: &Value,
    omsi2: bool,
) -> Option<usize> {
    ui.panel(r);
    let mut picked = None;
    let x = r.x + 12.0;
    let w = r.w - 24.0;
    let mut k = 0;
    for (i, name) in SECTIONS.iter().enumerate() {
        if !section_shown(i, s, omsi2) {
            continue;
        }
        let rr = Rect::new(x, r.y + 12.0 + k as f32 * 37.0, w, 33.0);
        k += 1;
        let on = i == sec && !mixed;
        let (hover, _, clicked) = ui.interact_key(id_of(&format!("set-sec-{i}")), rr);
        if on {
            ui.p().rounded(rr, 5.0, accent().alpha(0.14));
            ui.p().rounded_border(rr, 5.0, 1.0, accent().alpha(0.6));
        } else if hover {
            ui.p().rounded(rr, 5.0, Color::WHITE.alpha(0.05));
        }
        let name = crate::kit::fit_text(ui, name, 13.5, rr.w - 60.0);
        ui.text_in(
            &name,
            Rect::new(rr.x + 10.0, rr.y, rr.w - 50.0, rr.h),
            13.5,
            Weight::Medium,
            TEXT,
            Align::Left,
        );
        let (n, any_changed) = per_sec[i];
        if n > 0 {
            ui.text_in(
                &n.to_string(),
                Rect::new(rr.right() - 44.0, rr.y, 34.0, rr.h),
                12.0,
                Weight::Regular,
                TEXT_DIM,
                Align::Right,
            );
            if any_changed {
                ui.p()
                    .circle(Vec2::new(rr.right() - 52.0, rr.center().y), 3.0, accent());
            }
        }
        if clicked {
            picked = Some(i);
        }
    }
    let note = "Keys and controllers are on the Controls page.";
    let nh = ui.paragraph_height(note, w, 12.0, Weight::Regular);
    let ny = r.bottom() - 14.0 - nh;
    if ny > r.y + 12.0 + k as f32 * 37.0 {
        ui.paragraph(note, Vec2::new(x, ny), w, 12.0, Weight::Regular, TEXT_DIM);
    }
    picked
}

/// A card's frame and title, its count at the right. Returns the top of its body.
fn card_frame(ui: &mut Ui, r: Rect, title: &str, count: Option<usize>) -> f32 {
    ui.panel(r);
    let tw = if count.is_some() {
        r.w - 80.0
    } else {
        r.w - 32.0
    };
    crate::kit::heading(ui, Rect::new(r.x + 16.0, r.y + 6.0, tw, 30.0), title);
    if let Some(n) = count {
        ui.text_in(
            &n.to_string(),
            Rect::new(r.right() - 56.0, r.y + 8.0, 40.0, 24.0),
            12.0,
            Weight::Regular,
            TEXT_DIM,
            Align::Right,
        );
    }
    r.y + HEAD_H
}

fn group_card(ui: &mut Ui, r: Rect, title: &str, rows: &[&Opt], st: &mut Store) {
    let mut y = card_frame(ui, r, title, Some(rows.len()));
    for o in rows {
        option_row(ui, Rect::new(r.x, y, r.w, ROW_H), o, st);
        y += ROW_H;
    }
}

/// What the texture memory row says in place of its hint when the chosen amount is more than
/// the graphics adapter holds (the game then takes it down, see `memory::texture_budget`):
/// DirectX9 sees little of the card's memory, so a chosen amount can be above it.
fn texture_notice(o: &Opt, st: &Store) -> Option<String> {
    if !matches!(o.kind, Kind::TexMem) || st.ceiling_mb <= 0 {
        return None;
    }
    let chosen = st.get(o.key).as_i64().filter(|m| *m > 0)?;
    (chosen > st.ceiling_mb).then(|| {
        omsi_ui::tr("The graphics card holds about %{size}: the game uses that much")
            .replace("%{size}", &mb(st.ceiling_mb))
    })
}

/// One option: its name (a dot when changed), what it does and when it takes effect, its
/// control, and the way back to the default.
fn option_row(ui: &mut Ui, r: Rect, o: &Opt, st: &mut Store) {
    ui.p().rect(Rect::new(r.x, r.y, r.w, 1.0), EDGE);
    let locked = match shown(o, st) {
        Shown::Locked(t) => Some(t),
        _ => None,
    };
    let is_changed = locked.is_none() && changed(o, st);
    // (a switch takes little room: its name gets the rest)
    let cw = if matches!(o.kind, Kind::Toggle) && locked.is_none() {
        60.0
    } else {
        (r.w * 0.42).clamp(140.0, 210.0)
    };
    let ctrl = Rect::new(
        r.right() - 16.0 - 26.0 - cw,
        r.y + (r.h - 32.0) * 0.5,
        cw,
        32.0,
    );
    let room = ctrl.x - r.x - 16.0 - 12.0;
    let x = r.x + 16.0;
    let fade = if locked.is_some() { 0.5 } else { 1.0 };
    let label = crate::kit::fit_text(ui, o.label, 13.5, room - 12.0);
    ui.text_in(
        &label,
        Rect::new(x, r.y + 6.0, room, 20.0),
        13.5,
        Weight::Regular,
        TEXT.alpha(fade),
        Align::Left,
    );
    if is_changed {
        let lw = ui.width(&label, 13.5, Weight::Regular);
        ui.p()
            .circle(Vec2::new(x + lw + 9.0, r.y + 16.0), 3.0, accent());
    }
    // (the line under the name: what it does, then when a change takes effect - or why it is fixed)
    let (when, wc) = match &locked {
        Some(_) => ("Locked".to_string(), WARN),
        None => (
            omsi_ui::tr(o.when.text()).into_owned(),
            if o.when == When::Now { OK } else { TEXT_DIM },
        ),
    };
    let ww = ui.width(&when, 11.5, Weight::Regular);
    let hint = match &locked {
        Some(t) => t
            .split_once(" - ")
            .map(|x| x.1.to_string())
            .unwrap_or_default(),
        None => texture_notice(o, st).unwrap_or_else(|| omsi_ui::tr(o.hint).into_owned()),
    };
    let mut wx = x;
    if !hint.is_empty() {
        let h = crate::kit::fit_text(ui, &format!("{hint} · "), 11.5, (room - ww).max(20.0));
        ui.text_in(
            &h,
            Rect::new(x, r.y + 26.0, room, 16.0),
            11.5,
            Weight::Regular,
            TEXT_DIM.alpha(fade),
            Align::Left,
        );
        wx += ui.width(&h, 11.5, Weight::Regular);
    }
    ui.text_in(
        &when,
        Rect::new(wx, r.y + 26.0, ww + 4.0, 16.0),
        11.5,
        Weight::Regular,
        wc,
        Align::Left,
    );
    if let Some(t) = &locked {
        let value = t.split_once(" - ").map(|x| x.0).unwrap_or(t);
        ui.p().rounded(ctrl, 4.0, FIELD.alpha(0.6));
        ui.p().rounded_border(ctrl, 4.0, 1.0, EDGE);
        ui.text_in(
            value,
            Rect::new(ctrl.x + 12.0, ctrl.y, ctrl.w - 24.0, ctrl.h),
            13.0,
            Weight::Regular,
            TEXT_DIM,
            Align::Left,
        );
        ui.icon(
            "lock",
            Vec2::new(r.right() - 16.0 - 9.0, r.center().y),
            15.0,
            TEXT_DIM,
        );
        return;
    }
    let id = format!("opt-{}", o.key);
    match &o.kind {
        Kind::Toggle => {
            let mut v = st.on(o.key);
            if ui.toggle(&id, ctrl, &mut v, "") {
                st.set(o.key, json!(v));
                after(o.key, st);
            }
        }
        Kind::Slide {
            min,
            max,
            step,
            show,
            keep,
            typed,
        } => {
            let or = default_in(st, o.key)
                .and_then(|d| d.as_f64())
                .unwrap_or(*min as f64);
            let mut v = st.get(o.key).as_f64().unwrap_or(or) as f32;
            let mut moved = false;
            if *typed {
                // (the slider's own value text gives way to a field over it)
                moved |= ui.slider(&id, ctrl, &mut v, *min, *max, *step, "", &|_| String::new());
                // (right of the slider's own click area, which ends 50 px before the control)
                let fr = Rect::new(ctrl.right() - 48.0, ctrl.y + 2.0, 48.0, ctrl.h - 4.0);
                let name = format!("{id}-typed");
                let fid = omsi_ui::widgets::id_of(&name);
                let mut text = TYPED.with(|t| {
                    let t = t.borrow();
                    if ui.focus == Some(fid) && t.0 == fid {
                        t.1.clone()
                    } else {
                        show(v)
                    }
                });
                if ui.text_input(&name, fr, &mut text, "", None) {
                    TYPED.with(|t| *t.borrow_mut() = (fid, text.clone()));
                    let digits = text.trim().trim_end_matches('%').trim();
                    if let Ok(x) = digits.parse::<f32>() {
                        let x = if show(0.0).ends_with('%') && *max <= 5.0 {
                            x / 100.0
                        } else {
                            x
                        };
                        v = ((x / step.max(1e-6)).round() * step).clamp(*min, *max);
                        moved = true;
                    }
                }
            } else {
                moved |= ui.slider(&id, ctrl, &mut v, *min, *max, *step, "", &|x| show(x));
            }
            if moved {
                st.set(o.key, keep(v));
                after(o.key, st);
            }
        }
        _ => {
            let mut options = options_of(o, st);
            let mut sel = chosen(o, st, &mut options);
            let labels: Vec<String> = options.iter().map(|x| x.1.clone()).collect();
            if ui.select(&id, ctrl, &mut sel, &labels) {
                let value = options[sel].0.clone();
                choose(o, st, &value);
                after(o.key, st);
            }
        }
    }
    if is_changed
        && ui.icon_button(
            &format!("opt-reset-{}", o.key),
            Vec2::new(r.right() - 16.0 - 9.0, r.center().y),
            12.0,
            "restart_alt",
            "Back to the default",
        )
    {
        reset(o, st);
    }
}

thread_local! {
    /// The text being typed in a slider's field: which field, and what it holds.
    static TYPED: std::cell::RefCell<(omsi_ui::widgets::Id, String)> =
        const { std::cell::RefCell::new((0, String::new())) };
}

/// The graphics API first, as a card of its own across the page: it decides what the other
/// graphics options can do.
fn api_card(ui: &mut Ui, r: Rect, o: &Opt, st: &mut Store) {
    ui.panel(r);
    ui.p().rounded(r, RADIUS, accent().alpha(0.08));
    ui.p().rounded_border(r, RADIUS, 1.0, accent().alpha(0.55));
    let top = card_frame_plain(
        ui,
        r,
        "Graphics API",
        "decides what the other graphics options can do",
    );
    let fw = (r.w * 0.36).clamp(220.0, 320.0);
    let field = Rect::new(r.x + 16.0, top, fw, 40.0);
    let mut options = options_of(o, st);
    let mut sel = chosen(o, st, &mut options);
    let labels: Vec<String> = options.iter().map(|x| x.1.clone()).collect();
    if ui.select("opt-graphics_api", field, &mut sel, &labels) {
        let value = options[sel].0.clone();
        choose(o, st, &value);
    }
    let names = api_options()
        .iter()
        .map(|x| omsi_ui::tr(x.1).into_owned())
        .collect::<Vec<_>>()
        .join(" · ");
    let nh = ui.paragraph(
        &names,
        Vec2::new(field.x, field.bottom() + 8.0),
        fw,
        11.5,
        Weight::Regular,
        TEXT_DIM,
    );
    ui.text_in(
        "Next start",
        Rect::new(field.x, field.bottom() + 10.0 + nh, fw, 16.0),
        11.5,
        Weight::Regular,
        TEXT_DIM,
        Align::Left,
    );
    let tx = field.right() + 24.0;
    let text = if d3d9_fixes_vanilla(st.s) {
        "DirectX9 (D3D9Ex) is on. It is for compatibility and better performance on older hardware, and draws as close to the real OMSI 2 as it can: only the options OMSI 2 has, no improvements. What goes beyond OMSI 2 is hidden; what DirectX9 does not draw yet is shown locked. Your own choices come back when you switch API."
    } else {
        "The API you choose can limit other options. When one is not available with it, that option is locked and says why. Today only DirectX9 (D3D9Ex) limits options."
    };
    ui.paragraph(
        text,
        Vec2::new(tx, top + 2.0),
        (r.right() - 16.0 - tx).max(100.0),
        13.0,
        Weight::Regular,
        TEXT_SOFT,
    );
}

/// A card's title with a quiet note at its right. Returns the top of its body.
fn card_frame_plain(ui: &mut Ui, r: Rect, title: &str, note: &str) -> f32 {
    crate::kit::heading(ui, Rect::new(r.x + 16.0, r.y + 6.0, r.w * 0.5, 30.0), title);
    ui.text_in(
        note,
        Rect::new(r.x + r.w * 0.5, r.y + 8.0, r.w * 0.5 - 16.0, 24.0),
        12.0,
        Weight::Regular,
        TEXT_DIM,
        Align::Right,
    );
    r.y + HEAD_H
}

/// The page's own cards (radio stations, controls, plug-ins, presets and files). Returns the
/// height it needed.
fn own_card(
    ui: &mut Ui,
    r: Rect,
    name: &str,
    st: &mut Store,
    out: &mut Outside,
    root: &std::path::Path,
    host: &dyn Host,
) -> f32 {
    match name {
        "radio" => radio_stations(ui, r, host),
        "controls" => {
            let mut y = card_frame(ui, r, "Keys and controllers", None);
            let x = r.x + 16.0;
            let w = r.w - 32.0;
            y += ui.paragraph(
                "Which key or button does what is set on the Controls page.",
                Vec2::new(x, y),
                w,
                12.5,
                Weight::Regular,
                TEXT_DIM,
            ) + 10.0;
            if ui.button(
                "s-go-keys",
                Rect::new(x, y, w, 34.0),
                "Change the keys",
                Some("keyboard"),
                ButtonKind::Normal,
            ) {
                out.controls = Some(0);
            }
            y += 42.0;
            if ui.button(
                "s-go-pads",
                Rect::new(x, y, w, 34.0),
                "Set up a wheel or pedals",
                Some("sports_esports"),
                ButtonKind::Normal,
            ) {
                out.controls = Some(1);
            }
            y + 34.0 + 16.0 - r.y
        }
        "performance" => {
            let mut y = card_frame(ui, r, "About these options", None);
            y += ui.paragraph(
                "These options lower the load on the processor, the graphics chip and the memory. Some of them move the picture or the simulation away from OMSI 2, and are off until you choose them. They do not widen the hardware Depot supports; they may help in some cases, with no guarantee.",
                Vec2::new(r.x + 16.0, y),
                r.w - 32.0,
                12.5,
                Weight::Regular,
                TEXT_DIM,
            );
            y + 16.0 - r.y
        }
        "plugins" => plugins_card(ui, r, root),
        "omsi2-plugins" => omsi2_plugins_card(ui, r, root, out.game_runs),
        "omsi-prefetch" => module_card(ui, r, "omsi-prefetch", root, &mut out.modules),
        "openhud" => module_card(ui, r, "openhud", root, &mut out.modules),
        "oop" => option_presets_card(ui, r, st, root),
        "preview" => preview_card(ui, r),
        "profiles" => profiles_card(ui, r, st),
        _ => files_card(ui, r, out, host),
    }
}

thread_local! {
    /// The radio stations as the page edits them (`radio.cfg`, read the first time).
    pub static RADIO: RefCell<Option<Vec<(String, String)>>> = const { RefCell::new(None) };
}

/// The bus radios' internet stations (`radio.cfg`, see `radio`): each with its name and
/// address, removed or added here, saved at once (#857). Returns the card's height.
fn radio_stations(ui: &mut Ui, r: Rect, host: &dyn Host) -> f32 {
    let mut y = card_frame(ui, r, "Radio stations", None);
    let x = r.x + 16.0;
    let w = r.w - 32.0;
    y += ui.paragraph("A radio's station button n plays the n-th station, a cassette player the first; Shift+R steps through them. An address is an MP3, AAC or Ogg stream or an .m3u/.pls playlist.", Vec2::new(x, y), w, 12.5, Weight::Regular, TEXT_DIM) + 8.0;
    RADIO.with(|cell| {
        let mut cell = cell.borrow_mut();
        let list = cell.get_or_insert_with(|| host.own_stations());
        let mut changed = false;
        let mut remove = None;
        for (k, (name, address)) in list.iter_mut().enumerate() {
            let row = Rect::new(x, y, w, ROW - 2.0);
            let nw = (row.w * 0.3).round();
            changed |= ui.text_input(
                &format!("radio-name-{k}"),
                Rect::new(row.x, row.y, nw, row.h),
                name,
                "Name",
                None,
            );
            changed |= ui.text_input(
                &format!("radio-url-{k}"),
                Rect::new(row.x + nw + 8.0, row.y, row.w - nw - 8.0 - 36.0, row.h),
                address,
                "https://…",
                None,
            );
            if ui.icon_button(
                &format!("radio-del-{k}"),
                Vec2::new(row.right() - 16.0, row.center().y),
                14.0,
                "delete",
                "Remove this station",
            ) {
                remove = Some(k);
            }
            y += ROW + 4.0;
        }
        if let Some(k) = remove {
            list.remove(k);
            changed = true;
        }
        if ui.button(
            "radio-add",
            Rect::new(x, y, w, ROW - 2.0),
            "Add a station",
            Some("add"),
            ButtonKind::Normal,
        ) {
            list.push((String::new(), String::new()));
        }
        y += ROW + 4.0;
        if changed {
            if let Err(e) = host.save_stations(list) {
                log::warn!("radio.cfg: {e}");
            }
        }
    });
    y + 12.0 - r.y
}

thread_local! {
    /// The plug-ins of the OMSI 2 folder (`plugins/*.opl`), read once per folder.
    static PLUGINS_FOUND: RefCell<Option<(std::path::PathBuf, Vec<String>)>> = const { RefCell::new(None) };
}

/// The OMSI 2 folder's plug-ins: listed, read only (they are not loaded in this build).
fn plugins_card(ui: &mut Ui, r: Rect, root: &std::path::Path) -> f32 {
    let names = PLUGINS_FOUND.with(|c| {
        let mut c = c.borrow_mut();
        if c.as_ref().is_none_or(|x| x.0 != root) {
            let mut list: Vec<String> = std::fs::read_dir(root.join("plugins"))
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("opl")))
                .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
                .collect();
            list.sort_by_key(|n| n.to_lowercase());
            *c = Some((root.to_path_buf(), list));
        }
        c.as_ref().map(|x| x.1.clone()).unwrap_or_default()
    });
    let mut y = card_frame(ui, r, "Plug-ins found", Some(names.len()));
    let x = r.x + 16.0;
    let w = r.w - 32.0;
    if names.is_empty() {
        ui.text_in(
            "The OMSI 2 folder has no plug-ins.",
            Rect::new(x, y, w, 24.0),
            13.0,
            Weight::Regular,
            TEXT_DIM,
            Align::Left,
        );
        y += 30.0;
    }
    for n in &names {
        ui.p().rect(Rect::new(r.x, y, r.w, 1.0), EDGE);
        ui.text_in(
            n,
            Rect::new(x, y, w * 0.6, 34.0),
            13.0,
            Weight::Regular,
            TEXT,
            Align::Left,
        );
        ui.text_in(
            "Not loaded in this build",
            Rect::new(x + w * 0.6, y, w * 0.4, 34.0),
            12.5,
            Weight::Regular,
            TEXT_DIM,
            Align::Right,
        );
        y += 34.0;
    }
    y += 6.0;
    y += ui.paragraph("Plug-ins are listed so you can see what a bus or a map expects. They are not loaded in this build.", Vec2::new(x, y), w, 12.0, Weight::Regular, TEXT_DIM) + 14.0;
    y - r.y
}

thread_local! {
    /// OMSI 2's own option presets (`option_presets/*.oop`), read once for each OMSI 2 folder.
    static OOP: RefCell<Option<(std::path::PathBuf, Vec<(String, Value)>)>> = const { RefCell::new(None) };
}

/// OMSI 2's option presets: each applied with a click (only the options this game reads from them).
fn option_presets_card(ui: &mut Ui, r: Rect, st: &mut Store, root: &std::path::Path) -> f32 {
    // (read again for another OMSI 2 folder)
    let list = OOP.with(|c| {
        let mut c = c.borrow_mut();
        if c.as_ref().is_none_or(|(at, _)| at != root) {
            *c = Some((root.to_path_buf(), core::option_presets()));
        }
        c.as_ref().map(|x| x.1.clone()).unwrap_or_default()
    });
    let mut y = card_frame(ui, r, "OMSI 2's option presets", Some(list.len()));
    let x = r.x + 16.0;
    let w = r.w - 32.0;
    y += ui.paragraph("From the option_presets folder of the OMSI 2 folder. One changes only what this game reads from it: the frame limit, the object detail and distance, the anisotropic filtering, the texture memory and the mirrors.", Vec2::new(x, y), w, 12.0, Weight::Regular, TEXT_DIM) + 8.0;
    if list.is_empty() {
        ui.text_in(
            "No presets found.",
            Rect::new(x, y, w, 24.0),
            13.0,
            Weight::Regular,
            TEXT_DIM,
            Align::Left,
        );
        y += 30.0;
    }
    for (k, (name, values)) in list.iter().enumerate() {
        ui.text_in(
            name,
            Rect::new(x, y, w - 100.0, 34.0),
            13.0,
            Weight::Regular,
            TEXT,
            Align::Left,
        );
        if ui.button(
            &format!("oop-{k}"),
            Rect::new(r.right() - 16.0 - 90.0, y + 2.0, 90.0, 30.0),
            "Apply",
            None,
            ButtonKind::Normal,
        ) {
            for (key, v) in values.as_object().into_iter().flatten() {
                st.set(key, v.clone());
            }
        }
        y += 36.0;
    }
    y + 10.0 - r.y
}

/// The saved graphics profiles' card: the list, the name being typed.
#[derive(Default)]
struct GfxProfileUi {
    name: String,
    sel: usize,
    list: Option<Vec<String>>,
    msg: String,
}

thread_local! {
    static GFX_PROFILES: RefCell<GfxProfileUi> = RefCell::new(GfxProfileUi::default());
}

/// Save, load and delete the graphics settings as named profiles.
fn profiles_card(ui: &mut Ui, r: Rect, st: &mut Store) -> f32 {
    let mut y = card_frame(ui, r, "Graphics profiles", None);
    let x = r.x + 16.0;
    let w = r.w - 32.0;
    let row = |y: &mut f32| {
        let rr = Rect::new(x, *y, w, ROW - 2.0);
        *y += ROW + 4.0;
        rr
    };
    GFX_PROFILES.with(|g| {
        let mut g = g.borrow_mut();
        let g = &mut *g;
        let names: Vec<String> = g
            .list
            .get_or_insert_with(|| core::graphics_profiles().into_keys().collect())
            .clone();
        g.sel = g.sel.min(names.len().saturating_sub(1));
        let labels: Vec<String> = if names.is_empty() {
            vec!["No saved profiles".to_string()]
        } else {
            names.clone()
        };
        let r = row(&mut y);
        ui.label(Rect::new(r.x, r.y, r.w * 0.45, r.h), "Saved profile");
        if ui.select(
            "s-gp-sel",
            Rect::new(r.x + r.w * 0.45, r.y, r.w * 0.55, r.h),
            &mut g.sel,
            &labels,
        ) && !names.is_empty()
        {
            g.name = names[g.sel].clone();
        }
        let r = row(&mut y);
        let half = (r.w - GAP) * 0.5;
        if ui.button(
            "s-gp-load",
            Rect::new(r.x, r.y, half, r.h),
            "Load",
            Some("download"),
            ButtonKind::Normal,
        ) && !names.is_empty()
        {
            let name = names[g.sel].clone();
            match core::graphics_profiles().get(&name) {
                Some(p) => {
                    core::apply_graphics_profile(p, st.s);
                    st.changed = true;
                    g.msg = omsi_ui::tr("Loaded \"%{name}\".").replace("%{name}", &name);
                }
                None => g.msg = omsi_ui::tr("\"%{name}\" is gone.").replace("%{name}", &name),
            }
        }
        if ui.button(
            "s-gp-del",
            Rect::new(r.x + half + GAP, r.y, half, r.h),
            "Delete",
            Some("delete"),
            ButtonKind::Danger,
        ) && !names.is_empty()
        {
            let name = names[g.sel].clone();
            g.msg = match core::delete_graphics_profile(&name) {
                Ok(()) => omsi_ui::tr("Deleted \"%{name}\".").replace("%{name}", &name),
                Err(e) => format!("{e:#}"),
            };
            g.list = None;
        }
        let r = row(&mut y);
        ui.text_input("s-gp-name", r, &mut g.name, "Profile name", None);
        let r = row(&mut y);
        if ui.button(
            "s-gp-save",
            r,
            "Save current graphics as profile",
            Some("save"),
            ButtonKind::Primary,
        ) {
            g.msg = match core::save_graphics_profile(&g.name, st.s) {
                Ok(name) => {
                    g.name = name.clone();
                    g.list = None;
                    if let Some(i) = core::graphics_profiles().keys().position(|k| *k == name) {
                        g.sel = i;
                    }
                    omsi_ui::tr("Saved \"%{name}\".").replace("%{name}", &name)
                }
                Err(e) => format!("{e:#}"),
            };
        }
        if !g.msg.is_empty() {
            y += ui.paragraph(&g.msg, Vec2::new(x, y), w, 12.5, Weight::Regular, TEXT_DIM) + 8.0;
        }
    });
    y + 10.0 - r.y
}

/// Every setting back to how it came, the settings' folder, this build.
fn files_card(ui: &mut Ui, r: Rect, out: &mut Outside, host: &dyn Host) -> f32 {
    let mut y = card_frame(ui, r, "Reset and files", None);
    let x = r.x + 16.0;
    let w = r.w - 32.0;
    if ui.button(
        "s-reset",
        Rect::new(x, y, w, 34.0),
        "Reset every setting...",
        Some("restart_alt"),
        ButtonKind::Danger,
    ) {
        out.reset = true;
    }
    y += 42.0;
    if ui.button(
        "s-folder",
        Rect::new(x, y, w, 34.0),
        "Open the settings folder",
        Some("folder_open"),
        ButtonKind::Normal,
    ) {
        core::open_folder(&core::data_dir());
    }
    y += 46.0;
    // this build's version and the project's release page (in-app updating is disabled, see
    // docs/removed/UPDATER_DISABLED.md; the link opens only on a click)
    ui.text_in(
        &omsi_ui::tr("This is Project Depot %{version}").replace("%{version}", host.version().0),
        Rect::new(x, y, w, 22.0),
        12.5,
        Weight::Regular,
        TEXT_DIM,
        Align::Left,
    );
    y += 26.0;
    if ui.button(
        "s-about-releases",
        Rect::new(x, y, w, 34.0),
        "github.com/OmsiModsBR/depot/releases",
        Some("open_in_new"),
        ButtonKind::Ghost,
    ) {
        host.open_url(host.releases_url());
    }
    y + 34.0 + 14.0 - r.y
}

/// The dialog that asks before every setting goes back to how it came.
pub fn reset_dialog(f: &mut Frame, view: &mut SettingsView, open: &mut bool) {
    let size = f.ui.size;
    let full = Rect::new(0.0, 0.0, size.x, size.y);
    f.ui.solid(full);
    f.ui.p().rect(full, SCRIM);
    let w = (size.x - 48.0).min(520.0);
    let h = 190.0;
    let r = Rect::new((size.x - w) * 0.5, (size.y - h) * 0.5, w, h);
    f.ui.panel(r);
    let inner = Rect::new(r.x + 24.0, r.y + 20.0, r.w - 48.0, r.h - 40.0);
    f.ui.icon(
        "restart_alt",
        Vec2::new(inner.x + 14.0, inner.y + 14.0),
        26.0,
        DANGER,
    );
    f.ui.text_in(
        "Reset every setting?",
        Rect::new(inner.x + 38.0, inner.y, inner.w - 38.0, 28.0),
        18.0,
        Weight::Bold,
        TEXT,
        Align::Left,
    );
    f.ui.paragraph("Every option on this page goes back to how it came, the start's choices (season, traffic, passengers) included. The language, the drivers, the key bindings and the game folder stay.", Vec2::new(inner.x, inner.y + 40.0), inner.w, 13.0, Weight::Regular, TEXT_DIM);
    let by = inner.bottom() - 38.0;
    if f.ui.button(
        "reset-no",
        Rect::new(inner.right() - 250.0, by, 110.0, 38.0),
        "Cancel",
        None,
        ButtonKind::Normal,
    ) {
        *open = false;
    }
    if f.ui.button(
        "reset-yes",
        Rect::new(inner.right() - 130.0, by, 130.0, 38.0),
        "Reset",
        Some("restart_alt"),
        ButtonKind::Danger,
    ) {
        // (with OMSI 2 chosen the options on the page are its own: the Depot's stay as they are)
        if f.state.omsi2_engine() {
            let none = serde_json::Map::new();
            match omsi_launcher_lib::original_engine::config::save_settings_in(
                &omsi_launcher_lib::data_dir(),
                &none,
            ) {
                Ok(()) => {
                    view.omsi2_saved = Some(none);
                    f.state
                        .set_status("Every setting is back to how it came.", false);
                }
                Err(e) => f.state.set_status(
                    omsi_ui::tr("OMSI 2 mode settings were not reset: %{error}")
                        .replace("%{error}", &format!("{e:#}")),
                    true,
                ),
            }
            *open = false;
            return;
        }
        let language = f.state.settings.get("language").cloned();
        f.state.settings = core::settings_from_text(None);
        if let Some(lang) = language {
            f.state.settings["language"] = lang;
        }
        f.state.settings_dirty = 0.3;
        omsi_ui::theme::set_accent(omsi_ui::theme::Accent::default());
        // (the start's choices this page shows go back too, the season with what it brings)
        let season_before = f.state.choice.season.clone();
        apply_start(&mut f.state.choice, &start_values(&Choice::default()));
        let season = f.state.choice.season.clone();
        if season != season_before {
            f.state.set_season(&season);
        }
        f.state.touched();
        *open = false;
        f.state
            .set_status("Every setting is back to how it came.", false);
    }
}

// --- interface ----------------------------------------------------------------------------------

/// The accent colour as swatches: the launcher and every menu of the game wear it. Its title
/// marks a change, as a row's name does, and the button under it takes it back.
fn accent_card(ui: &mut Ui, r: Rect, o: &Opt, st: &mut Store) {
    use omsi_ui::theme::Accent;
    let top = card_frame(ui, r, o.label, None);
    let x = r.x + 16.0;
    let iw = r.w - 32.0;
    if changed(o, st) {
        let tw = ui.width(&omsi_ui::tr(o.label).to_uppercase(), 12.0, Weight::Bold) * 1.12;
        ui.p()
            .circle(Vec2::new(x + tw + 10.0, r.y + 22.0), 3.0, accent());
    }
    ui.text_in(
        &omsi_ui::tr(When::Now.text()),
        Rect::new(r.right() - 116.0, r.y + 8.0, 100.0, 24.0),
        11.5,
        Weight::Regular,
        OK,
        Align::Right,
    );
    let chosen = Accent::named(st.get(o.key).as_str().unwrap_or("")).unwrap_or_default();
    let gap = 12.0;
    let w = (iw - gap * 3.0) / 4.0;
    for (i, a) in Accent::ALL.into_iter().enumerate() {
        let sx = x + (i % 4) as f32 * (w + gap);
        let sy = top + (i / 4) as f32 * 84.0;
        let sw = Rect::new(sx, sy, w, 52.0);
        let id = id_of(&format!("accent-{}", a.name().to_ascii_lowercase()));
        let (hover, _, clicked) = ui.interact_key(id, Rect::new(sx, sy, w, 76.0));
        let on = a == chosen;
        ui.p().rounded(
            sw,
            RADIUS,
            if hover && !on {
                a.color().lighten(0.08)
            } else {
                a.color()
            },
        );
        if on {
            ui.p()
                .rounded_border(sw.inset(-3.0), RADIUS + 3.0, 2.0, TEXT);
            ui.icon(
                "check_circle",
                sw.center(),
                22.0,
                omsi_ui::theme::on(a.color()),
            );
        }
        let name = if a == Accent::default() {
            omsi_ui::tr("%{name} (default)").replace("%{name}", &omsi_ui::tr(a.name()))
        } else {
            a.name().to_string()
        };
        ui.text_in(
            &name,
            Rect::new(sx, sy + 56.0, w, 20.0),
            12.0,
            Weight::Regular,
            if on { TEXT } else { TEXT_DIM },
            Align::Center,
        );
        if clicked && !on {
            st.set(o.key, json!(a.name()));
            after(o.key, st);
        }
    }
    let mut y = top + 84.0 * 2.0;
    y += ui.paragraph("A short fixed palette, chosen for contrast on the dark background. It applies to the launcher and to every menu of the game, the Alt bar included.", Vec2::new(x, y), iw, 12.0, Weight::Regular, TEXT_DIM) + 10.0;
    if ui.button(
        "accent-default",
        Rect::new(x, y, 160.0, 34.0),
        "Back to default",
        Some("restart_alt"),
        ButtonKind::Normal,
    ) && chosen != Accent::default()
    {
        reset(o, st);
    }
}

/// What wears the accent, drawn and not clickable: the page's own widgets are the real ones.
fn preview_card(ui: &mut Ui, r: Rect) -> f32 {
    let mut y = card_frame(ui, r, "Preview", None);
    let x = r.x + 16.0;
    let iw = r.w - 32.0;
    let nav = Rect::new(x, y, 200.0, 40.0);
    ui.p().rounded(nav, 4.0, HOVER);
    ui.p()
        .rounded(Rect::new(nav.x, nav.y + 11.0, 3.0, 18.0), 1.5, accent());
    ui.icon(
        "map",
        Vec2::new(nav.x + 24.0, nav.center().y),
        20.0,
        accent(),
    );
    ui.text_in(
        "Maps",
        Rect::new(nav.x + 44.0, nav.y, 140.0, nav.h),
        14.0,
        Weight::Medium,
        TEXT,
        Align::Left,
    );
    y += 52.0;
    let go = Rect::new(x, y, 130.0, 34.0);
    ui.p().rounded(go, 6.0, accent());
    ui.text_in(
        "Start driving",
        go,
        13.0,
        Weight::Bold,
        on_accent(),
        Align::Center,
    );
    let cancel = Rect::new(go.right() + 10.0, y, 90.0, 34.0);
    ui.p().rounded(cancel, 6.0, FIELD);
    ui.p().rounded_border(cancel, 6.0, 1.0, EDGE);
    ui.text_in("Cancel", cancel, 13.0, Weight::Medium, TEXT, Align::Center);
    for (k, on) in [(0.0, true), (1.0, false)] {
        let t = Rect::new(cancel.right() + 16.0 + k * 50.0, y + 8.0, 34.0, 18.0);
        ui.p().rounded(
            t,
            9.0,
            if on {
                accent()
            } else {
                Color::rgba(62, 62, 62, 1.0)
            },
        );
        ui.p().circle(
            Vec2::new(if on { t.right() - 9.0 } else { t.x + 9.0 }, t.center().y),
            6.0,
            Color::rgba(240, 240, 240, 1.0),
        );
    }
    y += 50.0;
    let row = Rect::new(x, y, (iw * 0.55).max(200.0), 48.0);
    ui.p().rounded(row, 6.0, omsi_ui::theme::accent_soft());
    ui.p().rounded_border(row, 6.0, 1.0, accent().alpha(0.55));
    ui.text_in(
        "Example bus A",
        Rect::new(row.x + 12.0, row.y + 5.0, row.w - 24.0, 20.0),
        13.5,
        Weight::Bold,
        TEXT,
        Align::Left,
    );
    ui.text_in(
        "Selected item",
        Rect::new(row.x + 12.0, row.y + 24.0, row.w - 24.0, 18.0),
        12.0,
        Weight::Regular,
        TEXT_DIM,
        Align::Left,
    );
    ui.text_in(
        "Link and highlight text",
        Rect::new(
            row.right() + 14.0,
            row.y,
            (x + iw - row.right() - 14.0).max(0.0),
            row.h,
        ),
        13.5,
        Weight::Regular,
        accent(),
        Align::Left,
    );
    y + 60.0 - r.y
}
