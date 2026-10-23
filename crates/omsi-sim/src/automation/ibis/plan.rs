//! The search for a key sequence that reaches the duty.

use super::sim::*;
use super::typist::*;
use super::unit::*;
use super::*;
use omsi_script::VarId;

pub(super) fn debug() -> bool {
    omsi_cfg::flags::DEBUG_IBIS.is_set()
}

pub(super) fn goal_met(u: &Unit, t: &Target, var: &dyn Fn(VarId) -> Option<f32>) -> bool {
    let is =
        |v: Option<VarId>, x: f32| v.and_then(var).map(|y| (y - x).abs() < 0.5).unwrap_or(true);
    let has_any = u.line.is_some() || u.route.is_some() || u.terminus.is_some();
    has_any
        && is(u.line, t.line as f32)
        && t.route_index.map(|r| is(u.route, r as f32)).unwrap_or(true)
        && is(u.terminus, t.terminus_index as f32)
}

/// How many of the target's variables a state shows.
pub(super) fn progress(u: &Unit, t: &Target, s: &Sim) -> usize {
    let is = |v: Option<VarId>, x: f32| {
        v.is_some() && s.var(v).map(|y| (y - x).abs() < 0.5).unwrap_or(false)
    };
    is(u.line, t.line as f32) as usize
        + t.route_index
            .map(|r| is(u.route, r as f32) as usize)
            .unwrap_or(0)
        + is(u.terminus, t.terminus_index as f32) as usize
}

pub(super) fn met(u: &Unit, t: &Target, s: &Sim) -> bool {
    goal_met(u, t, &|v| s.var(Some(v)))
}

pub(super) fn find_plan(u: &Unit, t: &Target, base: Sim) -> Option<Plan> {
    if u.keypads.is_empty() || u.enters.is_empty() {
        return None;
    }
    let mut steps = Vec::new();
    let mut sim = if met(u, t, &base) {
        steps.push("the IBIS already shows the duty".to_string());
        base
    } else {
        entry_with_prerequisites(u, t, &base, &mut steps)?
    };
    // the stop the bus stands at
    if let Some(bs) = u.busstop {
        let start = sim.var(Some(bs)).unwrap_or(0.0).round() as i64;
        let mut at = start;
        // the key found to move the stop is used for the rest; the others get a moment
        let mut working: Option<&String> = None;
        let mut quiet = 0;
        while at < t.stop as i64 {
            // the silent key for the stops passed over, the announcing one for the last
            let last = at + 1 == t.stop as i64;
            let order: Vec<&String> = if last {
                u.next_stop.iter().rev().collect()
            } else {
                u.next_stop.iter().collect()
            };
            let mut tries: Vec<(&String, f32)> = order
                .iter()
                .map(|k| {
                    (
                        *k,
                        if Some(*k) == working {
                            KEY_WAIT
                        } else {
                            STOP_KEY_WAIT
                        },
                    )
                })
                .collect();
            if working.is_none() {
                if let Some(t) = tries.last_mut() {
                    t.1 = KEY_WAIT;
                }
            }
            let mut moved = None;
            for (k, wait) in tries {
                if sim.press_when(k, wait, &|a, b| a.var(Some(bs)) > b.var(Some(bs))) {
                    moved = Some(k);
                    break;
                }
            }
            let Some(k) = moved else {
                steps.push(format!("could not move on to stop {}", t.stop));
                break;
            };
            if !last {
                working = Some(k);
                // a stop passed over with a key that announces (the unit's silent key does
                // not work): the announcement is left out, as the silent key would have
                if let Some(pr) = sim.presses.iter_mut().rev().find(|p| p.key == *k) {
                    if pr.announces {
                        pr.quiet = true;
                        quiet += 1;
                    }
                }
            }
            at = sim.var(Some(bs)).unwrap_or(0.0).round() as i64;
        }
        if at > start {
            steps.push(format!(
                "stop {at}{}",
                if quiet > 0 {
                    format!(" ({quiet} passed over without their announcement)")
                } else {
                    String::new()
                }
            ));
        }
    }
    // back to the screen the driver drives with
    if let Some((g, keys, Some(home))) = &u.session {
        if sim.var(Some(*g)) != Some(*home) {
            for k in keys {
                let mut tried = sim.clone();
                tried.press(k);
                tried.frames(0.5);
                if tried.var(Some(*g)) == Some(*home) && met(u, t, &tried) {
                    steps.push(format!("screen {home} ({k})"));
                    sim = tried;
                    break;
                }
            }
        }
    }
    // or, on a unit without that gate, back from the entry screen the mode keys opened
    if let (None, Some((g, keys))) = (&u.session, &u.home) {
        for k in keys {
            let mut tried = sim.clone();
            tried.press(k);
            tried.frames(0.5);
            let moved_on = tried.var(Some(*g)) != sim.var(Some(*g));
            let kept = met(u, t, &tried)
                && tried.var(u.mode) == sim.var(u.mode)
                && u.busstop
                    .map(|b| tried.var(Some(b)) == sim.var(Some(b)))
                    .unwrap_or(true);
            if moved_on && kept {
                steps.push(format!(
                    "screen {} ({k})",
                    tried.var(Some(*g)).unwrap_or(0.0)
                ));
                sim = tried;
                break;
            }
        }
    }
    sim.frames(0.5);
    Some(Plan {
        length: sim.t,
        summary: steps.join(", "),
        presses: sim.presses,
    })
}

