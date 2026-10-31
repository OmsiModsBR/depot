//! The Controls page's keyboard on a computer, as the Windows 11 concept draws it: the actions
//! in groups on the left, the group's actions with their key and second key in the middle,
//! the chosen action on the right; above them, how the keys compare with the original OMSI 2's
//! and its Reload. A key used twice is flagged, never refused: both actions fire, as in OMSI 2.
//! The keys are named as OMSI 2 names them in the game's language (`Inputs/<lang>.kyb`).

use super::super::domain::frame::Frame;
use super::super::domain::keyboard::*;
use super::super::interface::output::{core, Host, State};
use super::dialog::{dialog, dialog_foot};
use super::heading::card_title;
use glam::Vec2;
use omsi_ui::paint::Align;
use omsi_ui::theme::{
    accent, DANGER, EDGE, FIELD, OK, TEXT, TEXT_DIM, TEXT_FAINT, TEXT_SOFT, WARN,
};
use omsi_ui::widgets::{id_of, matches, ButtonKind, Key, Ui};
use omsi_ui::{Color, Rect, Weight};
use serde_json::{json, Value};

/// The actions' names in the game's language (OMSI 2's own texts, the Depot's for its own).
struct Labels<'a> {
    host: &'a dyn Host,
    root: String,
    lang: String,
}

impl Labels<'_> {
    fn text(&self, action: &str) -> String {
        self.host.action_text(&self.root, &self.lang, action)
    }

    fn events(&self) -> Vec<(String, String)> {
        self.host.actions(&self.root, &self.lang)
    }
}

fn labels<'a>(f: &Frame<'a>) -> Labels<'a> {
    Labels {
        host: f.host,
        root: f.state.config.root.clone(),
        lang: language(f.state),
    }
}

/// The interface language, as the settings keep it (OMSI 2's three letters).
fn language(state: &State) -> String {
    state
        .settings
        .get("language")
        .and_then(|x| x.as_str())
        .unwrap_or("ENG")
        .to_string()
}

/// A key as a small key cap; a dim dash for none. Returns its rectangle.
fn cap(ui: &mut Ui, x: f32, cy: f32, text: &str, edge: Color) -> Rect {
    let w = ui.width(text, 12.0, Weight::Medium) + 16.0;
    let r = Rect::new(x, cy - 11.0, w.max(26.0), 22.0);
    ui.p().rounded(r, 4.0, Color::WHITE.alpha(0.05));
    ui.p().rounded_border(r, 4.0, 1.0, edge);
    ui.text_in(text, r, 12.0, Weight::Medium, TEXT, Align::Center);
    r
}

/// A small label in a pill (as "changed", "shared key").
fn pill(ui: &mut Ui, x: f32, cy: f32, text: &str, c: Color) -> f32 {
    let w = ui.width(text, 11.5, Weight::Medium) + 18.0;
    let r = Rect::new(x, cy - 10.0, w, 20.0);
    ui.p().rounded(r, 10.0, Color::WHITE.alpha(0.04));
    ui.p().rounded_border(r, 10.0, 1.0, EDGE);
    ui.text_in(text, r, 11.5, Weight::Medium, c, Align::Center);
    w
}

/// The keyboard tab on a computer, in `body` (under the page's tabs).
pub fn keyboard_tab(f: &mut Frame, view: &mut KeyboardView, body: Rect) {
    let lang = language(f.state);
    view.load(&lang);
    // (with a window open, the key goes to it: see `dialog`)
    if view.dialog.is_none() {
        capture(f, view);
    }
    let labels = labels(f);
    let label = |a: &str| labels.text(a);
    let original = view.original.clone().flatten();
    let factory = view.factory.clone();
    // (with OMSI 2 chosen the page shows the keys of the OMSI 2 folder's own controls file,
    // as they are, and changes nothing: they are changed inside OMSI 2)
    let read_only = f.state.omsi2_engine();
    let shown_keys = if read_only {
        original
            .clone()
            .unwrap_or_else(|| json!({ "vehicles": [], "game": [] }))
    } else {
        f.state.keybindings.clone()
    };
    let mut all = rows(&shown_keys, original.as_ref(), factory.as_ref(), &label);
    // (the original OMSI 2 has no action of the Depot's own: VR, chat, the any-bus doors)
    if f.state.omsi2_engine() {
        all.retain(|r| crate::omsi2_map::action_in_omsi2(&r.action));
    }
    // (as many as a reload would change: the original's actions the Depot lacks count too)
    let changed = original
        .as_ref()
        .filter(|_| !read_only)
        .map(|o| {
            reload_plan(&f.state.keybindings, o, &|a| a.to_string())
                .changes
                .len()
        })
        .unwrap_or(0);
    // (the chip at the tabs' right: how far the layout is from the original)
    let chip = if original.is_none() {
        "Your layout".to_string()
    } else if changed == 0 {
        "Your layout · same as your OMSI 2".into()
    } else {
        omsi_ui::tr("Your layout · %{n} changed from your OMSI 2")
            .replace("%{n}", &changed.to_string())
    };
    if !read_only {
        let cw = f.ui.width(&chip, 12.0, Weight::Regular) + 24.0;
        let cr = Rect::new(body.right() - cw, body.y - 44.0, cw, 26.0);
        f.ui.p().rounded(cr, 13.0, Color::WHITE.alpha(0.04));
        f.ui.p().rounded_border(cr, 13.0, 1.0, EDGE);
        f.ui.text_in(&chip, cr, 12.0, Weight::Regular, TEXT_DIM, Align::Center);
    }
    let mut y = body.y;
    if read_only {
        y += read_only_bar(f.ui, Rect::new(body.x, y, body.w, 44.0)) + 12.0;
    } else {
        y += preset_bar(f, Rect::new(body.x, y, body.w, 0.0));
        y += import_bar(
            f,
            view,
            Rect::new(body.x, y, body.w, 56.0),
            original.is_some(),
            changed,
        ) + 12.0;
    }
    let cols = Rect::new(body.x, y, body.w, body.bottom() - y);
    let gw = crate::kit::fit(210.0, cols.w);
    let dw = crate::kit::fit(330.0, cols.w);
    let groups = Rect::new(cols.x, cols.y, gw, cols.h);
    let detail = Rect::new(cols.right() - dw, cols.y, dw, cols.h);
    let list = Rect::new(
        groups.right() + 12.0,
        cols.y,
        detail.x - groups.right() - 24.0,
        cols.h,
    );
    groups_card(f, view, groups, &all);
    list_card(f, view, list, &all);
    detail_card(f, view, detail, &all, original.as_ref());
    if !f.state.keybindings_error.is_empty() {
        let e = f.state.keybindings_error.clone();
        f.ui.text_in(
            &e,
            Rect::new(body.x, body.bottom() + 4.0, body.w, 18.0),
            12.0,
            Weight::Medium,
            DANGER,
            Align::Left,
        );
    }
}

