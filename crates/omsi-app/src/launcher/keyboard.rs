//! The Controls page's keyboard on a computer, as the Windows 11 concept draws it: the actions
//! in groups on the left, the group's actions with their key and second key in the middle,
//! the chosen action on the right; above them, how the keys compare with the original OMSI 2's
//! and its Reload. A key used twice is flagged, never refused: both actions fire, as in OMSI 2.

use super::pages::{action_text, control_names, save_keys};
use super::theme::*;
use super::ui::{id_of, matches, ButtonKind, Key};
use super::Launcher;
use glam::Vec2;
use omsi_launcher_lib as core;
use omsi_ui::paint::Align;
use omsi_ui::{Color, Rect, Weight};
use serde_json::{json, Value};

/// The groups of actions, `All` first.
pub const GROUPS: [&str; 8] = ["All", "Driving", "Cab and lights", "Doors and tickets", "Cameras and views", "Game and menus", "VR", "Advanced"];
/// The lists at the groups' foot, after the groups.
pub const FILTERS: [&str; 3] = ["Conflicts", "Changed by me", "Not assigned"];

/// The sections of the keys, as `keyboard.cfg` has them (the page's `capturing` counts them so).
const SECTIONS: [&str; 2] = ["vehicles", "game"];

/// The group (an index of `GROUPS`, never 0) of `action` in the section `sec`.
pub fn group_of(sec: usize, action: &str) -> usize {
    let a = action.to_ascii_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| a.contains(w));
    if a.starts_with("vr_") {
        return 6;
    }
    if a.starts_with("view_") || has(&["camera", "zoom"]) {
        return 4;
    }
    if has(&["door", "ticket", "kneel", "haltewunsch", "stop_request", "ramp", "cash", "change", "fare", "printer"]) {
        return 3;
    }
    if has(&["throttle", "brake", "clutch", "steering", "blinker", "horn", "gear", "automatic_", "retarder", "kickdown", "enginestart", "engine"]) || (a.starts_with("kw_s_") && a.ends_with("_fest")) {
        return 1;
    }
    if sec == 1 {
        return 5;
    }
    if has(&["licht", "light", "lamp", "wiper", "scheinwerfer", "heiz", "heat", "fan", "cp_", "kw_"]) {
        return 2;
    }
    7
}

/// A key as the file keeps it: its scan code and its Shift / Ctrl / Alt (the "held" bit, the
/// action's, left out).
type Chord = (i64, i64);

fn chord_of(b: &Value) -> Chord {
    let scan = b.get("scan_code").and_then(|x| x.as_i64()).unwrap_or(0);
    let m = b.get("modifier").and_then(|x| x.as_i64()).unwrap_or(0) & !(omsi_content::input::KEY_HOLD as i64);
    (scan, m)
}

/// The keys of `action` in the section `key` of `kb`, unbound ones left out.
fn keys_of(kb: &Value, key: &str, action: &str) -> Vec<Chord> {
    kb.get(key)
        .and_then(|a| a.as_array())
        .map(|a| a.iter().filter(|b| b.get("action").and_then(|x| x.as_str()) == Some(action)).map(chord_of).filter(|c| c.0 != 0).collect())
        .unwrap_or_default()
}

fn has_action(kb: &Value, key: &str, action: &str) -> bool {
    kb.get(key).and_then(|a| a.as_array()).is_some_and(|a| a.iter().any(|b| b.get("action").and_then(|x| x.as_str()) == Some(action)))
}

fn same_keys(a: &[Chord], b: &[Chord]) -> bool {
    let (mut a, mut b) = (a.to_vec(), b.to_vec());
    a.sort();
    b.sort();
    a == b
}

/// One action of the page: its entries in the file (one per key), what it is, and how its
/// keys stand.
#[derive(Debug, Clone)]
pub struct Row {
    pub sec: usize,
    pub action: String,
    pub label: String,
    /// The indices of its entries in the section, in the file's order.
    pub entries: Vec<usize>,
    /// Their keys, the same order (scan 0: none).
    pub keys: Vec<Chord>,
    pub group: usize,
    /// Its keys differ from the original OMSI 2's.
    pub changed: bool,
    /// One of its keys is another action's too.
    pub shared: bool,
}

impl Row {
    pub fn unassigned(&self) -> bool {
        self.keys.iter().all(|k| k.0 == 0)
    }

    /// Whether the list `list` (an index of `GROUPS`, then of `FILTERS`) shows it.
    pub fn in_list(&self, list: usize) -> bool {
        match list {
            0 => true,
            l if l < GROUPS.len() => self.group == l,
            8 => self.shared,
            9 => self.changed,
            _ => self.unassigned(),
        }
    }
}

/// The page's actions from `kb` (as `get_keybindings` gives it), compared with the original
/// OMSI 2's keys `original` when there are any; `label` names an action.
pub fn rows(kb: &Value, original: Option<&Value>, label: &dyn Fn(&str) -> String) -> Vec<Row> {
    let mut out: Vec<Row> = Vec::new();
    for (sec, key) in SECTIONS.iter().enumerate() {
        let Some(list) = kb.get(*key).and_then(|a| a.as_array()) else { continue };
        for (i, b) in list.iter().enumerate() {
            let action = b.get("action").and_then(|x| x.as_str()).unwrap_or("").to_string();
            match out.iter_mut().find(|r| r.sec == sec && r.action == action) {
                Some(r) => {
                    r.entries.push(i);
                    r.keys.push(chord_of(b));
                }
                None => out.push(Row { sec, label: label(&action), group: group_of(sec, &action), entries: vec![i], keys: vec![chord_of(b)], changed: false, shared: false, action }),
            }
        }
    }
    // (a key on several actions is a conflict unless OMSI 2 as it comes has it so: its own
    // layout puts the manual and the automatic gears, the editor's keys and the driving ones
    // on the same keys on purpose)
    let mut on: std::collections::HashMap<Chord, Vec<&str>> = std::collections::HashMap::new();
    for r in &out {
        let mut mine: Vec<Chord> = r.keys.iter().copied().filter(|k| k.0 != 0).collect();
        mine.sort();
        mine.dedup();
        for k in mine {
            on.entry(k).or_default().push(&r.action);
        }
    }
    let clash: std::collections::HashSet<Chord> = on.iter().filter(|(k, actions)| actions.len() > 1 && actions.iter().any(|a| !factory_keys(a).contains(k))).map(|(k, _)| *k).collect();
    for r in &mut out {
        r.shared = r.keys.iter().any(|k| clash.contains(k));
        if let Some(o) = original {
            let key = SECTIONS[r.sec];
            if has_action(o, key, &r.action) {
                let now: Vec<Chord> = r.keys.iter().copied().filter(|k| k.0 != 0).collect();
                r.changed = !same_keys(&now, &keys_of(o, key, &r.action));
            }
        }
    }
    out
}

/// What reloading from the original OMSI 2 does: the actions already the same, the ones that
/// change (the action, its keys now, the original's), the ones only the Depot has (kept), and
/// the keys after it.
pub struct Reload {
    pub identical: usize,
    pub changes: Vec<(String, String, String)>,
    pub depot_only: usize,
    pub merged: Value,
}

