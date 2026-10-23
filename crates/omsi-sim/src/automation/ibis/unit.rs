//! What a bus's IBIS is made of, learned from its compiled scripts.

use super::program::*;
use hashbrown::HashSet;
use omsi_script::{Op, Program, VarId};

/// What the scripts tell about the unit.
#[derive(Debug, Clone, Default)]
pub(super) struct Unit {
    pub(super) mode: Option<VarId>,
    pub(super) line: Option<VarId>,
    pub(super) route: Option<VarId>,
    pub(super) terminus: Option<VarId>,
    pub(super) busstop: Option<VarId>,
    /// Digit keys: the prefix and the ten triggers.
    pub(super) keypads: Vec<(String, Vec<String>)>,
    pub(super) enters: Vec<String>,
    pub(super) mode_keys: Vec<String>,
    /// Pure toggles the unit's own script tests against 1 before it shows its keypad
    /// (the ALMEX's driver card).
    pub(super) toggles: Vec<(String, VarId)>,
    /// Numbers the entry key compares the typed number with (driver number, PIN).
    pub(super) logins: Vec<u32>,
    /// Keys that advance the stop, silent ones first.
    pub(super) next_stop: Vec<String>,
    /// The variable the number keys look at before they type (the ALMEX's screen), the
    /// keys that set it, and the value the driver drives with.
    pub(super) session: Option<(VarId, Vec<String>, Option<f32>)>,
    /// For a unit without such a gate: the screen variable the mode keys switch to their
    /// entry screen (the FloFix Atron's `atron_display_mode` 3), and the keys that switch it
    /// elsewhere without typing (its "Löschen" goes back to the main menu with the ticket
    /// keys).
    pub(super) home: Option<(VarId, Vec<String>)>,
    /// Float and string variables the number keys type into (the entry key reads them).
    pub(super) input: Vec<VarId>,
    pub(super) input_str: Vec<u32>,
    /// State variables an entry key press may move.
    pub(super) watch: Vec<VarId>,
    pub(super) watch_str: Vec<u32>,
    /// Learnt by trial, not from the scripts (see `differs`): mode keys may take two presses.
    pub(super) dynamic: bool,
}

impl Unit {
    pub(super) fn learn(p: &Program, operable: &dyn Fn(&str) -> bool) -> Unit {
        let mut u = Unit {
            mode: p.var("IBIS_mode"),
            line: p.var("IBIS_LinieKurs").or_else(|| p.var("IBIS_Linie")),
            route: p
                .var("IBIS_RouteIndex")
                .or_else(|| p.var("ibox_routenindex")),
            terminus: p.var("IBIS_TerminusIndex"),
            // Aachen's ibox keeps the stop on `ibox_busstop` (no stock `IBIS_busstop`).
            busstop: p.var("IBIS_busstop").or_else(|| p.var("ibox_busstop")),
            ..Default::default()
        };
        let names: Vec<String> = {
            let mut n: Vec<String> = p.triggers.keys().cloned().collect();
            n.sort();
            n
        };
        let press_keys: Vec<&String> = names
            .iter()
            .filter(|n| !n.ends_with("_off") && !n.ends_with("_drag") && !n.starts_with("ai_"))
            .collect();
        Self::number_keys(p, operable, &press_keys, &mut u);
        let digit_keys: HashSet<String> =
            u.keypads.iter().flat_map(|k| k.1.iter().cloned()).collect();
        let (typed, digit_reads) = Self::typed_inputs(p, u.mode, &digit_keys);
        let driver = |n: &str| operable(n) || !press_keys.iter().any(|k| operable(k));
        Self::entry_keys(
            p,
            &mut u,
            &press_keys,
            &digit_keys,
            &typed,
            &digit_reads,
            &driver,
        );
        Self::mode_keys(p, &mut u, &press_keys, &digit_keys, &driver);
        let states = Self::state_variables(p);
        Self::watch_keys(p, &mut u, &states);
        Self::toggle_keys(p, &mut u, &press_keys, &driver);
        Self::busstop_keys(p, &mut u, &press_keys, &digit_keys, &driver);
        Self::session_keys(p, &mut u, &press_keys, &digit_keys, &driver);
        Self::home_keys(p, &mut u, &press_keys, &digit_keys, &driver, &states);
        u
    }

