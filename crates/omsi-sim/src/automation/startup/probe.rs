//! What the scripts say about the bus: which switches are on, whether the engine runs.

use crate::VehicleInstance;

/// Names tried first, as the stock scripts and `Inputs/keyboard.cfg` spell them.
pub(super) const POWER_NAMES: &[&str] = &[
    "cp_batterietrennschalter_toggle",
    "kw_batterietrennschalter",
    "batterie_toggle",
];

pub(super) const STARTER_NAMES: &[&str] = &[
    "kw_m_enginestart",
    "kw_m_engine_startbutton",
    "engine_start",
];

/// The stock state names, followed by harmless common alternatives found in vehicle mods.
/// They describe state, not a particular cockpit mesh: `cp_batterietrennschalter`, for
/// example, is merely a key position and must not by itself make Shift+U think the electrical
/// system is alive.
pub(super) const POWER_VARIABLES: &[&str] = &[
    "elec_busbar_main_sw",
    "elec_busbar_main",
    // Hong Kong Volvo B9TL variants expose the key position as `elec_real_main`
    // and only raise `elec_busbar_main` after the first starter/ignition press.
    "elec_real_main",
    "elec_battery_on",
    "battery_on",
    "batterie_on",
    "electric_on",
    "electrics_on",
    "main_power",
];

pub(super) const STARTER_VARIABLES: &[&str] = &[
    "engine_starter",
    "engine_starter_on",
    "motor_starter",
    "motor_anlasser",
];

/// A loose match for a deliberately named input, not a variable.  This is only the final
/// fallback after the exact variables and stock trigger names; it lets Shift+U find controls
/// such as `master_battery`, `key_ignition` and `motor_anlasser` in otherwise conventional
/// bus scripts without accidentally operating lamp or door controls.
pub(super) fn semantic_triggers(
    p: &omsi_script::Program,
    bound: &[String],
    words: &[&str],
) -> Vec<String> {
    let matches = |n: &str| {
        let n = n.to_ascii_lowercase();
        !n.ends_with("_off")
            && !n.ends_with("_drag")
            && !n.starts_with("ai_")
            && words.iter().any(|word| n.contains(word))
    };
    let mut out = Vec::new();
    for n in bound.iter().chain(p.trigger_names().iter()) {
        let n = n.to_ascii_lowercase();
        if matches(&n) && p.trigger(&n).is_some() && !out.contains(&n) {
            out.push(n);
        }
    }
    out
}

/// The press triggers (no `_off`/`_drag` halves) that can write one of `vars`, the preferred
/// names first: bound keys, then the stock names, then whatever else the scripts offer.  The
/// semantic names are a last-resort bridge for a mod that does not keep OMSI's stock variable
/// spelling.
pub(super) fn candidates(
    v: &VehicleInstance,
    vars: &[&str],
    stock: &[&str],
    bound: &[String],
    semantic_words: &[&str],
) -> Vec<String> {
    let p = &v.ty.program;
    let storing: Vec<String> =
        vars.iter()
            .flat_map(|var| p.triggers_setting(var))
            .fold(Vec::new(), |mut all, n| {
                if !all.iter().any(|x| x.eq_ignore_ascii_case(&n)) {
                    all.push(n);
                }
                all
            });
    let has_state = vars.iter().any(|var| p.var(var).is_some());
    let usable = |n: &str| !n.ends_with("_off") && !n.ends_with("_drag") && p.trigger(n).is_some();
    let mut out: Vec<String> = Vec::new();
    let add = |n: &str, out: &mut Vec<String>| {
        let n = n.to_ascii_lowercase();
        if usable(&n) && !out.contains(&n) {
            out.push(n);
        }
    };
    for b in bound {
        if storing.iter().any(|s| s.eq_ignore_ascii_case(b)) {
            add(b, &mut out);
        }
    }
    for s in stock {
        // a stock name counts when it writes the variable, or when the scripts have no
        // such variable at all (then nothing can be checked and the name is all there is)
        if !has_state || storing.iter().any(|x| x.eq_ignore_ascii_case(s)) {
            add(s, &mut out);
        }
    }
    for s in &storing {
        add(s, &mut out);
    }
    for s in semantic_triggers(p, bound, semantic_words) {
        add(&s, &mut out);
    }
    out
}

pub(super) fn flag(v: &VehicleInstance, name: &str) -> Option<bool> {
    v.var(name).map(|x| x > 0.5)
}

/// `Some` when this vehicle exposes any of `names`; a system is on if *any* state rail says
/// so.  Several buses leave the main battery bus live after the key-position switch itself
/// falls back to zero, so looking only at the first present variable made an already powered
/// bus look dead.
pub(super) fn any_flag(
    v: &VehicleInstance,
    names: impl IntoIterator<Item = impl AsRef<str>>,
) -> Option<bool> {
    let mut known = false;
    for name in names {
        if let Some(value) = flag(v, name.as_ref()) {
            known = true;
            if value {
                return Some(true);
            }
        }
    }
    known.then_some(false)
}