/// A key pressed while an entry waits for one (`pages.capturing`), or for the Add window.
fn capture(f: &mut Frame, view: &mut KeyboardView) {
    // (with OMSI 2 chosen the keys are only shown, see `read_only`)
    if f.state.omsi2_engine() {
        view.capturing = None;
        return;
    }
    let Some(code) = f.ui.input.raw_key else {
        return;
    };
    use winit::keyboard::KeyCode as K;
    if matches!(
        code,
        K::ShiftLeft
            | K::ShiftRight
            | K::ControlLeft
            | K::ControlRight
            | K::AltLeft
            | K::AltRight
            | K::SuperLeft
            | K::SuperRight
    ) {
        return;
    }
    let add_waiting = matches!(view.dialog, Some(Dialog::Add { waiting: true, .. }));
    if view.capturing.is_none() && !add_waiting {
        return;
    }
    f.ui.input.raw_key = None;
    if code == K::Escape {
        drop_added(f, view);
        view.capturing = None;
        if let Some(Dialog::Add { waiting, .. }) = &mut view.dialog {
            *waiting = false;
        }
        // (Escape leaves the key as it was; it does not close the window too)
        f.ui.input.keys.retain(|k| *k != Key::Escape);
        return;
    }
    let Some(scan) = f.host.dik_code(code) else {
        f.state.set_status(
            omsi_ui::tr("%{key} has no DirectInput scan code the game understands.")
                .replace("%{key}", &format!("{code:?}")),
            true,
        );
        view.capturing = None;
        return;
    };
    let m = omsi_content::input::chord(f.ui.input.shift, f.ui.input.ctrl, f.ui.input.alt) as i64;
    if let Some(Dialog::Add { key, waiting, .. }) = &mut view.dialog {
        *key = Some((scan as i64, m));
        *waiting = false;
        return;
    }
    let Some((sec, idx)) = view.capturing.take() else {
        return;
    };
    view.added = None;
    let section = SECTIONS[sec];
    let vr = f
        .state
        .keybindings
        .get(section)
        .and_then(|a| a.as_array())
        .and_then(|a| a.get(idx))
        .and_then(|b| b.get("action"))
        .and_then(|a| a.as_str())
        .is_some_and(|a| a.starts_with("vr_"));
    if let Some(b) = f
        .state
        .keybindings
        .get_mut(section)
        .and_then(|a| a.as_array_mut())
        .and_then(|a| a.get_mut(idx))
    {
        // (the entry's "held" bit is the action's, not the key's: it stays)
        let hold = b.get("modifier").and_then(|x| x.as_i64()).unwrap_or(0)
            & omsi_content::input::KEY_HOLD as i64;
        b["scan_code"] = json!(scan);
        b["modifier"] = json!(m | hold);
    }
    save_keys(f.state, vr);
}

/// With OMSI 2 chosen the keys and controllers are only shown: where they are changed.
pub fn read_only_bar(ui: &mut Ui, r: Rect) -> f32 {
    ui.p().rounded(r, 6.0, accent().alpha(0.08));
    ui.p().rounded_border(r, 6.0, 1.0, accent().alpha(0.45));
    ui.icon("info", Vec2::new(r.x + 20.0, r.center().y), 16.0, accent());
    let text = omsi_ui::tr("Keys and controllers are read from your OMSI 2 folder. Change them inside OMSI 2: they are never written from here.");
    let text = crate::kit::fit_text(ui, &text, 12.5, r.w - 56.0);
    ui.text_in(
        &text,
        Rect::new(r.x + 40.0, r.y, r.w - 52.0, r.h),
        12.5,
        Weight::Regular,
        TEXT_SOFT,
        Align::Left,
    );
    r.h
}

/// The ready-made driving keys win over this page's: say so, with the switch (none with
/// Custom controls). Returns the height used.
fn preset_bar(f: &mut Frame, r: Rect) -> f32 {
    let preset = f
        .state
        .settings
        .get("drive_keys")
        .and_then(|v| v.as_str())
        .unwrap_or("simple")
        .to_string();
    if preset == "omsi" {
        return 0.0;
    }
    let name = match preset.as_str() {
        "wasd" => "W A S D only",
        "arrows" => "Arrow keys only",
        _ => "W A S D + arrows",
    };
    let bar = Rect::new(r.x, r.y, r.w, 40.0);
    f.ui.p().rounded(bar, 6.0, Color::WHITE.alpha(0.04));
    f.ui.p().rounded_border(bar, 6.0, 1.0, EDGE);
    f.ui.icon(
        "info",
        Vec2::new(bar.x + 18.0, bar.center().y),
        16.0,
        accent(),
    );
    let bw = 150.0;
    let text = omsi_ui::tr("Driving keys: %{name} (Settings) drive the bus and win over the list below; any key changed here switches to your own layout.").replace("%{name}", &omsi_ui::tr(&name));
    let text = crate::kit::fit_text(f.ui, &text, 12.5, bar.w - bw - 60.0);
    f.ui.text_in(
        &text,
        Rect::new(bar.x + 36.0, bar.y, bar.w - bw - 50.0, bar.h),
        12.5,
        Weight::Regular,
        TEXT_SOFT,
        Align::Left,
    );
    if f.ui.button(
        "kb-use-custom",
        Rect::new(bar.right() - bw - 6.0, bar.y + 5.0, bw, 30.0),
        "Use these keys",
        None,
        ButtonKind::Normal,
    ) {
        use_custom_keys(f.state);
    }
    bar.h + 10.0
}

/// How the keys stand against the original OMSI 2's, and its Reload. Returns its height.
fn import_bar(f: &mut Frame, view: &mut KeyboardView, r: Rect, found: bool, changed: usize) -> f32 {
    f.ui.p().rounded(r, 6.0, accent().alpha(0.08));
    f.ui.p().rounded_border(r, 6.0, 1.0, accent().alpha(0.45));
    f.ui.icon(
        "o-folder",
        Vec2::new(r.x + 20.0, r.center().y),
        16.0,
        TEXT_SOFT,
    );
    let (lead, rest) = if let Some(e) = &view.original_error {
        (
            "Your OMSI 2 controls file could not be read".to_string(),
            format!(" · {e}"),
        )
    } else if !found {
        (
            "No controls file in your OMSI 2 folder".to_string(),
            " · the keys are the Depot's own.".to_string(),
        )
    } else {
        let lead = if changed == 0 {
            "Same keys as your original OMSI 2".to_string()
        } else if changed == 1 {
            "1 key differs from your original OMSI 2".to_string()
        } else {
            omsi_ui::tr("%{n} keys differ from your original OMSI 2")
                .replace("%{n}", &changed.to_string())
        };
        let reloaded = view
            .reloaded
            .map(day)
            .unwrap_or_else(|| omsi_ui::tr("never").into_owned());
        let rest = match view.saved {
            Some(t) => omsi_ui::tr(" · Controls file found in your OMSI 2 folder, saved %{day}. Last reloaded: %{reloaded}.").replace("%{day}", &day(t)),
            None => omsi_ui::tr(" · Controls file found in your OMSI 2 folder. Last reloaded: %{reloaded}.").into_owned(),
        };
        (lead, rest.replace("%{reloaded}", &reloaded))
    };
    let bw = [
        f.ui.width("Preview changes", 13.0, Weight::Medium) + 32.0,
        f.ui.width("Reload from OMSI 2", 13.0, Weight::Bold) + 50.0,
    ];
    let room = r.w - 44.0 - bw[0] - bw[1] - 36.0;
    let lw = f.ui.width(&lead, 12.5, Weight::Bold);
    f.ui.text_in(
        &lead,
        Rect::new(r.x + 40.0, r.y, lw + 2.0, r.h),
        12.5,
        Weight::Bold,
        TEXT,
        Align::Left,
    );
    let rest = crate::kit::fit_text(f.ui, &rest, 12.5, (room - lw).max(40.0));
    f.ui.text_in(
        &rest,
        Rect::new(r.x + 40.0 + lw, r.y, (room - lw).max(40.0), r.h),
        12.5,
        Weight::Regular,
        TEXT_SOFT,
        Align::Left,
    );
    if found {
        let b2 = Rect::new(r.right() - 12.0 - bw[1], r.center().y - 17.0, bw[1], 34.0);
        let b1 = Rect::new(b2.x - 10.0 - bw[0], b2.y, bw[0], 34.0);
        if f.ui.button(
            "kb-preview",
            b1,
            "Preview changes",
            None,
            ButtonKind::Normal,
        ) || f.ui.button(
            "kb-reload",
            b2,
            "Reload from OMSI 2",
            Some("restart_alt"),
            ButtonKind::Primary,
        ) {
            view.dialog = Some(Dialog::Reload {
                backup: true,
                pads: true,
            });
        }
    }
    r.h
}