    pub(super) fn number_keys(
        p: &Program,
        operable: &dyn Fn(&str) -> bool,
        press_keys: &[&String],
        u: &mut Unit,
    ) {
        // number keys: ten triggers <prefix>0 … <prefix>9; a driver's first
        for driver_only in [true, false] {
            for n in press_keys {
                let Some(prefix) = n.strip_suffix('0') else {
                    continue;
                };
                if prefix.is_empty() || prefix.ends_with(|c: char| c.is_ascii_digit()) {
                    continue;
                }
                let keys: Vec<String> = (0..10).map(|d| format!("{prefix}{d}")).collect();
                if keys
                    .iter()
                    .all(|k| p.trigger(k).is_some() && (!driver_only || operable(k)))
                {
                    u.keypads.push((prefix.to_string(), keys));
                }
            }
            if !u.keypads.is_empty() {
                break;
            }
        }
    }

    pub(super) fn typed_inputs(
        p: &Program,
        mode: Option<VarId>,
        digit_keys: &HashSet<String>,
    ) -> (Sets, Sets) {
        // what the number keys type into: written by them (not by letting go) and read by
        // an entry key - or by the number keys themselves, like the count of digits typed
        // (a leading 0 changes nothing else)
        let mut typed = (HashSet::new(), HashSet::new());
        let mut digit_reads = (HashSet::new(), HashSet::new());
        for k in digit_keys {
            let b = p.trigger(k).unwrap();
            let (f, s) = stores(p, b);
            let (of, os) = p
                .trigger(&format!("{k}_off"))
                .map(|o| stores(p, o))
                .unwrap_or_default();
            typed
                .0
                .extend(f.difference(&of).copied().filter(|v| Some(*v) != mode));
            typed.1.extend(s.difference(&os).copied());
            let (lf, ls) = loads(p, b);
            digit_reads.0.extend(lf);
            digit_reads.1.extend(ls);
        }
        (typed, digit_reads)
    }

    pub(super) fn state_variables(p: &Program) -> HashSet<VarId> {
        // variables that hold a state (compared with numbers somewhere)
        let mut states: HashSet<VarId> = HashSet::new();
        for b in &p.blocks {
            for w in b.ops.windows(3) {
                if let (Op::Load(v), Op::Push(_), Op::Eq) = (&w[0], &w[1], &w[2]) {
                    states.insert(*v);
                }
            }
        }
        states
    }

