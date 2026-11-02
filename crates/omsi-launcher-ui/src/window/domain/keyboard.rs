//! What the Controls page's keyboard tab keeps and works out: the actions in their groups, the
//! rows with their keys against the original OMSI 2's and its factory table, a reload's plan,
//! the keys' names in the game's language, and the windows over the tab.

use super::super::interface::output::{core, Host};
use omsi_content::input::gamectrler::DeviceCfg;
use omsi_ui::Rect;
use serde_json::{json, Value};

/// The groups of actions, `All` first.
pub const GROUPS: [&str; 8] = [
    "All",
    "Driving",
    "Cab and lights",
    "Doors and tickets",
    "Cameras and views",
    "Game and menus",
    "VR",
    "Advanced",
];
/// The lists at the groups' foot, after the groups.
pub const FILTERS: [&str; 3] = ["Conflicts", "Changed by me", "Not assigned"];

/// The sections of the keys, as `keyboard.cfg` has them (the page's `capturing` counts them so).
pub(crate) const SECTIONS: [&str; 2] = ["vehicles", "game"];

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
    if has(&[
        "door",
        "ticket",
        "kneel",
        "haltewunsch",
        "stop_request",
        "ramp",
        "cash",
        "change",
        "fare",
        "printer",
    ]) {
        return 3;
    }
    if has(&[
        "throttle",
        "brake",
        "clutch",
        "steering",
        "blinker",
        "horn",
        "gear",
        "automatic_",
        "retarder",
        "kickdown",
        "enginestart",
        "engine",
    ]) || (a.starts_with("kw_s_") && a.ends_with("_fest"))
    {
        return 1;
    }
    if sec == 1 {
        return 5;
    }
    if has(&[
        "licht",
        "light",
        "lamp",
        "wiper",
        "scheinwerfer",
        "heiz",
        "heat",
        "fan",
        "cp_",
        "kw_",
    ]) {
        return 2;
    }
    7
}

/// A key as the file keeps it: its scan code and its Shift / Ctrl / Alt (the "held" bit, the
/// action's, left out).
pub type Chord = (i64, i64);

pub(crate) fn chord_of(b: &Value) -> Chord {
    let scan = b.get("scan_code").and_then(|x| x.as_i64()).unwrap_or(0);
    let m = b.get("modifier").and_then(|x| x.as_i64()).unwrap_or(0)
        & !(omsi_content::input::KEY_HOLD as i64);
    (scan, m)
}

/// The keys of `action` in the section `key` of `kb`, unbound ones left out.
pub(crate) fn keys_of(kb: &Value, key: &str, action: &str) -> Vec<Chord> {
    kb.get(key)
        .and_then(|a| a.as_array())
        .map(|a| {
            a.iter()
                .filter(|b| b.get("action").and_then(|x| x.as_str()) == Some(action))
                .map(chord_of)
                .filter(|c| c.0 != 0)
                .collect()
        })
        .unwrap_or_default()
}