fn groups_card(f: &mut Frame, view: &mut KeyboardView, r: Rect, all: &[KeyRow]) {
    f.ui.panel(r);
    let inner = card_title(
        f.ui,
        Rect::new(r.x + 16.0, r.y + 14.0, r.w - 32.0, r.h - 28.0),
        "Groups",
    );
    let row = |f: &mut Frame, view: &mut KeyboardView, i: usize, name: &str, y: f32, warn: bool| {
        let n = all.iter().filter(|x| x.in_list(i)).count();
        let rr = Rect::new(inner.x - 6.0, y, inner.w + 12.0, 32.0);
        let on = view.list == i;
        let (hover, _, clicked) = f.ui.interact_key(id_of(&format!("kb-group-{i}")), rr);
        if on {
            f.ui.p().rounded(rr, 5.0, accent().alpha(0.14));
            f.ui.p().rounded_border(rr, 5.0, 1.0, accent().alpha(0.6));
        } else if hover {
            f.ui.p().rounded(rr, 5.0, Color::WHITE.alpha(0.05));
        }
        let c = if warn { WARN } else { TEXT };
        let name = crate::kit::fit_text(f.ui, name, 13.5, rr.w - 60.0);
        f.ui.text_in(
            &name,
            Rect::new(rr.x + 10.0, rr.y, rr.w - 50.0, rr.h),
            13.5,
            Weight::Medium,
            c,
            Align::Left,
        );
        f.ui.text_in(
            &n.to_string(),
            Rect::new(rr.right() - 44.0, rr.y, 34.0, rr.h),
            12.0,
            Weight::Regular,
            if warn { WARN } else { TEXT_DIM },
            Align::Right,
        );
        if clicked {
            view.list = i;
            view.query.clear();
        }
    };
    for (i, name) in GROUPS.iter().enumerate() {
        row(f, view, i, name, inner.y + i as f32 * 35.0, false);
    }
    let foot = r.bottom() - 14.0 - FILTERS.len() as f32 * 35.0;
    let foot = foot.max(inner.y + GROUPS.len() as f32 * 35.0 + 8.0);
    for (k, name) in FILTERS.iter().enumerate() {
        row(
            f,
            view,
            GROUPS.len() + k,
            name,
            foot + k as f32 * 35.0,
            k == 0,
        );
    }
}

/// The list's name, for its card's title.
fn list_name(i: usize) -> &'static str {
    if i < GROUPS.len() {
        GROUPS[i]
    } else {
        FILTERS[(i - GROUPS.len()).min(FILTERS.len() - 1)]
    }
}