fn names_of(keys: &[Chord]) -> String {
    if keys.is_empty() {
        return "none".into();
    }
    keys.iter().map(|k| crate::keys::key_name(k.0, k.1)).collect::<Vec<_>>().join(", ")
}

/// Every action the original OMSI 2 has gets its keys (its entries in place of the current
/// ones, where the first of those stood); the actions only the Depot has stay as they are.
pub fn reload_plan(current: &Value, original: &Value, label: &dyn Fn(&str) -> String) -> Reload {
    let mut merged = current.clone();
    let (mut identical, mut changes, mut depot_only) = (0, Vec::new(), 0);
    for key in SECTIONS {
        let now: Vec<Value> = current.get(key).and_then(|a| a.as_array()).cloned().unwrap_or_default();
        let orig: Vec<Value> = original.get(key).and_then(|a| a.as_array()).cloned().unwrap_or_default();
        let mut actions: Vec<String> = Vec::new();
        for b in now.iter().chain(orig.iter()) {
            let a = b.get("action").and_then(|x| x.as_str()).unwrap_or("").to_string();
            if !actions.contains(&a) {
                actions.push(a);
            }
        }
        let mut out = now.clone();
        for a in &actions {
            let from: Vec<Value> = orig.iter().filter(|b| b.get("action").and_then(|x| x.as_str()) == Some(a.as_str())).cloned().collect();
            if from.is_empty() {
                depot_only += 1;
                continue;
            }
            let (k_now, k_orig) = (keys_of(current, key, a), keys_of(original, key, a));
            if same_keys(&k_now, &k_orig) {
                identical += 1;
                continue;
            }
            changes.push((label(a), names_of(&k_now), names_of(&k_orig)));
            let at = out.iter().position(|b| b.get("action").and_then(|x| x.as_str()) == Some(a.as_str())).unwrap_or(out.len());
            out.retain(|b| b.get("action").and_then(|x| x.as_str()) != Some(a.as_str()));
            let at = at.min(out.len());
            for (k, b) in from.into_iter().enumerate() {
                out.insert(at + k, b);
            }
        }
        merged[key] = json!(out);
    }
    Reload { identical, changes, depot_only, merged }
}

/// OMSI 2's own keys for `action`, as it comes (the factory assignment).
pub fn factory_keys(action: &str) -> Vec<Chord> {
    omsi_content::input::stock_keys::STOCK_KEYS
        .iter()
        .filter(|(a, _, _)| a.eq_ignore_ascii_case(action))
        .map(|(_, scan, m)| (*scan as i64, (*m & !omsi_content::input::KEY_HOLD) as i64))
        .collect()
}

/// A window over the keyboard page.
pub enum Dialog {
    /// Reload from the original OMSI 2: keep a backup first, read the controllers too.
    Reload { backup: bool, pads: bool },
    /// Add a control: the search, the action chosen, the key pressed for it.
    Add { query: String, chosen: Option<String>, key: Option<Chord>, waiting: bool },
}

/// What the page keeps between frames.
#[derive(Default)]
pub struct KeyboardView {
    /// The list shown: an index of `GROUPS`, then of `FILTERS`.
    pub list: usize,
    pub query: String,
    /// The action chosen (its section and name).
    pub chosen: Option<(usize, String)>,
    pub dialog: Option<Dialog>,
    /// The original OMSI 2's keys, once read (`None` inside: it has none).
    pub original: Option<Option<Value>>,
    /// When its controls file was saved, and when the keys were last reloaded from it.
    pub saved: Option<u64>,
    pub reloaded: Option<u64>,
}

impl KeyboardView {
    fn load(&mut self) {
        if self.original.is_none() {
            self.original = Some(core::original_keybindings().ok().flatten());
            self.saved = core::original_keys_saved();
            self.reloaded = core::keys_reloaded_at();
        }
    }
}

/// A date of a Unix time, day first.
fn day(t: u64) -> String {
    let days = (t / 86400) as i64;
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{d:02}/{m:02}/{y:04}")
}

/// A key as a small key cap; a dim dash for none. Returns its rectangle.
fn cap(l: &mut Launcher, x: f32, cy: f32, text: &str, edge: Color) -> Rect {
    let w = l.ui.width(text, 12.0, Weight::Medium) + 16.0;
    let r = Rect::new(x, cy - 11.0, w.max(26.0), 22.0);
    l.ui.p().rounded(r, 4.0, Color::WHITE.alpha(0.05));
    l.ui.p().rounded_border(r, 4.0, 1.0, edge);
    l.ui.text_in(text, r, 12.0, Weight::Medium, TEXT, Align::Center);
    r
}

/// A small label in a pill (as "changed", "shared key").
fn pill(l: &mut Launcher, x: f32, cy: f32, text: &str, c: Color) -> f32 {
    let w = l.ui.width(text, 11.5, Weight::Medium) + 18.0;
    let r = Rect::new(x, cy - 10.0, w, 20.0);
    l.ui.p().rounded(r, 10.0, Color::WHITE.alpha(0.04));
    l.ui.p().rounded_border(r, 10.0, 1.0, EDGE);
    l.ui.text_in(text, r, 11.5, Weight::Medium, c, Align::Center);
    w
}

/// The keyboard tab on a computer, in `body` (under the page's tabs).
pub fn draw(l: &mut Launcher, body: Rect) {
    l.pages.kb.load();
    // (with a window open, the key goes to it: see `dialog`)
    if l.pages.kb.dialog.is_none() {
        capture(l);
    }
    let names = control_names(l);
    let label = |a: &str| action_text(names, a);
    let original = l.pages.kb.original.clone().flatten();
    let all = rows(&l.state.keybindings, original.as_ref(), &label);
    let changed = all.iter().filter(|r| r.changed).count();
    // (the chip at the tabs' right: how far the layout is from the original)
    let chip = if original.is_none() { "Your layout".to_string() } else if changed == 0 { "Your layout · same as your OMSI 2".into() } else { format!("Your layout · {changed} changed from your OMSI 2") };
    let cw = l.ui.width(&chip, 12.0, Weight::Regular) + 24.0;
    let cr = Rect::new(body.right() - cw, body.y - 44.0, cw, 26.0);
    l.ui.p().rounded(cr, 13.0, Color::WHITE.alpha(0.04));
    l.ui.p().rounded_border(cr, 13.0, 1.0, EDGE);
    l.ui.text_in(&chip, cr, 12.0, Weight::Regular, TEXT_DIM, Align::Center);
    let mut y = body.y;
    y += preset_bar(l, Rect::new(body.x, y, body.w, 0.0));
    y += import_bar(l, Rect::new(body.x, y, body.w, 56.0), original.is_some(), changed) + 12.0;
    let cols = Rect::new(body.x, y, body.w, body.bottom() - y);
    let gw = omsi_launcher_ui::kit::fit(210.0, cols.w);
    let dw = omsi_launcher_ui::kit::fit(330.0, cols.w);
    let groups = Rect::new(cols.x, cols.y, gw, cols.h);
    let detail = Rect::new(cols.right() - dw, cols.y, dw, cols.h);
    let list = Rect::new(groups.right() + 12.0, cols.y, detail.x - groups.right() - 24.0, cols.h);
    groups_card(l, groups, &all);
    list_card(l, list, &all);
    detail_card(l, detail, &all, original.as_ref());
    if !l.state.keybindings_error.is_empty() {
        let e = l.state.keybindings_error.clone();
        l.ui.text_in(&e, Rect::new(body.x, body.bottom() + 4.0, body.w, 18.0), 12.0, Weight::Medium, DANGER, Align::Left);
    }
}