/// The entry, after the driver's card, log-in and screen where the unit wants them. The
/// log-in is tried at once, then not at all (a driver already logged in, who may have to
/// open the unit's menu first), then after waiting for the unit to boot.
pub(super) fn entry_with_prerequisites(
    u: &Unit,
    t: &Target,
    base: &Sim,
    steps: &mut Vec<String>,
) -> Option<Sim> {
    let logins: &[Option<f32>] = if u.logins.is_empty() {
        &[None]
    } else {
        &[Some(2.0), None, Some(BOOT_WAIT)]
    };
    let p = &base.program;
    let file_of = |key: &str| p.trigger(key).map(|b| p.blocks[b as usize].file.clone());
    for (prefix, digits) in &u.keypads {
        // the entry key of the keypad's own script first (the ALMEX's is the ticket
        // printer's, the IBIS's its own)
        let home = file_of(&digits[1]);
        let mut enters: Vec<&String> = u.enters.iter().collect();
        enters.sort_by_key(|e| file_of(e) != home);
        for enter in enters {
            for login in logins {
                let mut s = base.clone();
                let mut done: Vec<String> = Vec::new();
                for (key, var) in &u.toggles {
                    if s.var(Some(*var)) == Some(0.0) {
                        s.press(key);
                        done.push(format!("{key} on"));
                    }
                }
                if let Some(wait) = login {
                    let mut step = String::new();
                    let logged_in = u.logins.iter().all(|n| {
                        let typed = type_number(u, &mut s, digits, &n.to_string(), *wait);
                        let entered = typed
                            && s.press_when(enter, KEY_WAIT, &|a, b| {
                                differs(a, b, &u.watch, &u.watch_str)
                            });
                        step = format!("{n}: typed {typed} entered {entered}");
                        entered
                    });
                    if debug() {
                        log::info!(
                            "IBIS log-in trial with {enter} after {wait} s: {} ({step})",
                            if logged_in { "done" } else { "failed" }
                        );
                    }
                    if !logged_in {
                        continue;
                    }
                    done.push(format!("log-in {:?}", u.logins));
                }
                // the unit's screen as it is, else each one a key opens
                let mut screens: Vec<Option<&String>> = vec![None];
                // (after a log-in as well: the ALMEX logs in to its menu, and its entry key
                // opens the line entry from there)
                if let Some((g, keys, _)) = &u.session {
                    let g = *g;
                    for k in keys.iter().filter(|k| !u.enters.contains(k)) {
                        if s.clone()
                            .press_when(k, 0.0, &|a, b| a.var(Some(g)) != b.var(Some(g)))
                        {
                            screens.push(Some(k));
                        }
                    }
                }
                for screen in screens {
                    let mut s1 = s.clone();
                    if let Some(k) = screen {
                        s1.press(k);
                        s1.frames(0.5);
                    }
                    if let Some((sim, what)) = entry(u, t, &s1, digits, enter, 0) {
                        done.extend(screen.map(|k| format!("screen {k}")));
                        done.push(format!("{what} with {prefix}0-9 and {enter}"));
                        steps.extend(done);
                        return Some(sim);
                    }
                }
            }
        }
    }
    None
}