fn list_card(f: &mut Frame, view: &mut KeyboardView, r: Rect, all: &[KeyRow]) {
    f.ui.panel(r);
    let q = view.query.trim().to_lowercase();
    // (a search looks through every action, whatever the group)
    let shown: Vec<&KeyRow> = all
        .iter()
        .filter(|x| {
            if q.is_empty() {
                x.in_list(view.list)
            } else {
                matches(&x.label, &q)
                    || x.action.to_lowercase().contains(&q)
                    || x.keys.iter().any(|k| {
                        k.0 != 0 && view.names.name(f.host, *k).to_lowercase().contains(&q)
                    })
            }
        })
        .collect();
    let title = if q.is_empty() {
        list_name(view.list)
    } else {
        "Search"
    };
    let inner = card_title(
        f.ui,
        Rect::new(r.x + 16.0, r.y + 14.0, r.w - 32.0, r.h - 28.0),
        title,
    );
    let note = if shown.len() == 1 {
        "1 action".to_string()
    } else {
        omsi_ui::tr("%{n} actions").replace("%{n}", &shown.len().to_string())
    };
    f.ui.text_in(
        &note,
        Rect::new(inner.x, r.y + 14.0, inner.w, 18.0),
        12.0,
        Weight::Regular,
        TEXT_DIM,
        Align::Right,
    );
    let read_only = f.state.omsi2_engine();
    let aw = if read_only {
        0.0
    } else {
        f.ui.width("Add a control", 13.0, Weight::Bold) + 48.0
    };
    let mut query = std::mem::take(&mut view.query);
    f.ui.text_input(
        "kb-search",
        Rect::new(
            inner.x,
            inner.y,
            inner.w - if read_only { 0.0 } else { aw + 10.0 },
            34.0,
        ),
        &mut query,
        "Search every action...",
        Some("search"),
    );
    view.query = query;
    if !read_only
        && f.ui.button(
            "kb-add",
            Rect::new(inner.right() - aw, inner.y, aw, 34.0),
            "Add a control",
            Some("add"),
            ButtonKind::Primary,
        )
    {
        view.dialog = Some(Dialog::Add {
            query: String::new(),
            chosen: None,
            key: None,
            waiting: false,
        });
    }
    // the columns: the action, its key, its second key, back to the original
    let kx = inner.x + (inner.w * 0.42).max(180.0);
    let k2x = kx + (inner.w - (kx - inner.x)) * 0.42;
    let hy = inner.y + 52.0;
    for (t, x) in [("Action", inner.x + 8.0), ("Key", kx), ("Second key", k2x)] {
        f.ui.text_in(
            t,
            Rect::new(x, hy - 9.0, 120.0, 18.0),
            12.0,
            Weight::Regular,
            TEXT_DIM,
            Align::Left,
        );
    }
    let area = Rect::new(
        inner.x - 6.0,
        hy + 14.0,
        inner.w + 12.0,
        inner.bottom() - hy - 14.0,
    );
    let chosen = view.chosen.clone();
    let capturing = view.capturing;
    let time = f.ui.time;
    let names = view.names.clone();
    // (what was clicked: the row, a key cell (row, which key), back to the original)
    let mut pick: Option<(usize, String)> = None;
    let mut key_click: Option<(usize, usize)> = None;
    let mut reset: Option<usize> = None;
    let rows: Vec<KeyRow> = shown.into_iter().cloned().collect();
    f.ui.scroll_area(
        &format!("kb-list-{}-{}", view.list, q.is_empty()),
        area,
        &mut |ui, v| {
            let rh = 35.0;
            for (n, row) in rows.iter().enumerate() {
                let rr = Rect::new(v.x + 6.0, v.y + n as f32 * rh, v.w - 16.0, rh - 3.0);
                if !near_view(rr, area) {
                    continue;
                }
                let on = chosen
                    .as_ref()
                    .is_some_and(|c| c.0 == row.sec && c.1 == row.action);
                let (hover, _, clicked) =
                    ui.interact_key(id_of(&format!("kb-row-{}-{}", row.sec, row.action)), rr);
                if on {
                    ui.p().rounded(rr, 5.0, accent().alpha(0.14));
                    ui.p().rounded_border(rr, 5.0, 1.0, accent().alpha(0.6));
                } else if hover {
                    ui.p().rounded(rr, 5.0, Color::WHITE.alpha(0.05));
                }
                if clicked {
                    pick = Some((row.sec, row.action.clone()));
                }
                let cy = rr.center().y;
                let lw = ui.width(&row.label, 13.5, Weight::Regular);
                let room = kx - inner.x - 20.0;
                ui.text_in_tip(
                    &row.label,
                    Rect::new(rr.x + 8.0, rr.y, room, rr.h),
                    13.5,
                    Weight::Regular,
                    TEXT,
                    Align::Left,
                );
                let mut px = rr.x + 8.0 + lw.min(room) + 8.0;
                for (show, text, c) in [
                    (row.changed, "changed", accent()),
                    (row.shared, "shared key", WARN),
                ] {
                    if show && px + 70.0 < kx {
                        let w = ui.width(text, 11.5, Weight::Medium) + 18.0;
                        let pr = Rect::new(px, cy - 10.0, w, 20.0);
                        ui.p().rounded(pr, 10.0, Color::WHITE.alpha(0.04));
                        ui.p().rounded_border(pr, 10.0, 1.0, EDGE);
                        ui.text_in(text, pr, 11.5, Weight::Medium, c, Align::Center);
                        px += w + 6.0;
                    }
                }
                for (k, x) in [(0usize, kx), (1, k2x)] {
                    let entry = row.entries.get(k).copied();
                    let waiting = entry.is_some_and(|e| capturing == Some((row.sec, e)));
                    let key = row.keys.get(k).filter(|c| c.0 != 0);
                    let text = if waiting {
                        "press a key…".to_string()
                    } else {
                        key.map(|c| names.name(f.host, *c))
                            .unwrap_or_else(|| "–".into())
                    };
                    let w = if key.is_some() || waiting {
                        ui.width(&text, 12.0, Weight::Medium) + 16.0
                    } else {
                        26.0
                    };
                    let cr = Rect::new(x, cy - 11.0, w.max(26.0), 22.0);
                    let (h, _, c) =
                        ui.interact(id_of(&format!("kb-cap-{}-{}-{k}", row.sec, row.action)), cr);
                    if key.is_some() || waiting {
                        let edge = if waiting {
                            accent().alpha(0.4 + 0.4 * (time * 6.0).sin().abs())
                        } else if row.shared {
                            WARN
                        } else if h {
                            TEXT_DIM
                        } else {
                            EDGE
                        };
                        ui.p().rounded(cr, 4.0, Color::WHITE.alpha(0.05));
                        ui.p().rounded_border(cr, 4.0, 1.0, edge);
                        ui.text_in(&text, cr, 12.0, Weight::Medium, TEXT, Align::Center);
                    } else {
                        ui.text_in(
                            &text,
                            cr,
                            12.0,
                            Weight::Regular,
                            if h { TEXT_SOFT } else { TEXT_FAINT },
                            Align::Center,
                        );
                    }
                    if c {
                        key_click = Some((n, k));
                    }
                }
                if row.off_factory && !read_only {
                    let xr = Rect::new(rr.right() - 30.0, cy - 12.0, 24.0, 24.0);
                    let (hx, _, cx) =
                        ui.interact(id_of(&format!("kb-reset-{}-{}", row.sec, row.action)), xr);
                    ui.icon(
                        "restart_alt",
                        xr.center(),
                        15.0,
                        if hx { accent() } else { TEXT_DIM },
                    );
                    ui.tooltip(xr, "Back to the default");
                    if cx {
                        reset = Some(n);
                    }
                }
            }
            rows.len() as f32 * rh
        },
    );
    if let Some(p) = pick {
        view.chosen = Some(p);
    }
    if let Some((n, k)) = key_click {
        let row = rows[n].clone();
        view.chosen = Some((row.sec, row.action.clone()));
        wait_for_key(f, view, &row, k);
    }
    if let Some(n) = reset {
        back_to_original(f, view, &rows[n].clone());
    }
}

/// The `k`-th key of `row` waits to be pressed (a second key's entry is added for it when the
/// action has one key only).
fn wait_for_key(f: &mut Frame, view: &mut KeyboardView, row: &KeyRow, k: usize) {
    if f.state.omsi2_engine() {
        return;
    }
    drop_added(f, view);
    if let Some(&e) = row.entries.get(k) {
        view.capturing = Some((row.sec, e));
        return;
    }
    let Some(&last) = row.entries.last() else {
        return;
    };
    if let Some(a) = f
        .state
        .keybindings
        .get_mut(SECTIONS[row.sec])
        .and_then(|a| a.as_array_mut())
    {
        if let Some(b) = a.get(last).cloned() {
            // (the held bit is the action's: it goes with it)
            let hold = b.get("modifier").and_then(|x| x.as_i64()).unwrap_or(0)
                & omsi_content::input::KEY_HOLD as i64;
            a.insert(
                last + 1,
                json!({ "action": row.action, "scan_code": 0, "modifier": hold }),
            );
            view.capturing = Some((row.sec, last + 1));
            view.added = Some((row.sec, last + 1));
        }
    }
}

/// The entry added for a second key goes again when it got no key (Esc, or another key clicked).
fn drop_added(f: &mut Frame, view: &mut KeyboardView) {
    let Some((sec, idx)) = view.added.take() else {
        return;
    };
    if let Some(a) = f
        .state
        .keybindings
        .get_mut(SECTIONS[sec])
        .and_then(|a| a.as_array_mut())
    {
        if a.get(idx)
            .is_some_and(|b| b.get("scan_code").and_then(|x| x.as_i64()).unwrap_or(0) == 0)
        {
            a.remove(idx);
        }
    }
    if view.capturing == Some((sec, idx)) {
        view.capturing = None;
    }
}