    pub(super) fn entry_keys(
        p: &Program,
        u: &mut Unit,
        press_keys: &[&String],
        digit_keys: &HashSet<String>,
        typed: &Sets,
        digit_reads: &Sets,
        driver: &dyn Fn(&str) -> bool,
    ) {
        // entry keys: read what was typed and move the mode
        for n in press_keys {
            if digit_keys.contains(n.as_str()) || refused(n) || !driver(n) {
                continue;
            }
            let b = p.trigger(n).unwrap();
            let (lf, ls) = loads(p, b);
            let reads_input =
                typed.0.iter().any(|v| lf.contains(v)) || typed.1.iter().any(|v| ls.contains(v));
            let moves_mode = u.mode.map(|m| stores(p, b).0.contains(&m)).unwrap_or(false);
            if reads_input && moves_mode {
                u.enters.push(n.to_string());
            }
        }
        let rank = |n: &str, words: &[&str]| {
            words
                .iter()
                .position(|w| n.contains(w))
                .unwrap_or(words.len())
        };
        // a unit whose keys only store a key code for its frame macro: the entry key by its
        // name, among the keys of the keypad's own script
        if u.enters.is_empty() && !u.keypads.is_empty() {
            let pad_file = p
                .trigger(&u.keypads[0].1[1])
                .map(|b| p.blocks[b as usize].file.clone());
            for n in press_keys {
                let low = n.to_ascii_lowercase();
                let same_file = p.trigger(n).map(|b| p.blocks[b as usize].file.clone()) == pad_file;
                if digit_keys.contains(n.as_str()) || refused(&low) || !driver(n) || !same_file {
                    continue;
                }
                if ["eingabe", "enter", "ok", "quit", "bestaet", "confirm", "_e"]
                    .iter()
                    .any(|w| low.contains(w))
                {
                    u.enters.push(n.to_string());
                }
            }
        }
        u.enters
            .sort_by_key(|n| rank(n, &["eingabe", "enter", "ok", "quit"]));
        for e in &u.enters {
            let b = p.trigger(e).unwrap();
            let (lf, ls) = loads(p, b);
            u.input.extend(
                typed
                    .0
                    .iter()
                    .filter(|v| lf.contains(*v) || digit_reads.0.contains(*v)),
            );
            u.input_str.extend(
                typed
                    .1
                    .iter()
                    .filter(|v| ls.contains(*v) || digit_reads.1.contains(*v)),
            );
            // the numbers it compares with: `… (C.L.driver_pin) =`
            each_op(p, b, &mut |ops, i| {
                if let (Op::Const(c), Some(Op::Eq)) = (&ops[i], ops.get(i + 1)) {
                    if *c >= 1.0 && *c < 1e8 && c.fract() == 0.0 && !u.logins.contains(&(*c as u32))
                    {
                        u.logins.push(*c as u32);
                    }
                }
            });
        }
        u.input.sort_unstable();
        u.input.dedup();
        u.input_str.sort_unstable();
        u.input_str.dedup();
    }

    pub(super) fn mode_keys(
        p: &Program,
        u: &mut Unit,
        press_keys: &[&String],
        digit_keys: &HashSet<String>,
        driver: &dyn Fn(&str) -> bool,
    ) {
        // mode keys: store a mode other than 0
        if let Some(m) = u.mode {
            for n in press_keys {
                if digit_keys.contains(n.as_str())
                    || u.enters.contains(n)
                    || refused(n)
                    || !driver(n)
                {
                    continue;
                }
                if constants_stored(p, p.trigger(n).unwrap(), m)
                    .iter()
                    .any(|c| *c != 0.0)
                {
                    u.mode_keys.push(n.to_string());
                }
            }
            u.mode_keys.sort_by_key(|n| {
                rank(
                    n,
                    &[
                        "linie", "line", "lsk", "kurs", "route", "ziel", "dest", "terminus",
                    ],
                )
            });
            // no key stores a mode: the frame macro switches it on a key code, so any key of
            // the keypad may open an entry screen (the Procity's 7 = line, 0 then 3 = code)
            if u.mode_keys.is_empty() && u.input.is_empty() && u.input_str.is_empty() {
                let pad_file = u
                    .keypads
                    .first()
                    .and_then(|k| p.trigger(&k.1[1]))
                    .map(|b| p.blocks[b as usize].file.clone());
                for n in press_keys {
                    let same_file =
                        p.trigger(n).map(|b| p.blocks[b as usize].file.clone()) == pad_file;
                    if same_file
                        && driver(n)
                        && !u.enters.contains(n)
                        && !refused(&n.to_ascii_lowercase())
                    {
                        u.mode_keys.push(n.to_string());
                    }
                }
                u.dynamic = true;
            }
        }
    }