/// Type `number` on the keypad, each key once the unit takes it.
pub(super) fn type_number(
    u: &Unit,
    s: &mut Sim,
    digits: &[String],
    number: &str,
    wait: f32,
) -> bool {
    number.chars().all(|c| {
        let d = c.to_digit(10).unwrap_or(0) as usize;
        s.press_when(&digits[d], wait, &|a, b| {
            differs(a, b, &u.input, &u.input_str)
        })
    })
}

/// How many digits the unit takes now.
pub(super) fn room(u: &Unit, s: &Sim, digits: &[String]) -> usize {
    let mut probe = s.clone();
    let mut n = 0;
    while n < 12 {
        let mut next = probe.clone();
        next.press(&digits[1]);
        if !differs(&next, &probe, &u.input, &u.input_str) {
            break;
        }
        probe = next;
        n += 1;
    }
    n
}

/// A mode key (or none), a number and the entry key, confirmed as often as the unit asks;
/// a second entry when the first one set part of the duty. Returns the state and what was
/// typed.
pub(super) fn entry(
    u: &Unit,
    t: &Target,
    s: &Sim,
    digits: &[String],
    enter: &str,
    depth: usize,
) -> Option<(Sim, String)> {
    let before = progress(u, t, s);
    let mut keys: Vec<Vec<&String>> = u.mode_keys.iter().map(|k| vec![k]).collect();
    keys.push(Vec::new());
    if u.dynamic {
        // a list screen first, then the key that opens its code entry
        for a in &u.mode_keys {
            for b in &u.mode_keys {
                keys.push(vec![a, b]);
            }
        }
    }
    for prefix in keys {
        let mut s1 = s.clone();
        let mode = u.mode;
        let mut ok = true;
        for k in &prefix {
            if !s1.press_when(k, if u.dynamic { 0.5 } else { 2.0 }, &|a, b| {
                a.var(mode) != b.var(mode) || (!u.dynamic && differs(a, b, &u.input, &u.input_str))
            }) {
                ok = false;
                break;
            }
        }
        if !ok {
            continue;
        }
        let key: Option<String> = (!prefix.is_empty()).then(|| {
            prefix
                .iter()
                .map(|k| k.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        });
        let key = key.as_ref();
        // a unit learnt by trial: what the number keys type into on this screen - the
        // string that a press of 1 makes end in a 1
        let probed;
        let u: &Unit = if u.dynamic {
            let mut pressed = s1.clone();
            pressed.press(&digits[1]);
            let mut idle = s1.clone();
            idle.frames(HOLD + GAP);
            let typed: Vec<u32> = (0..pressed.state.str_vars.len() as u32)
                .filter(|&i| {
                    let (a, b) = (
                        &pressed.state.str_vars[i as usize],
                        &idle.state.str_vars[i as usize],
                    );
                    a != b && a.trim_end().ends_with('1') && a.trim().len() > b.trim().len()
                })
                .collect();
            if debug() {
                let changed: Vec<String> = (0..pressed.state.str_vars.len())
                    .filter(|&i| pressed.state.str_vars[i] != idle.state.str_vars[i])
                    .map(|i| {
                        format!(
                            "{}={:?} (idle {:?})",
                            pressed
                                .program
                                .str_var_names
                                .get(i)
                                .cloned()
                                .unwrap_or_default(),
                            pressed.state.str_vars[i],
                            idle.state.str_vars[i]
                        )
                    })
                    .collect();
                log::info!(
                    "IBIS probe after {:?} (mode {:?}): strings changed by 1: {changed:?}",
                    prefix,
                    s1.var(u.mode)
                );
            }
            if typed.is_empty() {
                continue;
            }
            let mut v = u.clone();
            v.input_str = typed;
            probed = v;
            &probed
        } else {
            u
        };
        // wait for the unit to take numbers (a "please wait" after the mode key)
        let mut waited = 0.0;
        let mut n = room(u, &s1, digits);
        while n == 0 && waited < 4.0 {
            s1.frames(0.25);
            waited += 0.25;
            n = room(u, &s1, digits);
        }
        if debug() {
            log::info!(
                "IBIS trial prefix {:?}: mode {:?}, room {n}",
                key,
                s1.var(u.mode)
            );
        }
        if n == 0 {
            continue;
        }
        for layout in layouts(t, n) {
            let mut s2 = s1.clone();
            if !type_number(u, &mut s2, digits, &layout, 1.0) {
                if debug() {
                    log::info!("IBIS trial: {layout} did not type");
                }
                continue;
            }
            if !s2.press_when(enter, 1.0, &|a, b| differs(a, b, &u.watch, &u.watch_str)) {
                if debug() {
                    log::info!("IBIS trial: {enter} after {layout} changed nothing");
                }
                continue;
            }
            confirm(u, t, &mut s2, enter);
            // a unit that shows the entry "coming" and takes it with a key of its own (the
            // Scania's ALMEX: "Quitieren" on its RBL panel)
            let mut quit_with = String::new();
            if !met(u, t, &s2) {
                for other in u.enters.iter().filter(|e| e.as_str() != enter) {
                    let mut s3 = s2.clone();
                    if s3.press_when(other, 2.0, &|a, b| differs(a, b, &u.watch, &u.watch_str)) {
                        confirm(u, t, &mut s3, other);
                        if met(u, t, &s3) || progress(u, t, &s3) > progress(u, t, &s2) {
                            s2 = s3;
                            quit_with = format!(" + {other}");
                            break;
                        }
                    }
                }
            }
            let what = format!(
                "{}{layout}{quit_with}",
                key.map(|k| format!("{k} ")).unwrap_or_default()
            );
            if debug() {
                log::info!("IBIS trial (depth {depth}): {what} + {enter} -> mode {:?} line {:?} route {:?} terminus {:?} at {:.1} s (want {} {:?} {})", s2.var(u.mode), s2.var(u.line), s2.var(u.route), s2.var(u.terminus), s2.t, t.line, t.route_index, t.terminus_index);
            }
            if met(u, t, &s2) {
                return Some((s2, what));
            }
            if depth == 0 && progress(u, t, &s2) > before {
                if let Some((s3, more)) = entry(u, t, &s2, digits, enter, 1) {
                    return Some((s3, format!("{what}, {more}")));
                }
            }
        }
    }
    None
}

/// Press the entry key while the unit asks for it: until the duty shows (and then as long
/// as a press still moves something without undoing it), waiting where the unit moves on
/// by itself.
pub(super) fn confirm(u: &Unit, t: &Target, s: &mut Sim, enter: &str) {
    for _ in 0..MAX_CONFIRM {
        let shown = met(u, t, s);
        let mut pressed = s.clone();
        if !pressed.press_when(enter, if shown { 0.0 } else { KEY_WAIT }, &|a, b| {
            differs(a, b, &u.watch, &u.watch_str)
        }) {
            return;
        }
        if shown && !met(u, t, &pressed) {
            return;
        }
        *s = pressed;
    }
}