/// `row`'s keys as OMSI 2 has them as it comes (its `keyboard_reset.cfg`).
fn back_to_original(f: &mut Frame, view: &mut KeyboardView, row: &KeyRow) {
    if f.state.omsi2_engine() {
        return;
    }
    let key = SECTIONS[row.sec];
    let from = factory_entries(view.factory.as_ref(), key, &row.action);
    if from.is_empty() {
        return;
    }
    let mut only = json!({ "vehicles": [], "game": [] });
    only[key] = json!(from);
    // (the reload of one action: every other one as it is)
    let plan = reload_plan(&f.state.keybindings, &only, &|a| a.to_string());
    f.state.keybindings = plan.merged;
    save_keys(f.state, row.action.starts_with("vr_"));
    if let Some(e) = view.factory_error.clone() {
        f.state.set_status(omsi_ui::tr("OMSI 2's keyboard_reset.cfg could not be read (%{error}): the keys came from the Depot's built-in table").replace("%{error}", &e), true);
    }
}

fn detail_card(
    f: &mut Frame,
    view: &mut KeyboardView,
    r: Rect,
    all: &[KeyRow],
    original: Option<&Value>,
) {
    f.ui.panel(r);
    let row = view
        .chosen
        .as_ref()
        .and_then(|(sec, a)| all.iter().find(|x| x.sec == *sec && x.action == *a))
        .cloned();
    let Some(row) = row else {
        let inner = card_title(
            f.ui,
            Rect::new(r.x + 16.0, r.y + 14.0, r.w - 32.0, r.h - 28.0),
            "Action",
        );
        f.ui.paragraph(
            if f.state.omsi2_engine() {
                "Click an action to see its keys."
            } else {
                "Click an action to see its keys, change them, or take the original OMSI 2's back."
            },
            Vec2::new(inner.x, inner.y + 4.0),
            inner.w,
            13.0,
            Weight::Regular,
            TEXT_DIM,
        );
        return;
    };
    let inner = card_title(
        f.ui,
        Rect::new(r.x + 16.0, r.y + 14.0, r.w - 32.0, r.h - 28.0),
        &row.label,
    );
    let group = GROUPS[row.group];
    f.ui.text_in(
        group,
        Rect::new(inner.x, r.y + 14.0, inner.w, 18.0),
        12.0,
        Weight::Regular,
        TEXT_DIM,
        Align::Right,
    );
    let mut y = inner.y;
    f.ui.text_in(
        &omsi_ui::tr("In the controls file: %{action}").replace("%{action}", &row.action),
        Rect::new(inner.x, y, inner.w, 18.0),
        12.0,
        Weight::Regular,
        TEXT_DIM,
        Align::Left,
    );
    y += 30.0;
    for (k, what) in [(0usize, "Key"), (1, "Second key")] {
        f.ui.text_in(
            what,
            Rect::new(inner.x, y, inner.w, 16.0),
            12.0,
            Weight::Regular,
            TEXT_DIM,
            Align::Left,
        );
        y += 20.0;
        let field = Rect::new(inner.x, y, inner.w, 34.0);
        let waiting = row
            .entries
            .get(k)
            .is_some_and(|e| view.capturing == Some((row.sec, *e)));
        let key = row
            .keys
            .get(k)
            .filter(|c| c.0 != 0)
            .map(|c| view.names.name(f.host, *c));
        let (h, _, c) = f.ui.interact_key(id_of(&format!("kb-field-{k}")), field);
        f.ui.p().rounded(field, 4.0, FIELD);
        f.ui.p().rounded_border(
            field,
            4.0,
            1.0,
            if waiting {
                accent()
            } else if h {
                TEXT_DIM
            } else {
                EDGE
            },
        );
        if waiting {
            f.ui.p()
                .rounded_border(field.inset(-1.0), 5.0, 1.0, accent());
            f.ui.text_in(
                "Press a key now...",
                Rect::new(field.x + 12.0, field.y, field.w - 24.0, field.h),
                13.0,
                Weight::Regular,
                TEXT,
                Align::Left,
            );
            f.ui.text_in(
                "Esc cancels",
                Rect::new(field.x + 12.0, field.y, field.w - 24.0, field.h),
                12.5,
                Weight::Regular,
                TEXT_DIM,
                Align::Right,
            );
        } else {
            f.ui.text_in(
                key.as_deref().unwrap_or("None"),
                Rect::new(field.x + 12.0, field.y, field.w - 40.0, field.h),
                13.0,
                Weight::Regular,
                if key.is_some() { TEXT } else { TEXT_FAINT },
                Align::Left,
            );
            if !f.state.omsi2_engine() {
                f.ui.icon(
                    if key.is_some() { "edit" } else { "add" },
                    Vec2::new(field.right() - 18.0, field.center().y),
                    15.0,
                    TEXT_DIM,
                );
            }
        }
        if c {
            wait_for_key(f, view, &row, k);
        }
        y += 46.0;
    }
    let mine: Vec<Chord> = row.keys.iter().copied().filter(|k| k.0 != 0).collect();
    let line = |f: &mut Frame,
                view: &mut KeyboardView,
                y: &mut f32,
                what: &str,
                keys: &[Chord],
                differs: bool| {
        f.ui.text_in(
            what,
            Rect::new(inner.x, *y, inner.w, 16.0),
            12.0,
            Weight::Regular,
            TEXT_DIM,
            Align::Left,
        );
        *y += 20.0;
        let mut x = inner.x;
        if keys.is_empty() {
            f.ui.text_in(
                "none",
                Rect::new(x, *y, 80.0, 22.0),
                12.5,
                Weight::Regular,
                TEXT_FAINT,
                Align::Left,
            );
            x += 44.0;
        }
        for k in keys {
            let n = view.names.name(f.host, *k);
            x = cap(f.ui, x, *y + 11.0, &n, EDGE).right() + 6.0;
        }
        if differs {
            let now = view.names.list(f.host, &mine);
            let t = crate::kit::fit_text(
                f.ui,
                &omsi_ui::tr("you changed it to %{key}").replace("%{key}", &now),
                12.0,
                inner.right() - x - 6.0,
            );
            f.ui.text_in(
                &t,
                Rect::new(x + 4.0, *y, inner.right() - x, 22.0),
                12.0,
                Weight::Regular,
                TEXT_DIM,
                Align::Left,
            );
        }
        *y += 34.0;
    };
    let in_omsi = original
        .filter(|o| has_action(o, SECTIONS[row.sec], &row.action))
        .map(|o| keys_of(o, SECTIONS[row.sec], &row.action));
    match &in_omsi {
        Some(k) => line(
            f,
            view,
            &mut y,
            "In your OMSI 2 controls file",
            k,
            row.changed,
        ),
        None => {
            f.ui.text_in(
                "In your OMSI 2 controls file",
                Rect::new(inner.x, y, inner.w, 16.0),
                12.0,
                Weight::Regular,
                TEXT_DIM,
                Align::Left,
            );
            f.ui.text_in(
                "Not there: a Depot key, kept as it is.",
                Rect::new(inner.x, y + 20.0, inner.w, 22.0),
                12.5,
                Weight::Regular,
                TEXT_FAINT,
                Align::Left,
            );
            y += 54.0;
        }
    }
    let factory = factory_keys(view.factory.as_ref(), &row.action);
    if !factory.is_empty() {
        line(
            f,
            view,
            &mut y,
            "Original (OMSI 2 as it comes)",
            &factory,
            false,
        );
    }
    let _ = y;
    // the foot: back to the original, clear; and why a shared key is fine
    let read_only = f.state.omsi2_engine();
    let note = if read_only {
        "A key used twice is flagged: both actions fire together."
    } else {
        "A key used twice is flagged, never blocked: both actions fire together, and you can save it that way."
    };
    let nh = f.ui.paragraph_height(note, inner.w, 12.0, Weight::Regular);
    let by = inner.bottom() - nh - 46.0;
    let cw = 80.0;
    let back = Rect::new(inner.x, by, inner.w - cw - 10.0, 34.0);
    if read_only {
        // (nothing to change here)
    } else if row.off_factory {
        if f.ui.button(
            "kb-back",
            back,
            "Back to original",
            Some("restart_alt"),
            ButtonKind::Normal,
        ) {
            back_to_original(f, view, &row);
        }
    } else {
        f.ui.p().rounded(back, 4.0, Color::WHITE.alpha(0.03));
        f.ui.text_in(
            "Back to original",
            back,
            13.0,
            Weight::Medium,
            TEXT_FAINT,
            Align::Center,
        );
    }
    if !read_only
        && f.ui.button(
            "kb-clear",
            Rect::new(inner.right() - cw, by, cw, 34.0),
            "Clear",
            None,
            ButtonKind::Normal,
        )
        && !row.unassigned()
    {
        let key = SECTIONS[row.sec];
        if let Some(a) = f
            .state
            .keybindings
            .get_mut(key)
            .and_then(|a| a.as_array_mut())
        {
            for &e in &row.entries {
                if let Some(b) = a.get_mut(e) {
                    let hold = b.get("modifier").and_then(|x| x.as_i64()).unwrap_or(0)
                        & omsi_content::input::KEY_HOLD as i64;
                    b["scan_code"] = json!(0);
                    b["modifier"] = json!(hold);
                }
            }
        }
        save_keys(f.state, row.action.starts_with("vr_"));
    }
    f.ui.paragraph(
        note,
        Vec2::new(inner.x, by + 44.0),
        inner.w,
        12.0,
        Weight::Regular,
        TEXT_FAINT,
    );
}