    pub(super) fn watch_keys(p: &Program, u: &mut Unit, states: &HashSet<VarId>) {
        for e in &u.enters {
            let b = p.trigger(e).unwrap();
            let (f, s) = stores(p, b);
            let (of, os) = p
                .trigger(&format!("{e}_off"))
                .map(|o| stores(p, o))
                .unwrap_or_default();
            let goal = [u.mode, u.line, u.route, u.terminus, u.busstop];
            u.watch.extend(f.iter().filter(|v| {
                !of.contains(*v)
                    && (states.contains(*v) || u.input.contains(*v) || goal.contains(&Some(**v)))
            }));
            u.watch_str.extend(
                s.iter()
                    .filter(|v| !os.contains(*v) && u.input_str.contains(*v)),
            );
        }
        // (a key-code unit's entry key stores only the key code, which its frame resets:
        // what the entry moves is found by comparing everything)
        if u.dynamic {
            u.watch.clear();
            u.watch_str.clear();
        }
        u.watch.sort_unstable();
        u.watch.dedup();
        u.watch_str.sort_unstable();
        u.watch_str.dedup();
    }

    pub(super) fn toggle_keys(
        p: &Program,
        u: &mut Unit,
        press_keys: &[&String],
        driver: &dyn Fn(&str) -> bool,
    ) {
        // a card or switch of the unit: a pure toggle its script compares with 1
        let unit_files: HashSet<&std::path::Path> = u
            .keypads
            .iter()
            .flat_map(|k| k.1.iter())
            .filter_map(|k| p.trigger(k))
            .map(|b| p.blocks[b as usize].file.as_path())
            .collect();
        for n in press_keys {
            if !driver(n) {
                continue;
            }
            let blk = &p.blocks[p.trigger(n).unwrap() as usize];
            let ops: Vec<&Op> = blk
                .ops
                .iter()
                .filter(|o| !matches!(o, Op::SoundTrigger(_)))
                .collect();
            let [Op::Load(a), Op::Not, Op::Store(b)] = ops.as_slice() else {
                continue;
            };
            if a != b {
                continue;
            }
            let tested = p.blocks.iter().filter(|x| unit_files.contains(x.file.as_path())).any(|x| x.ops.windows(3).any(|w| matches!((&w[0], &w[1], &w[2]), (Op::Load(v), Op::Push(c), Op::Eq) if v == a && *c == 1.0)));
            if tested {
                u.toggles.push((n.to_string(), *a));
            }
        }
    }

    pub(super) fn busstop_keys(
        p: &Program,
        u: &mut Unit,
        press_keys: &[&String],
        digit_keys: &HashSet<String>,
        driver: &dyn Fn(&str) -> bool,
    ) {
        // keys that move the IBIS on by a stop: `(L.L.IBIS_busstop) 1 + (S.L.IBIS_busstop)`
        if let Some(bs) = u.busstop {
            for n in press_keys {
                if !driver(n) || digit_keys.contains(n.as_str()) {
                    continue;
                }
                let mut adds = false;
                each_op(p, p.trigger(n).unwrap(), &mut |ops, i| {
                    if i >= 3
                        && ops[i] == Op::Store(bs)
                        && ops[i - 1] == Op::Add
                        && ops[i - 2] == Op::Push(1.0)
                        && ops[i - 3] == Op::Load(bs)
                    {
                        adds = true;
                    }
                });
                if adds {
                    u.next_stop.push(n.to_string());
                }
            }
            u.next_stop
                .sort_by_key(|n| rank(n, &["stumm", "mute", "silent", "quiet"]));
        }
    }