/// A key pressed while an entry waits for one (`pages.capturing`), or for the Add window.
fn capture(l: &mut Launcher) {
    let Some(code) = l.ui.input.raw_key else { return };
    use winit::keyboard::KeyCode as K;
    if matches!(code, K::ShiftLeft | K::ShiftRight | K::ControlLeft | K::ControlRight | K::AltLeft | K::AltRight | K::SuperLeft | K::SuperRight) {
        return;
    }
    let add_waiting = matches!(l.pages.kb.dialog, Some(Dialog::Add { waiting: true, .. }));
    if l.pages.capturing.is_none() && !add_waiting {
        return;
    }
    l.ui.input.raw_key = None;
    if code == K::Escape {
        l.pages.capturing = None;
        if let Some(Dialog::Add { waiting, .. }) = &mut l.pages.kb.dialog {
            *waiting = false;
        }
        // (Escape leaves the key as it was; it does not close the window too)
        l.ui.input.keys.retain(|k| *k != Key::Escape);
        return;
    }
    let Some(scan) = crate::keys::dik_code(code) else {
        l.state.set_status(format!("{code:?} has no DirectInput scan code the game understands."), true);
        l.pages.capturing = None;
        return;
    };
    let m = omsi_content::input::chord(l.ui.input.shift, l.ui.input.ctrl, l.ui.input.alt) as i64;
    if let Some(Dialog::Add { key, waiting, .. }) = &mut l.pages.kb.dialog {
        *key = Some((scan as i64, m));
        *waiting = false;
        return;
    }
    let Some((sec, idx)) = l.pages.capturing.take() else { return };
    let section = SECTIONS[sec];
    let vr = l.state.keybindings.get(section).and_then(|a| a.as_array()).and_then(|a| a.get(idx)).and_then(|b| b.get("action")).and_then(|a| a.as_str()).is_some_and(|a| a.starts_with("vr_"));
    if let Some(b) = l.state.keybindings.get_mut(section).and_then(|a| a.as_array_mut()).and_then(|a| a.get_mut(idx)) {
        // (the entry's "held" bit is the action's, not the key's: it stays)
        let hold = b.get("modifier").and_then(|x| x.as_i64()).unwrap_or(0) & omsi_content::input::KEY_HOLD as i64;
        b["scan_code"] = json!(scan);
        b["modifier"] = json!(m | hold);
    }
    save_keys(l, vr);
}

/// The ready-made driving keys win over this page's: say so, with the switch (none with
/// Custom controls). Returns the height used.
fn preset_bar(l: &mut Launcher, r: Rect) -> f32 {
    let preset = l.state.settings.get("drive_keys").and_then(|v| v.as_str()).unwrap_or("simple").to_string();
    if preset == "omsi" {
        return 0.0;
    }
    let name = match preset.as_str() {
        "wasd" => "W A S D only",
        "arrows" => "Arrow keys only",
        _ => "W A S D + arrows",
    };
    let bar = Rect::new(r.x, r.y, r.w, 40.0);
    l.ui.p().rounded(bar, 6.0, Color::WHITE.alpha(0.04));
    l.ui.p().rounded_border(bar, 6.0, 1.0, EDGE);
    l.ui.icon("info", Vec2::new(bar.x + 18.0, bar.center().y), 16.0, accent());
    let bw = 150.0;
    let text = format!("Driving keys: {name} (Settings) drive the bus and win over the list below; any key changed here switches to your own layout.");
    let text = omsi_launcher_ui::kit::fit_text(&mut l.ui, &text, 12.5, bar.w - bw - 60.0);
    l.ui.text_in(&text, Rect::new(bar.x + 36.0, bar.y, bar.w - bw - 50.0, bar.h), 12.5, Weight::Regular, TEXT_SOFT, Align::Left);
    if l.ui.button("kb-use-custom", Rect::new(bar.right() - bw - 6.0, bar.y + 5.0, bw, 30.0), "Use these keys", None, ButtonKind::Normal) {
        super::pages::use_custom_keys(l);
    }
    bar.h + 10.0
}

/// How the keys stand against the original OMSI 2's, and its Reload. Returns its height.
fn import_bar(l: &mut Launcher, r: Rect, found: bool, changed: usize) -> f32 {
    l.ui.p().rounded(r, 6.0, accent().alpha(0.08));
    l.ui.p().rounded_border(r, 6.0, 1.0, accent().alpha(0.45));
    l.ui.icon("o-folder", Vec2::new(r.x + 20.0, r.center().y), 16.0, TEXT_SOFT);
    let (lead, rest) = if !found {
        ("No controls file in your OMSI 2 folder".to_string(), " · the keys are the Depot's own.".to_string())
    } else {
        let lead = if changed == 0 { "Same keys as your original OMSI 2".to_string() } else if changed == 1 { "1 key differs from your original OMSI 2".to_string() } else { format!("{changed} keys differ from your original OMSI 2") };
        let saved = l.pages.kb.saved.map(|t| format!(", saved {}", day(t))).unwrap_or_default();
        let reloaded = l.pages.kb.reloaded.map(day).unwrap_or_else(|| "never".into());
        (lead, format!(" · Controls file found in your OMSI 2 folder{saved}. Last reloaded: {reloaded}."))
    };
    let bw = [l.ui.width("Preview changes", 13.0, Weight::Medium) + 32.0, l.ui.width("Reload from OMSI 2", 13.0, Weight::Bold) + 50.0];
    let room = r.w - 44.0 - bw[0] - bw[1] - 36.0;
    let lw = l.ui.width(&lead, 12.5, Weight::Bold);
    l.ui.text_in(&lead, Rect::new(r.x + 40.0, r.y, lw + 2.0, r.h), 12.5, Weight::Bold, TEXT, Align::Left);
    let rest = omsi_launcher_ui::kit::fit_text(&mut l.ui, &rest, 12.5, (room - lw).max(40.0));
    l.ui.text_in(&rest, Rect::new(r.x + 40.0 + lw, r.y, (room - lw).max(40.0), r.h), 12.5, Weight::Regular, TEXT_SOFT, Align::Left);
    if found {
        let b2 = Rect::new(r.right() - 12.0 - bw[1], r.center().y - 17.0, bw[1], 34.0);
        let b1 = Rect::new(b2.x - 10.0 - bw[0], b2.y, bw[0], 34.0);
        if l.ui.button("kb-preview", b1, "Preview changes", None, ButtonKind::Normal) || l.ui.button("kb-reload", b2, "Reload from OMSI 2", Some("restart_alt"), ButtonKind::Primary) {
            l.pages.kb.dialog = Some(Dialog::Reload { backup: true, pads: true });
        }
    }
    r.h
}