/// The window open over the keyboard page, if any (the page sees no mouse meanwhile).
pub fn keyboard_dialog(f: &mut Frame, view: &mut KeyboardView) {
    if f.state.omsi2_engine() {
        view.dialog = None;
        return;
    }
    capture(f, view);
    let esc = f.ui.input.keys.contains(&Key::Escape);
    match view.dialog.take() {
        Some(Dialog::Reload { backup, pads }) => {
            if !esc {
                reload_dialog(f, view, backup, pads);
            }
        }
        Some(Dialog::Add {
            query,
            chosen,
            key,
            waiting,
        }) => {
            if !esc || waiting {
                add_dialog(f, view, query, chosen, key, waiting);
            }
        }
        None => {}
    }
}

fn reload_dialog(f: &mut Frame, view: &mut KeyboardView, mut backup: bool, mut pads: bool) {
    let Some(orig) = view.original.clone().flatten() else {
        return;
    };
    let labels = labels(f);
    let plan = reload_plan(&f.state.keybindings, &orig, &|a| labels.text(a));
    let rows_h = (plan.changes.len().max(1) as f32 * 36.0 + 34.0).min(300.0);
    let inside = dialog(
        f.ui,
        880.0,
        56.0 + 40.0 + rows_h + 120.0 + 76.0,
        "Reload controls from OMSI 2",
        "your original OMSI 2 folder · read only",
    );
    let (buttons, body) = dialog_foot(f.ui, inside);
    let mut x = body.x;
    for (text, c) in [
        (
            omsi_ui::tr("%{n} already identical").replace("%{n}", &plan.identical.to_string()),
            OK,
        ),
        (
            omsi_ui::tr("%{n} will change").replace("%{n}", &plan.changes.len().to_string()),
            accent(),
        ),
        (
            omsi_ui::tr("%{n} only in the Depot, kept as they are")
                .replace("%{n}", &plan.depot_only.to_string()),
            TEXT_SOFT,
        ),
    ] {
        x += pill(f.ui, x, body.y + 10.0, &text, c) + 8.0;
    }
    let t = Rect::new(body.x, body.y + 34.0, body.w, rows_h);
    let cols = [t.x + 12.0, t.x + t.w * 0.4, t.x + t.w * 0.72];
    f.ui.p().rect(Rect::new(t.x, t.y, t.w, 1.0), EDGE);
    for (s, x) in [
        ("Action", cols[0]),
        ("Your key now", cols[1]),
        ("OMSI 2 key", cols[2]),
    ] {
        f.ui.text_in(
            s,
            Rect::new(x, t.y, 200.0, 32.0),
            12.5,
            Weight::Regular,
            TEXT_DIM,
            Align::Left,
        );
    }
    let names = view.names.clone();
    let changes: Vec<(String, String, String)> = plan
        .changes
        .iter()
        .map(|(a, now, omsi)| (a.clone(), names.list(f.host, now), names.list(f.host, omsi)))
        .collect();
    f.ui.scroll_area(
        "kb-reload-list",
        Rect::new(t.x, t.y + 34.0, t.w, t.h - 34.0),
        &mut |ui, v| {
            if changes.is_empty() {
                ui.text_in(
                    "Nothing to change: every key is already the original OMSI 2's.",
                    Rect::new(v.x + 12.0, v.y, v.w, 32.0),
                    13.0,
                    Weight::Regular,
                    TEXT_DIM,
                    Align::Left,
                );
                return 32.0;
            }
            for (k, (label, now, omsi)) in changes.iter().enumerate() {
                let y = v.y + k as f32 * 36.0;
                ui.p().rect(Rect::new(v.x, y, v.w, 1.0), EDGE);
                ui.text_in(
                    label,
                    Rect::new(cols[0], y, cols[1] - cols[0] - 10.0, 36.0),
                    13.0,
                    Weight::Regular,
                    TEXT,
                    Align::Left,
                );
                ui.text_in(
                    now,
                    Rect::new(cols[1], y, cols[2] - cols[1] - 40.0, 36.0),
                    12.5,
                    Weight::Medium,
                    TEXT_SOFT,
                    Align::Left,
                );
                ui.icon(
                    "o-chevron",
                    Vec2::new(cols[2] - 24.0, y + 18.0),
                    13.0,
                    TEXT_DIM,
                );
                ui.text_in(
                    omsi,
                    Rect::new(cols[2], y, v.right() - cols[2], 36.0),
                    12.5,
                    Weight::Medium,
                    accent(),
                    Align::Left,
                );
            }
            changes.len() as f32 * 36.0
        },
    );
    let mut y = t.bottom() + 12.0;
    f.ui.toggle(
        "kb-reload-backup",
        Rect::new(body.x, y, body.w, 30.0),
        &mut backup,
        "Keep a backup of my current layout (and controllers) first",
    );
    y += 34.0;
    f.ui.toggle(
        "kb-reload-pads",
        Rect::new(body.x, y, body.w, 30.0),
        &mut pads,
        "Also read the game controller assignments",
    );
    y += 38.0;
    f.ui.paragraph("After reloading, every key the original OMSI 2 has matches its keyboard file, and each controller it has is set up as there (matched by its name); keys and controllers only the Depot has are kept. The original files are only read, never written to.", Vec2::new(body.x, y), body.w, 12.0, Weight::Regular, TEXT_DIM);
    let apply = Rect::new(buttons.right() - 170.0, buttons.y, 170.0, 34.0);
    if f.ui.button(
        "kb-reload-apply",
        apply,
        "Reload and apply",
        None,
        ButtonKind::Primary,
    ) {
        match core::reload_keybindings(&plan.merged, backup) {
            Ok(kept) => {
                f.state.keybindings_error.clear();
                if let Ok(k) = core::get_keybindings() {
                    f.state.keybindings = k;
                }
                // (as any key changed here: the game uses these keys, not a ready-made layout)
                use_custom_keys(f.state);
                let mut said = omsi_ui::tr("Keys reloaded from OMSI 2 (%{n} changed).")
                    .replace("%{n}", &plan.changes.len().to_string());
                if let Some(p) = kept {
                    said.push_str(
                        &omsi_ui::tr(" Your layout before is kept as %{file}.").replace(
                            "%{file}",
                            &p.file_name()
                                .map(|f| f.to_string_lossy().to_string())
                                .unwrap_or_default(),
                        ),
                    );
                }
                if pads {
                    match reload_pads(f, view, backup) {
                        Ok(Some(n)) => said.push_str(&omsi_ui::tr(" Game controllers read too (%{n} from OMSI 2; the Depot's others kept).").replace("%{n}", &n.to_string())),
                        Ok(None) => {}
                        Err(e) => said.push_str(&omsi_ui::tr(" The game controllers were not read: %{error}").replace("%{error}", &e.to_string())),
                    }
                }
                f.state.set_status(said, false);
                view.original = None;
            }
            Err(e) => f.state.set_status(format!("{e:#}"), true),
        }
        return;
    }
    if f.ui.button(
        "kb-reload-cancel",
        Rect::new(apply.x - 120.0, buttons.y, 110.0, 34.0),
        "Cancel",
        None,
        ButtonKind::Normal,
    ) {
        return;
    }
    view.dialog = Some(Dialog::Reload { backup, pads });
}