/// The keys of `action` as the file has them, the held bit included (unbound ones left out).
pub(crate) fn entries_of(kb: &Value, key: &str, action: &str) -> Vec<Chord> {
    kb.get(key)
        .and_then(|a| a.as_array())
        .map(|a| {
            a.iter()
                .filter(|b| b.get("action").and_then(|x| x.as_str()) == Some(action))
                .map(|b| {
                    (
                        b.get("scan_code").and_then(|x| x.as_i64()).unwrap_or(0),
                        b.get("modifier").and_then(|x| x.as_i64()).unwrap_or(0),
                    )
                })
                .filter(|c| c.0 != 0)
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn has_action(kb: &Value, key: &str, action: &str) -> bool {
    kb.get(key).and_then(|a| a.as_array()).is_some_and(|a| {
        a.iter()
            .any(|b| b.get("action").and_then(|x| x.as_str()) == Some(action))
    })
}

pub(crate) fn same_keys(a: &[Chord], b: &[Chord]) -> bool {
    let (mut a, mut b) = (a.to_vec(), b.to_vec());
    a.sort();
    b.sort();
    a == b
}

/// One action of the page: its entries in the file (one per key), what it is, and how its
/// keys stand.
#[derive(Debug, Clone)]
pub struct KeyRow {
    pub sec: usize,
    pub action: String,
    pub label: String,
    /// The indices of its entries in the section, in the file's order.
    pub entries: Vec<usize>,
    /// Their keys, the same order (scan 0: none).
    pub keys: Vec<Chord>,
    pub group: usize,
    /// Its keys differ from the ones in the original OMSI 2's controls file.
    pub changed: bool,
    /// Its keys differ from OMSI 2's as it comes, which has the action ("Back to original").
    pub off_factory: bool,
    /// One of its keys is another action's too.
    pub shared: bool,
}

impl KeyRow {
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
/// The keys of OMSI 2 as it comes are `factory` (its `keyboard_reset.cfg`; the built-in table
/// where there is none).
pub fn rows(
    kb: &Value,
    original: Option<&Value>,
    factory: Option<&Value>,
    label: &dyn Fn(&str) -> String,
) -> Vec<KeyRow> {
    let mut out: Vec<KeyRow> = Vec::new();
    for (sec, key) in SECTIONS.iter().enumerate() {
        let Some(list) = kb.get(*key).and_then(|a| a.as_array()) else {
            continue;
        };
        for (i, b) in list.iter().enumerate() {
            let action = b
                .get("action")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            match out.iter_mut().find(|r| r.sec == sec && r.action == action) {
                Some(r) => {
                    r.entries.push(i);
                    r.keys.push(chord_of(b));
                }
                None => out.push(KeyRow {
                    sec,
                    label: label(&action),
                    group: group_of(sec, &action),
                    entries: vec![i],
                    keys: vec![chord_of(b)],
                    changed: false,
                    off_factory: false,
                    shared: false,
                    action,
                }),
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
    let clash: std::collections::HashSet<Chord> = on
        .iter()
        .filter(|(k, actions)| {
            actions.len() > 1
                && actions
                    .iter()
                    .any(|a| !factory_keys(factory, a).contains(k))
        })
        .map(|(k, _)| *k)
        .collect();
    for r in &mut out {
        r.shared = r.keys.iter().any(|k| clash.contains(k));
        let now: Vec<Chord> = r.keys.iter().copied().filter(|k| k.0 != 0).collect();
        let fk = factory_keys(factory, &r.action);
        r.off_factory = !factory_entries(factory, SECTIONS[r.sec], &r.action).is_empty()
            && !same_keys(&now, &fk);
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
    pub changes: Vec<(String, Vec<Chord>, Vec<Chord>)>,
    pub depot_only: usize,
    pub merged: Value,
}

/// The keys' names as OMSI 2 spells them in the game's language (`Inputs/<lang>.kyb`); the
/// Depot's own for a key the table does not name.
#[derive(Default, Clone)]
pub struct KeyNames(pub Vec<(i32, String)>);

impl KeyNames {
    pub fn name(&self, host: &dyn Host, k: Chord) -> String {
        let Some((_, n)) = self.0.iter().find(|(c, _)| *c as i64 == k.0 && k.0 != 0) else {
            return host.key_name(k.0, k.1);
        };
        let mut out = String::new();
        for (bit, m) in [
            (omsi_content::input::KEY_SHIFT, "Shift+"),
            (omsi_content::input::KEY_CTRL, "Ctrl+"),
            (omsi_content::input::KEY_ALT, "Alt+"),
        ] {
            if k.1 & bit as i64 != 0 {
                out.push_str(m);
            }
        }
        out + n
    }

    pub fn list(&self, host: &dyn Host, keys: &[Chord]) -> String {
        if keys.is_empty() {
            return "none".into();
        }
        keys.iter()
            .map(|k| self.name(host, *k))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Every action the original OMSI 2 has gets its keys (its entries in place of the current
/// ones, where the first of those stood); the actions only the Depot has stay as they are.
pub fn reload_plan(current: &Value, original: &Value, label: &dyn Fn(&str) -> String) -> Reload {
    let mut merged = current.clone();
    let (mut identical, mut changes, mut depot_only) = (0, Vec::new(), 0);
    for key in SECTIONS {
        let now: Vec<Value> = current
            .get(key)
            .and_then(|a| a.as_array())
            .cloned()
            .unwrap_or_default();
        let orig: Vec<Value> = original
            .get(key)
            .and_then(|a| a.as_array())
            .cloned()
            .unwrap_or_default();
        let mut actions: Vec<String> = Vec::new();
        for b in now.iter().chain(orig.iter()) {
            let a = b
                .get("action")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            if !actions.contains(&a) {
                actions.push(a);
            }
        }
        let mut out = now.clone();
        for a in &actions {
            let from: Vec<Value> = orig
                .iter()
                .filter(|b| b.get("action").and_then(|x| x.as_str()) == Some(a.as_str()))
                .cloned()
                .collect();
            if from.is_empty() {
                depot_only += 1;
                continue;
            }
            // (the held bit too: the files are the same after it)
            if same_keys(&entries_of(current, key, a), &entries_of(original, key, a)) {
                identical += 1;
                continue;
            }
            changes.push((
                label(a),
                keys_of(current, key, a),
                keys_of(original, key, a),
            ));
            let at = out
                .iter()
                .position(|b| b.get("action").and_then(|x| x.as_str()) == Some(a.as_str()))
                .unwrap_or(out.len());
            out.retain(|b| b.get("action").and_then(|x| x.as_str()) != Some(a.as_str()));
            let at = at.min(out.len());
            for (k, b) in from.into_iter().enumerate() {
                out.insert(at + k, b);
            }
        }
        merged[key] = json!(out);
    }
    Reload {
        identical,
        changes,
        depot_only,
        merged,
    }
}

/// The entries of `action` in OMSI 2 as it comes, in the section `key`: its `keyboard_reset.cfg`
/// (`factory`), or the built-in table of it where there is none.
pub(crate) fn factory_entries(factory: Option<&Value>, key: &str, action: &str) -> Vec<Value> {
    match factory {
        Some(f) => f
            .get(key)
            .and_then(|a| a.as_array())
            .map(|a| {
                a.iter()
                    .filter(|b| b.get("action").and_then(|x| x.as_str()) == Some(action))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default(),
        None => omsi_content::input::stock_keys::STOCK_KEYS
            .iter()
            .filter(|(a, _, _)| a.eq_ignore_ascii_case(action))
            .map(|(_, scan, m)| json!({ "action": action, "scan_code": scan, "modifier": m }))
            .collect(),
    }
}

/// OMSI 2's own keys for `action`, as it comes (the factory assignment), in either section.
pub fn factory_keys(factory: Option<&Value>, action: &str) -> Vec<Chord> {
    // (the built-in table has no sections: once)
    let keys: &[&str] = if factory.is_some() {
        &SECTIONS
    } else {
        &SECTIONS[..1]
    };
    keys.iter()
        .flat_map(|key| factory_entries(factory, key, action))
        .map(|b| chord_of(&b))
        .filter(|c| c.0 != 0)
        .collect()
}

/// A window over the keyboard page.
pub enum Dialog {
    /// Reload from the original OMSI 2: keep a backup first, read the controllers too.
    Reload { backup: bool, pads: bool },
    /// Add a control: the search, the action chosen, the key pressed for it.
    Add {
        query: String,
        chosen: Option<String>,
        key: Option<Chord>,
        waiting: bool,
    },
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
    /// The original OMSI 2's keys, once read (`None` inside: it has none), and why they could
    /// not be read.
    pub original: Option<Option<Value>>,
    pub original_error: Option<String>,
    /// OMSI 2's keys as it comes (`keyboard_reset.cfg`; `None`: the built-in table).
    pub factory: Option<Value>,
    /// Why `keyboard_reset.cfg` could not be read, when it could not.
    pub factory_error: Option<String>,
    /// The keys' names, and the language they were read for.
    pub names: KeyNames,
    pub(crate) names_lang: String,
    /// The entry added for a second key, while it waits for its key (Esc takes it away).
    pub(crate) added: Option<(usize, usize)>,
    /// When its controls file was saved, and when the keys were last reloaded from it.
    pub saved: Option<u64>,
    pub reloaded: Option<u64>,
    /// The entry waiting for a key (its section and index in `keybindings`), on this tab or
    /// on the controllers' and VR's.
    pub capturing: Option<(usize, usize)>,
    /// The game controllers were read again from OMSI 2 (the game forgets what it showed).
    pub pads_read: bool,
}

impl KeyboardView {
    pub(crate) fn load(&mut self, lang: &str) {
        if self.original.is_none() {
            // (a file that cannot be read is said so, not taken for a missing one)
            let (original, error) = match core::original_keybindings() {
                Ok(v) => (v, None),
                Err(e) => (None, Some(format!("{e:#}"))),
            };
            self.original = Some(original);
            self.original_error = error;
            // (a keyboard_reset.cfg that cannot be read: the built-in table, and said so)
            (self.factory, self.factory_error) = match core::factory_keybindings() {
                Ok(f) => (f, None),
                Err(e) => (None, Some(format!("{e:#}"))),
            };
            self.saved = core::original_keys_saved();
            self.reloaded = core::keys_reloaded_at();
        }
        if self.names_lang != lang {
            self.names = KeyNames(core::key_names(lang));
            self.names_lang = lang.to_string();
        }
    }
}

/// A date of a Unix time, day first.
pub fn day(t: u64) -> String {
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

/// Whether a row at `rr` is in or close to the list's view `area` (both in window coordinates:
/// the rows are already moved by the scrolling, the view is not).
pub fn near_view(rr: Rect, area: Rect) -> bool {
    rr.bottom() >= area.y - 40.0 && rr.y <= area.bottom() + 40.0
}

/// The devices `theirs` in `own`: each in place of the one of its name (whatever the case), or
/// added at the end.
pub fn merge_devices(own: &mut Vec<DeviceCfg>, theirs: Vec<DeviceCfg>) {
    for d in theirs {
        match own
            .iter_mut()
            .find(|o| o.name.eq_ignore_ascii_case(&d.name))
        {
            Some(o) => *o = d,
            None => own.push(d),
        }
    }
}