    pub(super) fn session_keys(
        p: &Program,
        u: &mut Unit,
        press_keys: &[&String],
        digit_keys: &HashSet<String>,
        driver: &dyn Fn(&str) -> bool,
    ) {
        // the variable the number keys test before they type (the ALMEX's screen), the keys
        // that set it, and the screen a driver drives with: where the entry and stop keys
        // work but the number keys do not
        if let Some((_, keys)) = u.keypads.first() {
            let blk = &p.blocks[p.trigger(&keys[1]).unwrap() as usize];
            let gate = blk
                .ops
                .windows(3)
                .find_map(|w| {
                    if let (Op::Load(v), Op::Push(_), Op::Eq) = (&w[0], &w[1], &w[2]) {
                        Some(*v)
                    } else {
                        None
                    }
                })
                .filter(|v| Some(*v) != u.mode);
            if let Some(g) = gate {
                let setters: Vec<String> = press_keys
                    .iter()
                    .filter(|n| {
                        driver(n)
                            && !digit_keys.contains(n.as_str())
                            && constants_stored(p, p.trigger(n).unwrap(), g)
                                .iter()
                                .any(|c| *c != 0.0)
                    })
                    .map(|n| n.to_string())
                    .collect();
                let values = |keys: &[String]| -> HashSet<i64> {
                    keys.iter()
                        .filter_map(|k| p.trigger(k))
                        .flat_map(|b| gate_values(p, b, g))
                        .map(|c| c as i64)
                        .collect()
                };
                let digits = values(keys);
                let enter = values(&u.enters);
                let stop = values(&u.next_stop);
                let mut driving: Vec<i64> = enter
                    .iter()
                    .filter(|v| !digits.contains(*v) && (stop.is_empty() || stop.contains(*v)))
                    .copied()
                    .collect();
                driving.sort_unstable();
                u.session = Some((g, setters, driving.first().map(|v| *v as f32)));
            }
        }
    }

    pub(super) fn home_keys(
        p: &Program,
        u: &mut Unit,
        press_keys: &[&String],
        digit_keys: &HashSet<String>,
        driver: &dyn Fn(&str) -> bool,
        states: &HashSet<VarId>,
    ) {
        if let (None, Some(m)) = (&u.session, u.mode) {
            let goal = [u.line, u.route, u.terminus, u.busstop];
            let mut entry_screens: Vec<(VarId, f32)> = Vec::new();
            for k in &u.mode_keys {
                each_op(p, p.trigger(k).unwrap(), &mut |ops, i| {
                    if let Op::Store(v) = ops[i] {
                        if let Some(c) = stored_constant(ops, i) {
                            if c != 0.0
                                && v != m
                                && !u.input.contains(&v)
                                && !goal.contains(&Some(v))
                                && states.contains(&v)
                                && !entry_screens.contains(&(v, c))
                            {
                                entry_screens.push((v, c));
                            }
                        }
                    }
                });
            }
            for (v, c) in entry_screens {
                let keys: Vec<String> = press_keys
                    .iter()
                    .filter(|n| {
                        refused(n)
                            && driver(n)
                            && !digit_keys.contains(n.as_str())
                            && !u.enters.contains(n)
                            && !u.mode_keys.contains(n)
                    })
                    .filter(|n| {
                        constants_stored(p, p.trigger(n).unwrap(), v)
                            .iter()
                            .any(|x| *x != c && *x != 0.0)
                    })
                    .map(|n| n.to_string())
                    .collect();
                if !keys.is_empty() {
                    u.home = Some((v, keys));
                    break;
                }
            }
        }
    }

    pub(super) fn describe(&self, p: &Program) -> String {
        let vn = |v: &VarId| p.var_names.get(*v as usize).cloned().unwrap_or_default();
        format!(
            "keypads {:?}, entry {:?}, modes {:?}, log-in {:?}, toggles {:?}, next stop {:?}, screen {:?}, home {:?}, typed into {:?}",
            self.keypads.iter().map(|k| &k.0).collect::<Vec<_>>(),
            self.enters,
            self.mode_keys,
            self.logins,
            self.toggles.iter().map(|t| &t.0).collect::<Vec<_>>(),
            self.next_stop,
            self.session.as_ref().map(|(g, k, h)| (vn(g), k.clone(), *h)),
            self.home.as_ref().map(|(g, k)| (vn(g), k.clone())),
            self.input.iter().map(vn).collect::<Vec<_>>()
        )
    }
}