/// The original OMSI 2's controller assignments (`Inputs/gamectrler.cfg`): each of its devices
/// in place of the Depot's of that name (or added), the Depot's others kept; with `backup`, the
/// Depot's file is copied beside itself first. The number of devices read; `None` when it has
/// no such file.
fn reload_pads(
    f: &mut Frame,
    view: &mut KeyboardView,
    backup: bool,
) -> Result<Option<usize>, String> {
    let root = std::path::PathBuf::from(&f.state.config.root);
    let Some(file) = omsi_cfg::original_inputs_file(&root, "gamectrler.cfg") else {
        return Ok(None);
    };
    let mut theirs = omsi_content::input::gamectrler::read(&file).map_err(|e| e.to_string())?;
    // (as reading it in place does: a wheel's 0/0 force feedback scale is the Depot's default)
    for d in &mut theirs {
        if d.ff_scale == Some((0.0, 0.0)) {
            d.ff_scale = None;
        }
    }
    let own_file = f.host.controllers_file();
    let mut devices = match &own_file {
        Some(f) if f.is_file() => {
            omsi_content::input::gamectrler::read(f).map_err(|e| e.to_string())?
        }
        _ => Vec::new(),
    };
    if backup {
        if let Some(f) = &own_file {
            core::backup_beside(f).map_err(|e| format!("{e:#}"))?;
        }
    }
    let n = theirs.len();
    merge_devices(&mut devices, theirs);
    f.host.save_controllers(&devices)?;
    view.pads_read = true;
    Ok(Some(n))
}