fn groups_card(l: &mut Launcher, r: Rect, all: &[Row]) {
    l.ui.panel(r);
    let inner = Launcher::card_title(&mut l.ui, Rect::new(r.x + 16.0, r.y + 14.0, r.w - 32.0, r.h - 28.0), "Groups");
    let row = |l: &mut Launcher, i: usize, name: &str, y: f32, warn: bool| {
        let n = all.iter().filter(|x| x.in_list(i)).count();
        let rr = Rect::new(inner.x - 6.0, y, inner.w + 12.0, 32.0);
        let on = l.pages.kb.list == i;
        let (hover, _, clicked) = l.ui.interact_key(id_of(&format!("kb-group-{i}")), rr);
        if on {
            l.ui.p().rounded(rr, 5.0, accent().alpha(0.14));
            l.ui.p().rounded_border(rr, 5.0, 1.0, accent().alpha(0.6));
        } else if hover {
            l.ui.p().rounded(rr, 5.0, Color::WHITE.alpha(0.05));
        }
        let c = if warn { WARN } else { TEXT };
        let name = omsi_launcher_ui::kit::fit_text(&mut l.ui, name, 13.5, rr.w - 60.0);
        l.ui.text_in(&name, Rect::new(rr.x + 10.0, rr.y, rr.w - 50.0, rr.h), 13.5, Weight::Medium, c, Align::Left);
        l.ui.text_in(&n.to_string(), Rect::new(rr.right() - 44.0, rr.y, 34.0, rr.h), 12.0, Weight::Regular, if warn { WARN } else { TEXT_DIM }, Align::Right);
        if clicked {
            l.pages.kb.list = i;
            l.pages.kb.query.clear();
        }
    };
    for (i, name) in GROUPS.iter().enumerate() {
        row(l, i, name, inner.y + i as f32 * 35.0, false);
    }
    let foot = r.bottom() - 14.0 - FILTERS.len() as f32 * 35.0;
    let foot = foot.max(inner.y + GROUPS.len() as f32 * 35.0 + 8.0);
    for (k, name) in FILTERS.iter().enumerate() {
        row(l, GROUPS.len() + k, name, foot + k as f32 * 35.0, k == 0);
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

fn list_card(l: &mut Launcher, r: Rect, all: &[Row]) {
    l.ui.panel(r);
    let q = l.pages.kb.query.trim().to_lowercase();
    // (a search looks through every action, whatever the group)
    let shown: Vec<&Row> = all
        .iter()
        .filter(|x| if q.is_empty() { x.in_list(l.pages.kb.list) } else { matches(&x.label, &q) || x.action.to_lowercase().contains(&q) || x.keys.iter().any(|k| k.0 != 0 && crate::keys::key_name(k.0, k.1).to_lowercase().contains(&q)) })
        .collect();
    let title = if q.is_empty() { list_name(l.pages.kb.list) } else { "Search" };
    let inner = Launcher::card_title(&mut l.ui, Rect::new(r.x + 16.0, r.y + 14.0, r.w - 32.0, r.h - 28.0), title);
    let note = if shown.len() == 1 { "1 action".to_string() } else { format!("{} actions", shown.len()) };
    l.ui.text_in(&note, Rect::new(inner.x, r.y + 14.0, inner.w, 18.0), 12.0, Weight::Regular, TEXT_DIM, Align::Right);
    let aw = l.ui.width("Add a control", 13.0, Weight::Bold) + 48.0;
    let mut query = std::mem::take(&mut l.pages.kb.query);
    l.ui.text_input("kb-search", Rect::new(inner.x, inner.y, inner.w - aw - 10.0, 34.0), &mut query, "Search every action...", Some("search"));
    l.pages.kb.query = query;
    if l.ui.button("kb-add", Rect::new(inner.right() - aw, inner.y, aw, 34.0), "Add a control", Some("add"), ButtonKind::Primary) {
        l.pages.kb.dialog = Some(Dialog::Add { query: String::new(), chosen: None, key: None, waiting: false });
    }
    // the columns: the action, its key, its second key, back to the original
    let kx = inner.x + (inner.w * 0.42).max(180.0);
    let k2x = kx + (inner.w - (kx - inner.x)) * 0.42;
    let hy = inner.y + 52.0;
    for (t, x) in [("Action", inner.x + 8.0), ("Key", kx), ("Second key", k2x)] {
        l.ui.text_in(t, Rect::new(x, hy - 9.0, 120.0, 18.0), 12.0, Weight::Regular, TEXT_DIM, Align::Left);
    }
    let area = Rect::new(inner.x - 6.0, hy + 14.0, inner.w + 12.0, inner.bottom() - hy - 14.0);
    let chosen = l.pages.kb.chosen.clone();
    let capturing = l.pages.capturing;
    let time = l.ui.time;
    // (what was clicked: the row, a key cell (row, which key), back to the original)
    let mut pick: Option<(usize, String)> = None;
    let mut key_click: Option<(usize, usize)> = None;
    let mut reset: Option<usize> = None;
    let rows: Vec<Row> = shown.into_iter().cloned().collect();
    l.ui.scroll_area(&format!("kb-list-{}-{}", l.pages.kb.list, q.is_empty()), area, &mut |ui, v| {
        let rh = 35.0;
        for (n, row) in rows.iter().enumerate() {
            let rr = Rect::new(v.x + 6.0, v.y + n as f32 * rh, v.w - 16.0, rh - 3.0);
            if rr.bottom() < v.y - 40.0 || rr.y > v.bottom() + 40.0 {
                continue;
            }
            let on = chosen.as_ref().is_some_and(|c| c.0 == row.sec && c.1 == row.action);
            let (hover, _, clicked) = ui.interact_key(id_of(&format!("kb-row-{}-{}", row.sec, row.action)), rr);
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
            ui.text_in(&row.label, Rect::new(rr.x + 8.0, rr.y, room, rr.h), 13.5, Weight::Regular, TEXT, Align::Left);
            let mut px = rr.x + 8.0 + lw.min(room) + 8.0;
            for (show, text, c) in [(row.changed, "changed", accent()), (row.shared, "shared key", WARN)] {
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
                let text = if waiting { "press a key…".to_string() } else { key.map(|c| crate::keys::key_name(c.0, c.1)).unwrap_or_else(|| "–".into()) };
                let w = if key.is_some() || waiting { ui.width(&text, 12.0, Weight::Medium) + 16.0 } else { 26.0 };
                let cr = Rect::new(x, cy - 11.0, w.max(26.0), 22.0);
                let (h, _, c) = ui.interact(id_of(&format!("kb-cap-{}-{}-{k}", row.sec, row.action)), cr);
                if key.is_some() || waiting {
                    let edge = if waiting { accent().alpha(0.4 + 0.4 * (time * 6.0).sin().abs()) } else if row.shared { WARN } else if h { TEXT_DIM } else { EDGE };
                    ui.p().rounded(cr, 4.0, Color::WHITE.alpha(0.05));
                    ui.p().rounded_border(cr, 4.0, 1.0, edge);
                    ui.text_in(&text, cr, 12.0, Weight::Medium, TEXT, Align::Center);
                } else {
                    ui.text_in(&text, cr, 12.0, Weight::Regular, if h { TEXT_SOFT } else { TEXT_FAINT }, Align::Center);
                }
                if c {
                    key_click = Some((n, k));
                }
            }
            if row.changed {
                let xr = Rect::new(rr.right() - 30.0, cy - 12.0, 24.0, 24.0);
                let (hx, _, cx) = ui.interact(id_of(&format!("kb-reset-{}-{}", row.sec, row.action)), xr);
                ui.icon("restart_alt", xr.center(), 15.0, if hx { accent() } else { TEXT_DIM });
                if cx {
                    reset = Some(n);
                }
            }
        }
        rows.len() as f32 * rh
    });
    if let Some(p) = pick {
        l.pages.kb.chosen = Some(p);
    }
    if let Some((n, k)) = key_click {
        let row = rows[n].clone();
        l.pages.kb.chosen = Some((row.sec, row.action.clone()));
        wait_for_key(l, &row, k);
    }
    if let Some(n) = reset {
        back_to_original(l, &rows[n].clone());
    }
}

/// The `k`-th key of `row` waits to be pressed (a second key's entry is added for it when the
/// action has one key only).
fn wait_for_key(l: &mut Launcher, row: &Row, k: usize) {
    if let Some(&e) = row.entries.get(k) {
        l.pages.capturing = Some((row.sec, e));
        return;
    }
    let Some(&last) = row.entries.last() else { return };
    if let Some(a) = l.state.keybindings.get_mut(SECTIONS[row.sec]).and_then(|a| a.as_array_mut()) {
        if let Some(b) = a.get(last).cloned() {
            // (the held bit is the action's: it goes with it)
            let hold = b.get("modifier").and_then(|x| x.as_i64()).unwrap_or(0) & omsi_content::input::KEY_HOLD as i64;
            a.insert(last + 1, json!({ "action": row.action, "scan_code": 0, "modifier": hold }));
            l.pages.capturing = Some((row.sec, last + 1));
        }
    }
}

/// `row`'s keys as the original OMSI 2 has them.
fn back_to_original(l: &mut Launcher, row: &Row) {
    let Some(orig) = l.pages.kb.original.clone().flatten() else { return };
    let key = SECTIONS[row.sec];
    let mut only = json!({ "vehicles": [], "game": [] });
    only[key] = json!(orig.get(key).and_then(|a| a.as_array()).map(|a| a.iter().filter(|b| b.get("action").and_then(|x| x.as_str()) == Some(row.action.as_str())).cloned().collect::<Vec<_>>()).unwrap_or_default());
    // (the reload of one action: every other one as it is)
    let plan = reload_plan(&l.state.keybindings, &only, &|a| a.to_string());
    l.state.keybindings = plan.merged;
    save_keys(l, row.action.starts_with("vr_"));
}

fn detail_card(l: &mut Launcher, r: Rect, all: &[Row], original: Option<&Value>) {
    l.ui.panel(r);
    let row = l.pages.kb.chosen.as_ref().and_then(|(sec, a)| all.iter().find(|x| x.sec == *sec && x.action == *a)).cloned();
    let Some(row) = row else {
        let inner = Launcher::card_title(&mut l.ui, Rect::new(r.x + 16.0, r.y + 14.0, r.w - 32.0, r.h - 28.0), "Action");
        l.ui.paragraph("Click an action to see its keys, change them, or take the original OMSI 2's back.", Vec2::new(inner.x, inner.y + 4.0), inner.w, 13.0, Weight::Regular, TEXT_DIM);
        return;
    };
    let inner = Launcher::card_title(&mut l.ui, Rect::new(r.x + 16.0, r.y + 14.0, r.w - 32.0, r.h - 28.0), &row.label);
    let group = GROUPS[row.group];
    l.ui.text_in(group, Rect::new(inner.x, r.y + 14.0, inner.w, 18.0), 12.0, Weight::Regular, TEXT_DIM, Align::Right);
    let mut y = inner.y;
    l.ui.text_in(&format!("In the controls file: {}", row.action), Rect::new(inner.x, y, inner.w, 18.0), 12.0, Weight::Regular, TEXT_DIM, Align::Left);
    y += 30.0;
    for (k, what) in [(0usize, "Key"), (1, "Second key")] {
        l.ui.text_in(what, Rect::new(inner.x, y, inner.w, 16.0), 12.0, Weight::Regular, TEXT_DIM, Align::Left);
        y += 20.0;
        let f = Rect::new(inner.x, y, inner.w, 34.0);
        let waiting = row.entries.get(k).is_some_and(|e| l.pages.capturing == Some((row.sec, *e)));
        let key = row.keys.get(k).filter(|c| c.0 != 0).map(|c| crate::keys::key_name(c.0, c.1));
        let (h, _, c) = l.ui.interact_key(id_of(&format!("kb-field-{k}")), f);
        l.ui.p().rounded(f, 4.0, FIELD);
        l.ui.p().rounded_border(f, 4.0, 1.0, if waiting { accent() } else if h { TEXT_DIM } else { EDGE });
        if waiting {
            l.ui.p().rounded_border(f.inset(-1.0), 5.0, 1.0, accent());
            l.ui.text_in("Press a key now...", Rect::new(f.x + 12.0, f.y, f.w - 24.0, f.h), 13.0, Weight::Regular, TEXT, Align::Left);
            l.ui.text_in("Esc cancels", Rect::new(f.x + 12.0, f.y, f.w - 24.0, f.h), 12.5, Weight::Regular, TEXT_DIM, Align::Right);
        } else {
            l.ui.text_in(key.as_deref().unwrap_or("None"), Rect::new(f.x + 12.0, f.y, f.w - 40.0, f.h), 13.0, Weight::Regular, if key.is_some() { TEXT } else { TEXT_FAINT }, Align::Left);
            l.ui.icon(if key.is_some() { "edit" } else { "add" }, Vec2::new(f.right() - 18.0, f.center().y), 15.0, TEXT_DIM);
        }
        if c {
            wait_for_key(l, &row, k);
        }
        y += 46.0;
    }
    let mine: Vec<Chord> = row.keys.iter().copied().filter(|k| k.0 != 0).collect();
    let line = |l: &mut Launcher, y: &mut f32, what: &str, keys: &[Chord], differs: bool| {
        l.ui.text_in(what, Rect::new(inner.x, *y, inner.w, 16.0), 12.0, Weight::Regular, TEXT_DIM, Align::Left);
        *y += 20.0;
        let mut x = inner.x;
        if keys.is_empty() {
            l.ui.text_in("none", Rect::new(x, *y, 80.0, 22.0), 12.5, Weight::Regular, TEXT_FAINT, Align::Left);
            x += 44.0;
        }
        for k in keys {
            x = cap(l, x, *y + 11.0, &crate::keys::key_name(k.0, k.1), EDGE).right() + 6.0;
        }
        if differs {
            let now = if mine.is_empty() { "none".to_string() } else { names_of(&mine) };
            let t = omsi_launcher_ui::kit::fit_text(&mut l.ui, &format!("you changed it to {now}"), 12.0, inner.right() - x - 6.0);
            l.ui.text_in(&t, Rect::new(x + 4.0, *y, inner.right() - x, 22.0), 12.0, Weight::Regular, TEXT_DIM, Align::Left);
        }
        *y += 34.0;
    };
    let in_omsi = original.filter(|o| has_action(o, SECTIONS[row.sec], &row.action)).map(|o| keys_of(o, SECTIONS[row.sec], &row.action));
    match &in_omsi {
        Some(k) => line(l, &mut y, "In your original OMSI 2", k, row.changed),
        None => {
            l.ui.text_in("In your original OMSI 2", Rect::new(inner.x, y, inner.w, 16.0), 12.0, Weight::Regular, TEXT_DIM, Align::Left);
            l.ui.text_in("Not there: a Depot key, kept as it is.", Rect::new(inner.x, y + 20.0, inner.w, 22.0), 12.5, Weight::Regular, TEXT_FAINT, Align::Left);
            y += 54.0;
        }
    }
    let factory = factory_keys(&row.action);
    if !factory.is_empty() {
        line(l, &mut y, "Factory key (OMSI 2 as it comes)", &factory, false);
    }
    let _ = y;
    // the foot: back to the original, clear; and why a shared key is fine
    let note = "A key used twice is flagged, never blocked: both actions fire together, and you can save it that way.";
    let nh = l.ui.paragraph_height(note, inner.w, 12.0, Weight::Regular);
    let by = inner.bottom() - nh - 46.0;
    let cw = 80.0;
    let back = Rect::new(inner.x, by, inner.w - cw - 10.0, 34.0);
    if row.changed && in_omsi.is_some() {
        if l.ui.button("kb-back", back, "Back to original", Some("restart_alt"), ButtonKind::Normal) {
            back_to_original(l, &row);
        }
    } else {
        l.ui.p().rounded(back, 4.0, Color::WHITE.alpha(0.03));
        l.ui.text_in("Back to original", back, 13.0, Weight::Medium, TEXT_FAINT, Align::Center);
    }
    if l.ui.button("kb-clear", Rect::new(inner.right() - cw, by, cw, 34.0), "Clear", None, ButtonKind::Normal) && !row.unassigned() {
        let key = SECTIONS[row.sec];
        if let Some(a) = l.state.keybindings.get_mut(key).and_then(|a| a.as_array_mut()) {
            for &e in &row.entries {
                if let Some(b) = a.get_mut(e) {
                    let hold = b.get("modifier").and_then(|x| x.as_i64()).unwrap_or(0) & omsi_content::input::KEY_HOLD as i64;
                    b["scan_code"] = json!(0);
                    b["modifier"] = json!(hold);
                }
            }
        }
        save_keys(l, row.action.starts_with("vr_"));
    }
    l.ui.paragraph(note, Vec2::new(inner.x, by + 44.0), inner.w, 12.0, Weight::Regular, TEXT_FAINT);
}

/// The window open over the keyboard page, if any (the page sees no mouse meanwhile).
pub fn dialog(l: &mut Launcher) {
    capture(l);
    let esc = l.ui.input.keys.contains(&Key::Escape);
    match l.pages.kb.dialog.take() {
        Some(Dialog::Reload { backup, pads }) => {
            if !esc {
                reload_dialog(l, backup, pads);
            }
        }
        Some(Dialog::Add { query, chosen, key, waiting }) => {
            if !esc || waiting {
                add_dialog(l, query, chosen, key, waiting);
            }
        }
        None => {}
    }
}

fn reload_dialog(l: &mut Launcher, mut backup: bool, mut pads: bool) {
    let Some(orig) = l.pages.kb.original.clone().flatten() else { return };
    let names = control_names(l);
    let plan = reload_plan(&l.state.keybindings, &orig, &|a| action_text(names, a));
    let rows_h = (plan.changes.len().max(1) as f32 * 36.0 + 34.0).min(300.0);
    let inside = super::start::dialog(l, 880.0, 56.0 + 40.0 + rows_h + 120.0 + 76.0, "Reload controls from OMSI 2", "your original OMSI 2 folder · read only");
    let (buttons, body) = super::start::dialog_foot(l, inside);
    let mut x = body.x;
    for (text, c) in [(format!("{} already identical", plan.identical), OK), (format!("{} will change", plan.changes.len()), accent()), (format!("{} only in the Depot, kept as they are", plan.depot_only), TEXT_SOFT)] {
        x += pill(l, x, body.y + 10.0, &text, c) + 8.0;
    }
    let t = Rect::new(body.x, body.y + 34.0, body.w, rows_h);
    let cols = [t.x + 12.0, t.x + t.w * 0.4, t.x + t.w * 0.72];
    l.ui.p().rect(Rect::new(t.x, t.y, t.w, 1.0), EDGE);
    for (s, x) in [("Action", cols[0]), ("Your key now", cols[1]), ("OMSI 2 key", cols[2])] {
        l.ui.text_in(s, Rect::new(x, t.y, 200.0, 32.0), 12.5, Weight::Regular, TEXT_DIM, Align::Left);
    }
    let changes = plan.changes.clone();
    l.ui.scroll_area("kb-reload-list", Rect::new(t.x, t.y + 34.0, t.w, t.h - 34.0), &mut |ui, v| {
        if changes.is_empty() {
            ui.text_in("Nothing to change: every key is already the original OMSI 2's.", Rect::new(v.x + 12.0, v.y, v.w, 32.0), 13.0, Weight::Regular, TEXT_DIM, Align::Left);
            return 32.0;
        }
        for (k, (label, now, omsi)) in changes.iter().enumerate() {
            let y = v.y + k as f32 * 36.0;
            ui.p().rect(Rect::new(v.x, y, v.w, 1.0), EDGE);
            ui.text_in(label, Rect::new(cols[0], y, cols[1] - cols[0] - 10.0, 36.0), 13.0, Weight::Regular, TEXT, Align::Left);
            ui.text_in(now, Rect::new(cols[1], y, cols[2] - cols[1] - 40.0, 36.0), 12.5, Weight::Medium, TEXT_SOFT, Align::Left);
            ui.icon("o-chevron", Vec2::new(cols[2] - 24.0, y + 18.0), 13.0, TEXT_DIM);
            ui.text_in(omsi, Rect::new(cols[2], y, v.right() - cols[2], 36.0), 12.5, Weight::Medium, accent(), Align::Left);
        }
        changes.len() as f32 * 36.0
    });
    let mut y = t.bottom() + 12.0;
    l.ui.toggle("kb-reload-backup", Rect::new(body.x, y, body.w, 30.0), &mut backup, "Keep a backup of my current layout first");
    y += 34.0;
    l.ui.toggle("kb-reload-pads", Rect::new(body.x, y, body.w, 30.0), &mut pads, "Also read the game controller assignments");
    y += 38.0;
    l.ui.paragraph("After reloading, every key the original OMSI 2 has matches its keyboard (and, with the switch above, controller) files; keys only the Depot has are kept. The original files are only read, never written to.", Vec2::new(body.x, y), body.w, 12.0, Weight::Regular, TEXT_DIM);
    let apply = Rect::new(buttons.right() - 170.0, buttons.y, 170.0, 34.0);
    if l.ui.button("kb-reload-apply", apply, "Reload and apply", None, ButtonKind::Primary) {
        match core::reload_keybindings(&plan.merged, backup) {
            Ok(kept) => {
                l.state.keybindings_error.clear();
                if let Ok(k) = core::get_keybindings() {
                    l.state.keybindings = k;
                }
                let mut said = format!("Keys reloaded from OMSI 2 ({} changed).", plan.changes.len());
                if let Some(p) = kept {
                    said.push_str(&format!(" Your layout before is kept as {}.", p.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()));
                }
                if pads {
                    match reload_pads(l) {
                        Ok(true) => said.push_str(" Game controllers read too."),
                        Ok(false) => {}
                        Err(e) => said.push_str(&format!(" The game controllers were not read: {e}")),
                    }
                }
                l.state.set_status(said, false);
                l.pages.kb.original = None;
            }
            Err(e) => l.state.set_status(format!("{e:#}"), true),
        }
        return;
    }
    if l.ui.button("kb-reload-cancel", Rect::new(apply.x - 120.0, buttons.y, 110.0, 34.0), "Cancel", None, ButtonKind::Normal) {
        return;
    }
    l.pages.kb.dialog = Some(Dialog::Reload { backup, pads });
}

/// The original OMSI 2's controller assignments (`Inputs/gamectrler.cfg`) as the Depot's own;
/// false when it has none.
fn reload_pads(l: &mut Launcher) -> Result<bool, String> {
    let file = std::path::Path::new(&l.state.config.root).join("Inputs").join("gamectrler.cfg");
    if !file.is_file() {
        return Ok(false);
    }
    let mut devices = omsi_content::input::gamectrler::read(&file).map_err(|e| e.to_string())?;
    // (as reading it in place does: a wheel's 0/0 force feedback scale is the Depot's default)
    for d in &mut devices {
        if d.ff_scale == Some((0.0, 0.0)) {
            d.ff_scale = None;
        }
    }
    crate::controllers::save_cfg(&devices)?;
    l.pages.pads = Default::default();
    Ok(true)
}

fn add_dialog(l: &mut Launcher, mut query: String, mut chosen: Option<String>, mut key: Option<Chord>, mut waiting: bool) {
    let names = control_names(l);
    let inside = super::start::dialog(l, 820.0, 470.0, "Add a control", "choose what it does, then press the key");
    let (buttons, body) = super::start::dialog_foot(l, inside);
    let right_w = 280.0;
    let left = Rect::new(body.x, body.y, body.w - right_w - 14.0, body.h);
    let right = Rect::new(left.right() + 14.0, body.y, right_w, body.h);
    l.ui.text_input("kb-add-search", Rect::new(left.x, left.y, left.w, 34.0), &mut query, "Search actions without a key...", Some("search"));
    let q = query.trim().to_lowercase();
    // the actions the game knows (the language files' and the mods' event tables) that have no
    // key yet
    let bound: std::collections::HashSet<String> = ["vehicles", "game"].iter().flat_map(|s| l.state.keybindings.get(*s).and_then(|a| a.as_array()).cloned().unwrap_or_default()).filter(|b| b.get("scan_code").and_then(|x| x.as_i64()).unwrap_or(0) != 0).filter_map(|b| b.get("action").and_then(|x| x.as_str()).map(|s| s.to_string())).collect();
    let mut list: Vec<(String, String)> = names.events().into_iter().filter(|(a, _)| !bound.contains(a)).collect();
    if !q.is_empty() {
        list.retain(|(a, label)| a.to_lowercase().contains(&q) || matches(label, &q));
    }
    // (a name no list has, as a mod's readme gives one for its bus's own trigger: added as
    // OMSI's own key dialog would, when typed whole)
    let typed = query.trim().to_string();
    let custom = list.is_empty() && typed.len() > 1 && typed.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if custom {
        list.push((typed.clone(), format!("\"{typed}\": a bus's own trigger (from a mod's readme)")));
    }
    let mut picked = None;
    let sel = chosen.clone();
    l.ui.scroll_area("kb-add-list", Rect::new(left.x, left.y + 44.0, left.w, left.h - 44.0), &mut |ui, v| {
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
            ui.text_in(label, Rect::new(rr.x + 10.0, rr.y + 4.0, rr.w - 20.0, 20.0), 13.5, Weight::Medium, TEXT, Align::Left);
            let g = GROUPS[group_of(0, a)];
            ui.text_in(&format!("{g} · no key yet"), Rect::new(rr.x + 10.0, rr.y + 24.0, rr.w - 20.0, 16.0), 12.0, Weight::Regular, TEXT_DIM, Align::Left);
            if clicked {
                picked = Some(a.clone());
            }
        }
        list.len() as f32 * rh
    });
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
            l.ui.paragraph("Choose an action on the left, then press its key.", Vec2::new(right.x, y), right.w, 13.0, Weight::Regular, TEXT_DIM);
        }
        Some(a) => {
            l.ui.text_in(&action_text(names, a), Rect::new(right.x, y, right.w, 18.0), 12.5, Weight::Medium, TEXT_SOFT, Align::Left);
            y += 24.0;
            let f = Rect::new(right.x, y, right.w, 34.0);
            let (h, _, c) = l.ui.interact_key(id_of("kb-add-key"), f);
            l.ui.p().rounded(f, 4.0, FIELD);
            l.ui.p().rounded_border(f, 4.0, 1.0, if waiting { accent() } else if h { TEXT_DIM } else { EDGE });
            let text = if waiting { "Press a key now...".to_string() } else { key.map(|k| crate::keys::key_name(k.0, k.1)).unwrap_or_else(|| "Click, then press a key".into()) };
            l.ui.text_in(&text, Rect::new(f.x + 12.0, f.y, f.w - 24.0, f.h), 13.0, Weight::Regular, if waiting || key.is_some() { TEXT } else { TEXT_FAINT }, Align::Left);
            if c {
                waiting = true;
            }
            y += 46.0;
            if let Some(k) = key {
                let rows = rows(&l.state.keybindings, None, &|a| action_text(names, a));
                let others: Vec<String> = rows.iter().filter(|r| r.keys.contains(&k)).map(|r| r.label.clone()).collect();
                if !others.is_empty() {
                    let t = format!("{} is also used by {}. Both will fire together. You can still add it.", crate::keys::key_name(k.0, k.1), others.join(", "));
                    y += l.ui.paragraph(&t, Vec2::new(right.x, y), right.w, 12.0, Weight::Regular, WARN) + 10.0;
                }
            }
            l.ui.paragraph("Only actions the game knows are listed, so you cannot create one that does nothing; a bus's own trigger is added when its whole name is typed.", Vec2::new(right.x, y), right.w, 12.0, Weight::Regular, TEXT_FAINT);
        }
    }
    let shared = key.is_some_and(|k| rows(&l.state.keybindings, None, &|a| a.to_string()).iter().any(|r| r.keys.contains(&k)));
    let add = Rect::new(buttons.right() - 140.0, buttons.y, 140.0, 34.0);
    let ready = chosen.is_some() && key.is_some();
    if ready && l.ui.button("kb-add-apply", add, if shared { "Add anyway" } else { "Add" }, None, ButtonKind::Primary) {
        if let (Some(a), Some(k)) = (chosen.clone(), key) {
            // (a game action goes with the game's keys, any other with the bus's)
            let sec = if l.state.keybindings.get("game").and_then(|x| x.as_array()).is_some_and(|x| x.iter().any(|b| b.get("action").and_then(|v| v.as_str()) == Some(a.as_str()))) { 1 } else { 0 };
            if let Some(list) = l.state.keybindings.get_mut(SECTIONS[sec]).and_then(|x| x.as_array_mut()) {
                // (a listed action without a key gets its key in its own entry)
                match list.iter_mut().find(|b| b.get("action").and_then(|v| v.as_str()) == Some(a.as_str()) && b.get("scan_code").and_then(|v| v.as_i64()).unwrap_or(0) == 0) {
                    Some(b) => {
                        let hold = b.get("modifier").and_then(|x| x.as_i64()).unwrap_or(0) & omsi_content::input::KEY_HOLD as i64;
                        b["scan_code"] = json!(k.0);
                        b["modifier"] = json!(k.1 | hold);
                    }
                    None => list.push(json!({ "action": a, "scan_code": k.0, "modifier": k.1 })),
                }
            }
            save_keys(l, a.starts_with("vr_"));
            l.pages.kb.chosen = Some((sec, a));
            l.pages.kb.list = 0;
        }
        return;
    }
    if !ready {
        l.ui.p().rounded(add, 4.0, Color::WHITE.alpha(0.04));
        l.ui.text_in("Add", add, 13.0, Weight::Bold, TEXT_FAINT, Align::Center);
    }
    if l.ui.button("kb-add-cancel", Rect::new(add.x - 120.0, buttons.y, 110.0, 34.0), "Cancel", None, ButtonKind::Normal) {
        return;
    }
    l.pages.kb.dialog = Some(Dialog::Add { query, chosen, key, waiting });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kb(vehicles: Value, game: Value) -> Value {
        json!({ "vehicles": vehicles, "game": game })
    }

    #[test]
    fn actions_fall_into_their_groups() {
        assert_eq!(GROUPS[group_of(0, "throttle")], "Driving");
        assert_eq!(GROUPS[group_of(0, "parking_brake_toggle")], "Driving");
        assert_eq!(GROUPS[group_of(0, "kw_scheinwerfer_toggle")], "Cab and lights");
        assert_eq!(GROUPS[group_of(0, "bus_doorfront0")], "Doors and tickets");
        assert_eq!(GROUPS[group_of(0, "ticket_give")], "Doors and tickets");
        assert_eq!(GROUPS[group_of(1, "view_set_driver")], "Cameras and views");
        assert_eq!(GROUPS[group_of(1, "sim_pause")], "Game and menus");
        assert_eq!(GROUPS[group_of(1, "vr_recenter")], "VR");
        assert_eq!(GROUPS[group_of(1, "door_1")], "Doors and tickets");
        assert_eq!(GROUPS[group_of(0, "ASS_toggle")], "Advanced");
    }

    /// An action's entries make one row (its key and second key); a key two actions share is
    /// flagged on both; a key that differs from the original is "changed", one only the Depot
    /// has never is.
    #[test]
    fn rows_flag_shared_and_changed_keys() {
        let now = kb(
            json!([{"action":"horn","scan_code":35,"modifier":1},{"action":"horn","scan_code":36,"modifier":0},{"action":"parking_brake_toggle","scan_code":52,"modifier":0}]),
            json!([{"action":"sim_pause","scan_code":36,"modifier":0},{"action":"vr_recenter","scan_code":0,"modifier":0}]),
        );
        let orig = kb(json!([{"action":"horn","scan_code":35,"modifier":1},{"action":"parking_brake_toggle","scan_code":83,"modifier":0}]), json!([{"action":"sim_pause","scan_code":36,"modifier":0}]));
        let r = rows(&now, Some(&orig), &|a| a.to_string());
        let get = |a: &str| r.iter().find(|x| x.action == a).unwrap();
        assert_eq!(get("horn").entries, vec![0, 1]);
        assert!(get("horn").shared && get("sim_pause").shared);
        assert!(!get("parking_brake_toggle").shared);
        assert!(get("parking_brake_toggle").changed);
        assert!(get("horn").changed, "a second key the original has not");
        assert!(!get("sim_pause").changed);
        assert!(!get("vr_recenter").changed && get("vr_recenter").unassigned());
        assert!(get("vr_recenter").in_list(10) && get("horn").in_list(8) && get("parking_brake_toggle").in_list(9));
        // (OMSI 2's own layout shares keys on purpose: the manual and the automatic R)
        let stock = kb(json!([{"action":"kw_s_R","scan_code":19,"modifier":0},{"action":"automatic_R","scan_code":19,"modifier":0}]), json!([]));
        assert!(rows(&stock, None, &|a| a.to_string()).iter().all(|r| !r.shared));
    }

    /// Reloading gives every action the original has its keys (the held bit with them) and
    /// keeps the Depot's own; the counts say what happens.
    #[test]
    fn a_reload_takes_the_originals_keys_and_keeps_the_depots_own() {
        let now = kb(json!([{"action":"throttle","scan_code":72,"modifier":1},{"action":"parking_brake_toggle","scan_code":52,"modifier":0}]), json!([{"action":"vr_recenter","scan_code":19,"modifier":0}]));
        let orig = kb(json!([{"action":"throttle","scan_code":72,"modifier":1},{"action":"parking_brake_toggle","scan_code":83,"modifier":0},{"action":"parking_brake_toggle","scan_code":25,"modifier":4}]), json!([]));
        let plan = reload_plan(&now, &orig, &|a| a.to_string());
        assert_eq!((plan.identical, plan.changes.len(), plan.depot_only), (1, 1, 1));
        let v = plan.merged["vehicles"].as_array().unwrap();
        assert_eq!(v.len(), 3);
        assert_eq!(v[1]["scan_code"], 83);
        assert_eq!(v[2]["modifier"], 4);
        assert_eq!(plan.merged["game"][0]["action"], "vr_recenter");
        assert_eq!(plan.changes[0].0, "parking_brake_toggle");
    }

    #[test]
    fn the_factory_keys_are_omsi_2s_own() {
        assert_eq!(factory_keys("parking_brake_toggle"), vec![(52, 0)]);
        assert!(factory_keys("no_such_action").is_empty());
        assert_eq!(day(0), "01/01/1970");
        assert_eq!(day(1_717_200_000), "01/06/2024");
    }
}
