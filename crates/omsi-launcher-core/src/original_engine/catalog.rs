//! The settings OmsiLaunch can apply to a session (its `/set:<key>=<value>` catalog), with
//! their ranges, the player's saved values and the launch arguments made from them.
//!
//! `~/.depot/omsi2/settings.json` holds the values by OmsiLaunch's own key; every start
//! sends all of them, so the original game always runs with the settings chosen in Depot and
//! whatever was changed inside OMSI 2 is ignored (OmsiLaunch restores its files afterwards).

use anyhow::{bail, Result};
use serde_json::{Map, Value};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Bool,
    Int(i64, i64),
    Float(f64, f64),
    Choice(&'static [&'static str]),
    /// `enabled,maxPerEmitter,playerVehicleOnly,inReflections`
    Particles,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    General,
    Graphics,
    Simulation,
    Traffic,
    Sound,
    Controls,
    View,
    Advanced,
}

#[derive(Debug, Clone, Copy)]
pub struct Entry {
    /// OmsiLaunch's key.
    pub key: &'static str,
    /// The Depot setting of the same meaning (`settings.cfg`), when there is one.
    pub depot: Option<&'static str>,
    pub group: Group,
    pub kind: Kind,
    pub default: &'static str,
    /// The `options.cfg` token read by "Import from OMSI 2", with its encoding.
    pub token: Option<Token>,
}