fn add_dialog(
    f: &mut Frame,
    view: &mut KeyboardView,
    mut query: String,
    mut chosen: Option<String>,
    mut key: Option<Chord>,
    mut waiting: bool,
) {
    let labels = labels(f);
    let inside = dialog(
        f.ui,
        820.0,
        470.0,
        "Add a control",
        "choose what it does, then press the key",
    );
    let (buttons, body) = dialog_foot(f.ui, inside);
    let right_w = 280.0;
    let left = Rect::new(body.x, body.y, body.w - right_w - 14.0, body.h);
    let right = Rect::new(left.right() + 14.0, body.y, right_w, body.h);
    f.ui.text_input(
        "kb-add-search",
        Rect::new(left.x, left.y, left.w, 34.0),
        &mut query,
        "Search actions without a key...",
        Some("search"),
    );
    let q = query.trim().to_lowercase();
    // the actions the game knows (the language files' and the mods' event tables) that have no
    // key yet
    let bound: std::collections::HashSet<String> = ["vehicles", "game"]
        .iter()
        .flat_map(|s| {
            f.state
                .keybindings
                .get(*s)
                .and_then(|a| a.as_array())
                .cloned()
                .unwrap_or_default()
        })
        .filter(|b| b.get("scan_code").and_then(|x| x.as_i64()).unwrap_or(0) != 0)
        .filter_map(|b| {
            b.get("action")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())
        })
        .collect();
    let mut list: Vec<(String, String)> = labels
        .events()
        .into_iter()
        .filter(|(a, _)| !bound.contains(a))
        .collect();
    if f.state.omsi2_engine() {
        list.retain(|(a, _)| crate::omsi2_map::action_in_omsi2(a));
    }
    if !q.is_empty() {
        list.retain(|(a, label)| a.to_lowercase().contains(&q) || matches(label, &q));
    }
    // (a name no list has, as a mod's readme gives one for its bus's own trigger: added as
    // OMSI's own key dialog would, when typed whole)
    let typed = query.trim().to_string();
    let custom = list.is_empty()
        && typed.len() > 1
        && typed.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if custom {
        list.push((
            typed.clone(),
            omsi_ui::tr("\"%{name}\": a bus's own trigger (from a mod's readme)")
                .replace("%{name}", &typed),
        ));
    }
    let mut picked = None;
    let sel = chosen.clone();
    f.ui.scroll_area(
        "kb-add-list",
        Rect::new(left.x, left.y + 44.0, left.w, left.h - 44.0),
        &mut |ui, v| {
            let rh = 48.0;
            for (k, (a, label)) in list.iter().enumerate() {
                let rr = Rect::new(v.x, v.y + k as f32 * rh, v.w - 10.0, rh - 4.0);
                let on = sel.as_deref() == Some(a.as_str());
                let (hover, _, clicked) = ui.interact_key(id_of(&format!("kb-add-{a}")), rr);
                if on {
                    ui.p().rounded(rr, 5.0, accent().alpha(0.14));
                    ui.p().rounded_border(rr, 5.0, 1.0, accent().alpha(0.6));
                } else if hover {
                    ui.p().rounded(rr, 5.0, Color::WHITE.alpha(0.05));
                }
                ui.text_in(
                    label,
                    Rect::new(rr.x + 10.0, rr.y + 4.0, rr.w - 20.0, 20.0),
                    13.5,
                    Weight::Medium,
                    TEXT,
                    Align::Left,
                );
                let g = GROUPS[group_of(0, a)];
                ui.text_in(
                    &omsi_ui::tr("%{group} · no key yet").replace("%{group}", &omsi_ui::tr(g)),
                    Rect::new(rr.x + 10.0, rr.y + 24.0, rr.w - 20.0, 16.0),
                    12.0,
                    Weight::Regular,
                    TEXT_DIM,
                    Align::Left,
                );
                if clicked {
                    picked = Some(a.clone());
                }
            }
            list.len() as f32 * rh
        },
    );
    if let Some(a) = picked {
        if chosen.as_deref() != Some(a.as_str()) {
            key = None;
        }
        chosen = Some(a);
        waiting = true;
    }
    let mut y = right.y;
    match &chosen {
        None => {
            f.ui.paragraph(
                "Choose an action on the left, then press its key.",
                Vec2::new(right.x, y),
                right.w,
                13.0,
                Weight::Regular,
                TEXT_DIM,
            );
        }
        Some(a) => {
            f.ui.text_in(
                &labels.text(a),
                Rect::new(right.x, y, right.w, 18.0),
                12.5,
                Weight::Medium,
                TEXT_SOFT,
                Align::Left,
            );
            y += 24.0;
            let field = Rect::new(right.x, y, right.w, 34.0);
            let (h, _, c) = f.ui.interact_key(id_of("kb-add-key"), field);
            f.ui.p().rounded(field, 4.0, FIELD);
            f.ui.p().rounded_border(
                field,
                4.0,
                1.0,
                if waiting {
                    accent()
                } else if h {
                    TEXT_DIM
                } else {
                    EDGE
                },
            );
            let text = if waiting {
                "Press a key now...".to_string()
            } else {
                key.map(|k| view.names.name(f.host, k))
                    .unwrap_or_else(|| "Click, then press a key".into())
            };
            f.ui.text_in(
                &text,
                Rect::new(field.x + 12.0, field.y, field.w - 24.0, field.h),
                13.0,
                Weight::Regular,
                if waiting || key.is_some() {
                    TEXT
                } else {
                    TEXT_FAINT
                },
                Align::Left,
            );
            if c {
                waiting = true;
            }
            y += 46.0;
            if let Some(k) = key {
                let rows = rows(&f.state.keybindings, None, None, &|a| labels.text(a));
                let others: Vec<String> = rows
                    .iter()
                    .filter(|r| r.keys.contains(&k))
                    .map(|r| r.label.clone())
                    .collect();
                if !others.is_empty() {
                    let t = omsi_ui::tr("%{key} is also used by %{actions}. Both will fire together. You can still add it.").replace("%{key}", &view.names.name(f.host, k)).replace("%{actions}", &others.join(", "));
                    y += f.ui.paragraph(
                        &t,
                        Vec2::new(right.x, y),
                        right.w,
                        12.0,
                        Weight::Regular,
                        WARN,
                    ) + 10.0;
                }
            }
            f.ui.paragraph("Only actions the game knows are listed, so you cannot create one that does nothing; a bus's own trigger is added when its whole name is typed.", Vec2::new(right.x, y), right.w, 12.0, Weight::Regular, TEXT_FAINT);
        }
    }
    let shared = key.is_some_and(|k| {
        rows(&f.state.keybindings, None, None, &|a| a.to_string())
            .iter()
            .any(|r| r.keys.contains(&k))
    });
    let add = Rect::new(buttons.right() - 140.0, buttons.y, 140.0, 34.0);
    let ready = chosen.is_some() && key.is_some();
    if ready
        && f.ui.button(
            "kb-add-apply",
            add,
            if shared { "Add anyway" } else { "Add" },
            None,
            ButtonKind::Primary,
        )
    {
        if let (Some(a), Some(k)) = (chosen.clone(), key) {
            // (a game action goes with the game's keys, any other with the bus's)
            let sec = if f
                .state
                .keybindings
                .get("game")
                .and_then(|x| x.as_array())
                .is_some_and(|x| {
                    x.iter()
                        .any(|b| b.get("action").and_then(|v| v.as_str()) == Some(a.as_str()))
                }) {
                1
            } else {
                0
            };
            if let Some(list) = f
                .state
                .keybindings
                .get_mut(SECTIONS[sec])
                .and_then(|x| x.as_array_mut())
            {
                // (a listed action without a key gets its key in its own entry)
                match list.iter_mut().find(|b| {
                    b.get("action").and_then(|v| v.as_str()) == Some(a.as_str())
                        && b.get("scan_code").and_then(|v| v.as_i64()).unwrap_or(0) == 0
                }) {
                    Some(b) => {
                        let hold = b.get("modifier").and_then(|x| x.as_i64()).unwrap_or(0)
                            & omsi_content::input::KEY_HOLD as i64;
                        b["scan_code"] = json!(k.0);
                        b["modifier"] = json!(k.1 | hold);
                    }
                    None => list.push(json!({ "action": a, "scan_code": k.0, "modifier": k.1 })),
                }
            }
            save_keys(f.state, a.starts_with("vr_"));
            view.chosen = Some((sec, a));
            view.list = 0;
        }
        return;
    }
    if !ready {
        f.ui.p().rounded(add, 4.0, Color::WHITE.alpha(0.04));
        f.ui.text_in("Add", add, 13.0, Weight::Bold, TEXT_FAINT, Align::Center);
    }
    if f.ui.button(
        "kb-add-cancel",
        Rect::new(add.x - 120.0, buttons.y, 110.0, 34.0),
        "Cancel",
        None,
        ButtonKind::Normal,
    ) {
        return;
    }
    view.dialog = Some(Dialog::Add {
        query,
        chosen,
        key,
        waiting,
    });
}

/// Settings → Driving keys: "Custom controls", the keys of the Controls page.
pub fn use_custom_keys(state: &mut State) -> bool {
    if state.settings.get("drive_keys").and_then(|v| v.as_str()) == Some("omsi") {
        return false;
    }
    state.settings["drive_keys"] = json!("omsi");
    state.settings_dirty = 0.3;
    state.set_status(
        "Driving keys: Custom controls - the game uses the keys of this page.",
        false,
    );
    true
}

pub fn save_keys(state: &mut State, vr_binding: bool) {
    // (with OMSI 2 chosen the keys are only shown: the file may be the OMSI 2 folder's own)
    if state.omsi2_engine() {
        return;
    }
    match core::save_keybindings(&state.keybindings) {
        Ok(()) => {
            state.keybindings_error.clear();
            if let Ok(k) = core::get_keybindings() {
                state.keybindings = k;
            }
            // a key changed is a key the player wants to use: with a ready-made layout it
            // would be ignored wherever that layout has a key of its own
            if !vr_binding && use_custom_keys(state) {
                state.set_status("Key bindings saved; Driving keys switched to Custom controls so the game uses them.", false);
            } else {
                state.set_status("Key bindings saved.", false);
            }
        }
        Err(e) => {
            state.keybindings_error = format!("{e:#}");
            state.set_status(format!("{e:#}"), true);
        }
    }
}