pub(super) fn custom_state_names<'a>(v: &'a VehicleInstance, kind: &str) -> Vec<&'a str> {
    v.ty.program
        .var_names
        .iter()
        .map(String::as_str)
        .filter(|name| {
            let n = name.to_ascii_lowercase();
            if n.starts_with("cp_") {
                return false;
            }
            match kind {
                "power" => {
                    (n.contains("busbar") && n.contains("main"))
                        || ((n.contains("batter") || n.contains("battery"))
                            && (n.contains("_on") || n.contains("main") || n.contains("trenner")))
                }
                "starter" => n.contains("starter") || n.contains("anlasser"),
                "engine" => {
                    (n.contains("engine") || n.contains("motor"))
                        && (n.ends_with("_on") || n.contains("running"))
                }
                _ => false,
            }
        })
        .collect()
}

pub(super) fn power_state(v: &VehicleInstance) -> Option<bool> {
    any_flag(v, POWER_VARIABLES.iter().copied())
        .or_else(|| any_flag(v, custom_state_names(v, "power")))
}

pub(crate) fn starter_engaged(v: &VehicleInstance) -> bool {
    any_flag(v, STARTER_VARIABLES.iter().copied())
        .or_else(|| any_flag(v, custom_state_names(v, "starter")))
        .unwrap_or(false)
        // Some buses request a delayed start without exposing a starter flag. This is
        // an enum (1=start, 2=stop, 3=off), not an electrical on/off flag.
        || v.var("engine_ignition") == Some(1.0)
}

/// The electrics are on: the main switch, or the busbar where a bus has no switch variable.
pub fn power_on(v: &VehicleInstance) -> bool {
    power_state(v).unwrap_or(false)
}

/// The engine speed the scripts keep, when they keep one.
pub fn engine_rpm(v: &VehicleInstance) -> Option<f32> {
    ["engine_n", "engine_rpm", "motor_n", "motor_rpm"]
        .into_iter()
        .find_map(|n| v.var(n))
}

/// The engine has caught: it runs and turns faster than a starter turns it. The PAZ's
/// carburettor engine sets `engine_on` on its first ignitions at 150 rpm, and let go there it
/// died again.
pub(crate) fn engine_caught(v: &VehicleInstance) -> bool {
    engine_running(v) && engine_rpm(v).map(|n| n > 300.0).unwrap_or(true)
}

/// A false flag is still authoritative unless its only potential setters are unreachable.
/// Some script sets include an unused engine macro alongside a different active drivetrain;
/// its stale flag must not suppress the RPM fallback. Keep explicitly initialized flags
/// with no enabling code, and follow macro calls from the active entry points.
pub(super) fn running_flag_is_authoritative(v: &VehicleInstance, name: &str) -> bool {
    let p = &v.ty.program;
    let Some(id) = p.var(name) else { return false };
    if p.init
        .iter()
        .chain(&p.frame)
        .chain(p.triggers.values())
        .any(|b| p.block_sets(*b, id))
    {
        return true;
    }
    !(0..p.blocks.len()).any(|i| p.block_sets(i as omsi_script::BlockId, id))
}

/// The engine runs. Live script flags are authoritative; RPM is a fallback when flags
/// are absent or only set by unreachable code. A starter can exceed 350 rpm without firing.
pub fn engine_running(v: &VehicleInstance) -> bool {
    let authoritative =
        |name: &&str| flag(v, name) == Some(true) || running_flag_is_authoritative(v, name);
    any_flag(
        v,
        ["engine_on", "engine_running", "motor_on", "motor_running"]
            .into_iter()
            .filter(authoritative),
    )
    .or_else(|| {
        any_flag(
            v,
            custom_state_names(v, "engine")
                .into_iter()
                .filter(authoritative),
        )
    })
    .unwrap_or_else(|| {
        ["engine_n", "engine_rpm", "motor_n", "motor_rpm"]
            .into_iter()
            .filter_map(|n| v.var(n))
            .any(|n| n > 350.0)
    })
}

/// The engine has come to rest: by its speed where the bus has one (the flags of some
/// scripts drop for a frame the moment the fuel is cut, while the engine still turns - the
/// D86's stop button was let go then and the engine ran on; let go at 150 rpm, its script
/// fired it up again: it is 0 only under 100 with the fuel still cut), else by the flags.
pub(super) fn engine_dead(v: &VehicleInstance) -> bool {
    let rpm: Vec<f32> = ["engine_n", "engine_rpm", "motor_n", "motor_rpm"]
        .into_iter()
        .filter_map(|n| v.var(n))
        .collect();
    if rpm.is_empty() {
        !engine_running(v)
    } else {
        rpm.iter().all(|n| *n < 30.0)
    }
}