#[derive(Debug, Clone, Copy)]
pub enum Token {
    /// A flag present or not; `inverted` when the file names the negative (`no_...`).
    Flag { name: &'static str, inverted: bool },
    /// `[name]` with the value on the next line, scaled by `divisor` (file = public / divisor).
    Value { name: &'static str, divisor: f64 },
    /// One line of the several-line `[AIMaxCountRandom]`.
    Line { name: &'static str, index: usize },
}

use Group::*;
use Kind::*;

const fn e(
    key: &'static str,
    depot: Option<&'static str>,
    group: Group,
    kind: Kind,
    default: &'static str,
    token: Option<Token>,
) -> Entry {
    Entry {
        key,
        depot,
        group,
        kind,
        default,
        token,
    }
}

const fn flag(name: &'static str) -> Option<Token> {
    Some(Token::Flag {
        name,
        inverted: false,
    })
}
const fn noflag(name: &'static str) -> Option<Token> {
    Some(Token::Flag {
        name,
        inverted: true,
    })
}
const fn val(name: &'static str) -> Option<Token> {
    Some(Token::Value { name, divisor: 1.0 })
}

pub static CATALOG: &[Entry] = &[
    e(
        "general.alternateView",
        None,
        General,
        Bool,
        "false",
        flag("altView"),
    ),
    e(
        "general.showOwnDriver",
        Some("driver"),
        General,
        Bool,
        "true",
        flag("see_own_driver"),
    ),
    e(
        "general.showErrorMessages",
        None,
        General,
        Bool,
        "false",
        flag("showerrormessages"),
    ),
    e(
        "general.autoSave",
        None,
        General,
        Bool,
        "true",
        noflag("noAutoSave"),
    ),
    e(
        "general.currentTime",
        None,
        General,
        Bool,
        "false",
        flag("useActTime"),
    ),
    e(
        "general.currentDate",
        None,
        General,
        Bool,
        "false",
        flag("useActDate"),
    ),
    e(
        "general.currentYear",
        None,
        General,
        Bool,
        "false",
        flag("useActYear"),
    ),
    e(
        "graphics.maxFPS",
        Some("max_fps"),
        Graphics,
        Int(10, 200),
        "60",
        val("maxFPS"),
    ),
    e(
        "graphics.tileDistance",
        None,
        Graphics,
        Int(1, 20),
        "6",
        val("performance_tiledistmax"),
    ),
    e(
        "graphics.maxObjectDistanceMeters",
        Some("max_obj_dist"),
        Graphics,
        Int(20, 5000),
        "1000",
        val("performance_maxObjDist"),
    ),
    e(
        "graphics.minObjectScreenPercent",
        None,
        Graphics,
        Float(0.0, 10.0),
        "1",
        Some(Token::Value {
            name: "performance_minObjSize",
            divisor: 0.01,
        }),
    ),
    e(
        "graphics.minReflectionObjectScreenPercent",
        None,
        Graphics,
        Float(0.0, 50.0),
        "5",
        Some(Token::Value {
            name: "performance_minObjSizeRefl",
            divisor: 0.01,
        }),
    ),
    e(
        "graphics.maxObjectComplexity",
        None,
        Graphics,
        Int(0, 3),
        "3",
        val("maxcomplexity"),
    ),
    e(
        "graphics.maxMapComplexity",
        None,
        Graphics,
        Int(0, 2),
        "2",
        val("maxcomplexity_map"),
    ),
    e(
        "graphics.sunGlow",
        None,
        Graphics,
        Bool,
        "true",
        flag("sunglow"),
    ),
    e(
        "graphics.loadAllTiles",
        None,
        Graphics,
        Bool,
        "false",
        flag("loadAllTiles"),
    ),
    e(
        "graphics.stencilBuffer",
        None,
        Graphics,
        Bool,
        "true",
        noflag("no_stencilbuffer"),
    ),
    e(
        "graphics.stencilShadows",
        None,
        Graphics,
        Bool,
        "false",
        None,
    ),
    e(
        "graphics.rainReflections",
        None,
        Graphics,
        Bool,
        "true",
        noflag("no_rain_refl"),
    ),
    e(
        "graphics.humansInRainReflections",
        None,
        Graphics,
        Bool,
        "true",
        noflag("no_humans_on_rain_refl"),
    ),
    e(
        "graphics.realTimeReflections",
        Some("mirror_refresh"),
        Graphics,
        Choice(&["economy", "full"]),
        "economy",
        None,
    ),
    e(
        "graphics.particles",
        None,
        Graphics,
        Particles,
        "true,50,false,true",
        None,
    ),
    e(
        "simulation.collision",
        Some("collision_objects"),
        Simulation,
        Bool,
        "true",
        noflag("no_collision"),
    ),
    e(
        "simulation.collisionTerrain",
        None,
        Simulation,
        Bool,
        "true",
        noflag("no_collision_terrain"),
    ),
    e(
        "simulation.collisionVehicles",
        Some("collision_vehicles"),
        Simulation,
        Bool,
        "true",
        noflag("no_collision_vehToVeh"),
    ),
    e(
        "simulation.collisionPedestrians",
        None,
        Simulation,
        Bool,
        "true",
        noflag("no_collision_pedastrians"),
    ),
    e(
        "simulation.ticketSelling",
        Some("boarding"),
        Simulation,
        Int(0, 2),
        "1",
        val("ticketselling"),
    ),
    e(
        "simulation.maintenance",
        Some("maintenance"),
        Simulation,
        Int(0, 4),
        "0",
        val("wear_lifespan"),
    ),
    e(
        "simulation.disableAutomaticScheduleAnalysisPopup",
        None,
        Simulation,
        Bool,
        "false",
        flag("no_schedAnaPopUp"),
    ),
    e(
        "simulation.ticketInfo",
        None,
        Simulation,
        Bool,
        "true",
        noflag("no_ticketinfo_visible"),
    ),
    e(
        "simulation.automaticClutch",
        None,
        Simulation,
        Bool,
        "true",
        noflag("no_automaticClutch"),
    ),
    e(
        "traffic.randomVehicles",
        Some("ai_max_random"),
        Traffic,
        Int(0, 1000),
        "150",
        Some(Token::Line {
            name: "AIMaxCountRandom",
            index: 0,
        }),
    ),
    e(
        "traffic.humans",
        Some("ai_max_humans"),
        Traffic,
        Int(0, 1000),
        "150",
        Some(Token::Line {
            name: "AIMaxCountRandom",
            index: 1,
        }),
    ),
    e(
        "traffic.factorPercent",
        Some("ai_unsched_factor"),
        Traffic,
        Int(1, 300),
        "100",
        val("AIUnschedFactor"),
    ),
    e(
        "traffic.parkedVehiclesPercent",
        Some("ai_max_parked"),
        Traffic,
        Int(0, 100),
        "50",
        val("AIMaxCountParked"),
    ),
    e(
        "traffic.scheduledVehicles",
        Some("ai_max_scheduled"),
        Traffic,
        Int(0, 1000),
        "150",
        val("AIMaxCountScheduled"),
    ),
    e(
        "traffic.scheduledLinePriority",
        Some("ai_priority_scheduled"),
        Traffic,
        Int(1, 4),
        "4",
        val("AIPriorityScheduled"),
    ),
    e(
        "traffic.passengerFactorPercent",
        Some("pax_density"),
        Traffic,
        Int(0, 200),
        "100",
        val("AIPassFactor"),
    ),
    e(
        "sound.stereo",
        None,
        Sound,
        Int(0, 100),
        "50",
        val("sound_stereo"),
    ),
    e(
        "sound.maxSimultaneousSounds",
        None,
        Sound,
        Int(5, 1000),
        "128",
        val("sound_maxcount"),
    ),
    e(
        "sound.masterVolume",
        Some("volume"),
        Sound,
        Float(0.0, 1.0),
        "1",
        val("sound_vol_master"),
    ),
    e(
        "view.driverSmooth",
        Some("driverview_smooth"),
        View,
        Bool,
        "true",
        flag("driverview_smooth"),
    ),
    e(
        "view.driverMoving",
        Some("head_movement"),
        View,
        Bool,
        "true",
        flag("driverview_moving"),
    ),
    e(
        "controls.autoCenter",
        None,
        Controls,
        Bool,
        "true",
        flag("autoCenter"),
    ),
    e(
        "controls.reducedSteeringSpeed",
        None,
        Controls,
        Bool,
        "false",
        flag("redSteerSpd"),
    ),
    e(
        "advanced.reducedMultithreading",
        None,
        Advanced,
        Bool,
        "false",
        None,
    ),
];

pub fn entry(key: &str) -> Option<&'static Entry> {
    CATALOG.iter().find(|e| e.key.eq_ignore_ascii_case(key))
}

/// The Depot setting names that have an OMSI 2 mode equivalent, with its key.
pub fn depot_map() -> Vec<(&'static str, &'static str)> {
    CATALOG
        .iter()
        .filter_map(|e| e.depot.map(|d| (d, e.key)))
        .collect()
}

fn num(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().replace(',', ".").parse().ok(),
        _ => None,
    }
}

fn boolean(v: &Value) -> Option<bool> {
    match v {
        Value::Bool(b) => Some(*b),
        Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn fmt_float(x: f64) -> String {
    let s = format!("{x:.4}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Checks a value against its entry and returns the text OmsiLaunch takes after `=`.
pub fn serialize(e: &Entry, v: &Value) -> Result<String> {
    let bad = || anyhow::anyhow!("{} has a value outside what OMSI 2 takes", e.key);
    Ok(match e.kind {
        Bool => boolean(v).ok_or_else(bad)?.to_string(),
        Int(lo, hi) => {
            let x = num(v).ok_or_else(bad)?;
            if x.fract() != 0.0 || (x as i64) < lo || (x as i64) > hi {
                return Err(bad());
            }
            (x as i64).to_string()
        }
        Float(lo, hi) => {
            let x = num(v).ok_or_else(bad)?;
            if !x.is_finite() || x < lo || x > hi {
                return Err(bad());
            }
            fmt_float(x)
        }
        Choice(options) => {
            let s = v.as_str().ok_or_else(bad)?.to_ascii_lowercase();
            if !options.contains(&s.as_str()) {
                return Err(bad());
            }
            s
        }
        Particles => {
            let s = v.as_str().ok_or_else(bad)?;
            let f: Vec<&str> = s.split(',').map(str::trim).collect();
            let ok = f.len() == 4
                && f[0].parse::<bool>().is_ok()
                && f[1].parse::<u32>().is_ok()
                && f[2].parse::<bool>().is_ok()
                && f[3].parse::<bool>().is_ok();
            if !ok {
                return Err(bad());
            }
            f.join(",")
        }
    })
}

/// The saved values with the defaults under them: what the next start will send.
pub fn effective(saved: &Map<String, Value>) -> Map<String, Value> {
    let mut out = Map::new();
    for e in CATALOG {
        let v = saved
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(e.key))
            .map(|(_, v)| v.clone())
            .filter(|v| serialize(e, v).is_ok());
        out.insert(
            e.key.to_string(),
            v.unwrap_or_else(|| Value::String(e.default.to_string())),
        );
    }
    out
}

/// `/set:key=value` for every setting, in catalog order. A saved key OmsiLaunch does not
/// know, or a value out of range, is an error before anything starts.
pub fn launch_args(saved: &Map<String, Value>) -> Result<Vec<String>> {
    for (k, v) in saved {
        match entry(k) {
            None => bail!("{k} is not a setting OMSI 2 mode knows"),
            Some(e) => {
                serialize(e, v)?;
            }
        }
    }
    let all = effective(saved);
    // (every start sends every key, so the file OmsiLaunch restores afterwards is always the
    // one it overlaid)
    CATALOG
        .iter()
        .map(|e| Ok(format!("/set:{}={}", e.key, serialize(e, &all[e.key])?)))
        .collect()
}

/// Initial values read once from the player's own `options.cfg` (never written).
pub fn import_from_options(root: &Path) -> Map<String, Value> {
    let mut out = Map::new();
    let Ok(o) = omsi_content::options::Options::load(&root.join("options.cfg")) else {
        return out;
    };
    for e in CATALOG {
        let Some(t) = e.token else { continue };
        let v: Option<Value> = match (t, e.kind) {
            (Token::Flag { name, inverted }, Bool) => {
                Some(Value::Bool(o.flag(&name.to_ascii_lowercase()) != inverted))
            }
            (Token::Value { name, divisor }, kind) => o
                .str(&name.to_ascii_lowercase())
                .and_then(|s| s.trim().replace(',', ".").parse::<f64>().ok())
                .map(|x| {
                    let x = x / divisor;
                    match kind {
                        Float(..) => Value::from(x),
                        _ => Value::from(x.round() as i64),
                    }
                }),
            (Token::Line { name, index }, _) => o
                .values
                .get(&name.to_ascii_lowercase())
                .and_then(|l| l.get(index))
                .and_then(|s| s.trim().parse::<i64>().ok())
                .map(Value::from),
            _ => None,
        };
        // a value outside what OMSI 2 mode takes is left out (the default stays)
        if let Some(v) = v.filter(|v| serialize(e, v).is_ok()) {
            out.insert(e.key.to_string(), v);
        }
    }
    out
}

fn kind_text(k: &Kind) -> String {
    match k {
        Bool => "on / off".into(),
        Int(a, b) => format!("whole number {a} to {b}"),
        Float(a, b) => format!("number {a} to {b}"),
        Choice(c) => format!("one of {}", c.join(", ")),
        Particles => "enabled, per-emitter maximum, player vehicle only, in reflections".into(),
    }
}

/// The table of `docs/ORIGINAL_ENGINE.md`: every setting Depot sends, in catalog order.
/// A test keeps the document equal to this text.
pub fn docs_table() -> String {
    let mut out = String::from(
        "| OmsiLaunch key | Depot setting | Group | Values | Default | Import from `options.cfg` |\n|---|---|---|---|---|---|\n",
    );
    for e in CATALOG {
        let imp = match e.token {
            Some(Token::Flag { name, inverted }) => {
                format!("`{name}`{}", if inverted { " (inverted)" } else { "" })
            }
            Some(Token::Value { name, .. }) => format!("`[{name}]`"),
            Some(Token::Line { name, index }) => format!("`[{name}]` line {}", index + 1),
            None => "-".into(),
        };
        out.push_str(&format!(
            "| `{}` | {} | {:?} | {} | `{}` | {} |\n",
            e.key,
            e.depot.map_or("-".to_string(), |d| format!("`{d}`")),
            e.group,
            kind_text(&e.kind),
            e.default,
            imp
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// `docs/ORIGINAL_ENGINE.md` carries the table between two markers; regenerate it with
    /// `cargo test -p omsi-launcher-core print_docs_table -- --ignored --nocapture`.
    #[test]
    fn the_documented_table_is_the_catalog() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/ORIGINAL_ENGINE.md");
        // A Windows checkout may have CRLF line endings.
        let doc = std::fs::read_to_string(path).unwrap().replace("\r\n", "\n");
        let a = doc.find("<!-- catalog:start -->\n").unwrap() + "<!-- catalog:start -->\n".len();
        let b = doc.find("<!-- catalog:end -->").unwrap();
        assert_eq!(&doc[a..b], docs_table());
    }

    #[test]
    #[ignore]
    fn print_docs_table() {
        print!("{}", docs_table());
    }

    #[test]
    fn every_default_is_valid_and_keys_are_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for e in CATALOG {
            assert!(seen.insert(e.key.to_ascii_lowercase()), "{} twice", e.key);
            serialize(e, &Value::String(e.default.into())).unwrap_or_else(|_| panic!("{}", e.key));
        }
    }

    #[test]
    fn ranges_are_enforced() {
        let e = entry("graphics.maxFPS").unwrap();
        assert_eq!(serialize(e, &json!(60)).unwrap(), "60");
        assert_eq!(serialize(e, &json!("75")).unwrap(), "75");
        assert!(serialize(e, &json!(5)).is_err() && serialize(e, &json!(201)).is_err());
        assert!(serialize(e, &json!(60.5)).is_err());
        let v = entry("sound.masterVolume").unwrap();
        assert_eq!(serialize(v, &json!(0.5)).unwrap(), "0.5");
        assert!(serialize(v, &json!(1.5)).is_err());
        let c = entry("graphics.realTimeReflections").unwrap();
        assert_eq!(serialize(c, &json!("Full")).unwrap(), "full");
        assert!(serialize(c, &json!("off")).is_err());
        let p = entry("graphics.particles").unwrap();
        assert!(serialize(p, &json!("true,50,false,true")).is_ok());
        assert!(serialize(p, &json!("true,50")).is_err());
    }

    #[test]
    fn launch_args_always_send_every_key() {
        let a = launch_args(&Map::new()).unwrap();
        assert_eq!(a.len(), CATALOG.len());
        assert!(a.contains(&"/set:graphics.maxFPS=60".to_string()));
        let mut s = Map::new();
        s.insert("graphics.maxFPS".into(), json!(90));
        s.insert("traffic.humans".into(), json!(0));
        let a = launch_args(&s).unwrap();
        assert!(a.contains(&"/set:graphics.maxFPS=90".to_string()));
        assert!(a.contains(&"/set:traffic.humans=0".to_string()));
    }

    #[test]
    fn unknown_or_bad_saved_values_stop_the_start() {
        let mut s = Map::new();
        s.insert("graphics.nonsense".into(), json!(1));
        assert!(launch_args(&s).is_err());
        let mut s = Map::new();
        s.insert("graphics.maxFPS".into(), json!(1));
        assert!(launch_args(&s).is_err());
    }

    #[test]
    fn depot_names_map_once() {
        let m = depot_map();
        let mut names: Vec<_> = m.iter().map(|(d, _)| *d).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), m.len());
        assert!(m.contains(&("max_fps", "graphics.maxFPS")));
    }

    #[test]
    fn import_reads_options_cfg_without_touching_it() {
        let d = std::env::temp_dir().join(format!("depot-oe-imp-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let text = "[maxFPS]\n75\n[no_collision]\n[AIMaxCountRandom]\n40\n12\n0\n0\n0\n0\n0\n0\n0\n[performance_minObjSize]\n0.05\n";
        std::fs::write(d.join("options.cfg"), text).unwrap();
        let m = import_from_options(&d);
        assert_eq!(m["graphics.maxFPS"], json!(75));
        assert_eq!(m["simulation.collision"], json!(false));
        assert_eq!(m["traffic.randomVehicles"], json!(40));
        assert_eq!(m["traffic.humans"], json!(12));
        assert_eq!(m["graphics.minObjectScreenPercent"], json!(5.0));
        assert_eq!(
            std::fs::read_to_string(d.join("options.cfg")).unwrap(),
            text
        );
        let _ = std::fs::remove_dir_all(&d);
    }
}
