//! What the Settings and Controls pages show while the original OMSI 2 is the engine: only
//! what OMSI 2 has. Anything else is the Depot's own and stays out of sight until the Depot
//! engine is chosen again (nothing is deleted, the Depot's files are not touched).

use omsi_launcher_lib::original_engine::catalog::{self, Kind};
use serde_json::{json, Map, Value};

/// Launcher settings that mean the same in both modes: how the launcher itself looks and
/// speaks. (The whole Interface section counts too.)
const LAUNCHER_OWN: &[&str] = &[
    "language",
    "ui_scale",
    "ui_scale_window",
    "ui_opacity",
    "tooltips",
    "launcher_rest",
];

/// Depot settings with a setting of the same meaning and the same kind of value in the
/// catalog that do not carry a `depot` link there.
const TWINS: &[(&str, &str)] = &[
    ("red_steer_spd", "controls.reducedSteeringSpeed"),
    ("auto_clutch", "simulation.automaticClutch"),
    ("collision_pedestrians", "simulation.collisionPedestrians"),
];

/// The OmsiLaunch key of the Depot setting `key`, when OMSI 2 has the setting.
pub fn catalog_key(key: &str) -> Option<&'static str> {
    catalog::depot_map()
        .into_iter()
        .find(|(d, _)| *d == key)
        .map(|(_, k)| k)
        .or_else(|| TWINS.iter().find(|(d, _)| *d == key).map(|(_, k)| *k))
}

/// Settings the catalog links but whose values the Depot's page cannot yet tell apart from
/// OMSI 2's (the three ways of boarding against the ticket-selling number 0 to 2): hidden
/// until the two are matched.
const NOT_YET: &[&str] = &["boarding"];

/// The Depot settings of OMSI 2 mode, each with its OmsiLaunch key.
fn mapped() -> Vec<(&'static str, &'static str)> {
    let mut out: Vec<_> = catalog::depot_map()
        .into_iter()
        .filter(|(d, _)| !NOT_YET.contains(d))
        .collect();
    out.extend(TWINS.iter().copied());
    out
}

/// Whether the setting `key` (of section `interface`, or not) is shown in OMSI 2 mode.
pub fn setting_in_omsi2(key: &str, interface: bool) -> bool {
    interface
        || LAUNCHER_OWN.contains(&key)
        || (!NOT_YET.contains(&key) && catalog_key(key).is_some())
}

fn number(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().replace(',', ".").parse().ok(),
        _ => None,
    }
}

/// A catalog value as the Depot's page keeps it.
fn to_depot(key: &str, v: &Value) -> Value {
    // (the catalog's defaults are text: "true", "150")
    let typed;
    let v = match (v, catalog_key(key).and_then(catalog::entry).map(|e| e.kind)) {
        (Value::String(s), Some(Kind::Bool)) => {
            typed = json!(s == "true");
            &typed
        }
        (Value::String(s), Some(Kind::Int(..))) => {
            typed = s.parse::<i64>().map_or(Value::Null, |n| json!(n));
            &typed
        }
        (Value::String(s), Some(Kind::Float(..))) => {
            typed = s.parse::<f64>().map_or(Value::Null, |n| json!(n));
            &typed
        }
        _ => v,
    };
    match key {
        // (OMSI 2 counts the share in whole percent, the page in hundredths)
        "pax_density" => number(v).map_or(Value::Null, |n| json!(n / 100.0)),
        "mirror_refresh" if v.as_str() == Some("economy") => json!("eco"),
        _ => v.clone(),
    }
}

/// The page's value as the catalog takes it; `None` when OMSI 2 has no such value.
fn to_catalog(key: &str, v: &Value) -> Option<(&'static str, Value)> {
    let ckey = catalog_key(key)?;
    let e = catalog::entry(ckey)?;
    let out = match (key, e.kind) {
        ("pax_density", _) => json!((number(v)? * 100.0).round() as i64),
        ("mirror_refresh", _) => match v.as_str()? {
            "eco" | "economy" => json!("economy"),
            "full" => json!("full"),
            _ => return None,
        },
        (_, Kind::Bool) => match v {
            Value::Bool(b) => json!(b),
            Value::String(s) => json!(matches!(s.as_str(), "1" | "true")),
            Value::Number(n) => json!(n.as_f64()? != 0.0),
            _ => return None,
        },
        (_, Kind::Int(..)) => json!(number(v)?.round() as i64),
        (_, Kind::Float(..)) => json!(number(v)?),
        (_, Kind::Choice(_)) => json!(v.as_str()?),
        _ => return None,
    };
    catalog::serialize(e, &out).ok()?;
    Some((ckey, out))
}

/// The catalog's default of the Depot setting `key`, as the page keeps it.
pub fn default_of(key: &str) -> Option<Value> {
    let e = catalog::entry(catalog_key(key)?)?;
    let v = match e.kind {
        Kind::Bool => json!(e.default == "true"),
        Kind::Int(..) => json!(e.default.parse::<i64>().ok()?),
        Kind::Float(..) => json!(e.default.parse::<f64>().ok()?),
        _ => json!(e.default),
    };
    Some(to_depot(key, &v))
}

/// Whether the dropdown option `key` offers `value` in OMSI 2 mode: only what the catalog
/// takes (no "Unlimited" frame limit, no "Automatic" distance, no frozen mirrors).
pub fn pick_allowed(key: &str, value: &str) -> bool {
    match key {
        "max_fps" | "max_obj_dist" | "mirror_refresh" | "maintenance" => {
            to_catalog(key, &json!(value)).is_some()
        }
        _ => true,
    }
}

/// `view` (the launcher's settings) with the settings OMSI 2 mode keeps laid over it.
pub fn overlay(view: &mut Value, saved: &Map<String, Value>) {
    let all = catalog::effective(saved);
    for (d, c) in mapped() {
        if let Some(v) = all.get(c) {
            view[d] = to_depot(d, v);
        }
    }
}

fn same(a: &Value, b: &Value) -> bool {
    a == b
        || number(a)
            .zip(number(b))
            .is_some_and(|(x, y)| (x - y).abs() < 1e-9)
}

/// Takes the page's changes (`before` against `after`) apart: the ones OMSI 2 mode keeps go
/// into `saved` (true when any did), the launcher's own are returned.
pub fn store_back(
    before: &Value,
    after: &Value,
    saved: &mut Map<String, Value>,
) -> (bool, Vec<(String, Value)>) {
    let mapped = mapped();
    let mut kept = false;
    for (d, _) in &mapped {
        let (b, a) = (&before[*d], &after[*d]);
        if same(b, a) {
            continue;
        }
        if let Some((c, v)) = to_catalog(d, a) {
            saved.insert(c.to_string(), v);
            kept = true;
        }
    }
    let mut own = Vec::new();
    for (k, a) in after.as_object().into_iter().flatten() {
        if mapped.iter().any(|(d, _)| d == k) {
            continue;
        }
        if !before.get(k).is_some_and(|b| same(b, a)) {
            own.push((k.clone(), a.clone()));
        }
    }
    (kept, own)
}

/// Whether the key action `action` exists in OMSI 2. Only the actions the Depot added are
/// named here; a bus's own triggers and everything else stay.
pub fn action_in_omsi2(action: &str) -> bool {
    let a = action.to_ascii_lowercase();
    !(a.starts_with("vr_")
        || a.starts_with("chat_")
        || a == "voice_radio"
        || a == "doors_all"
        || a == "view_toggle_interior"
        || matches!(a.as_str(), "door_1" | "door_2" | "door_3" | "door_4"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_twin_is_a_catalog_key() {
        for (d, k) in TWINS {
            assert!(catalog::entry(k).is_some(), "{d} -> {k}");
        }
    }

    #[test]
    fn the_catalog_links_are_shown() {
        for (d, _) in mapped() {
            assert!(setting_in_omsi2(d, false), "{d}");
        }
        assert!(!setting_in_omsi2("vr", false));
        assert!(setting_in_omsi2("ui_accent", true));
        assert!(setting_in_omsi2("language", false));
    }

    #[test]
    fn the_page_and_the_catalog_round_trip() {
        let mut saved = Map::new();
        saved.insert("traffic.passengerFactorPercent".into(), json!(150));
        saved.insert("graphics.realTimeReflections".into(), json!("economy"));
        let mut view = json!({"language": "ENG", "pax_density": 1.0});
        overlay(&mut view, &saved);
        assert_eq!(view["pax_density"], json!(1.5));
        assert_eq!(view["mirror_refresh"], json!("eco"));
        assert_eq!(view["driver"], json!(true));
        let before = view.clone();
        view["pax_density"] = json!(0.5);
        view["mirror_refresh"] = json!("full");
        view["language"] = json!("POR");
        view["max_fps"] = json!("120");
        let (kept, own) = store_back(&before, &view, &mut saved);
        assert!(kept);
        assert_eq!(saved["traffic.passengerFactorPercent"], json!(50));
        assert_eq!(saved["graphics.realTimeReflections"], json!("full"));
        assert_eq!(saved["graphics.maxFPS"], json!(120));
        assert_eq!(own, vec![("language".to_string(), json!("POR"))]);
        // everything saved is something OmsiLaunch takes
        assert!(catalog::launch_args(&saved).is_ok());
    }

    #[test]
    fn what_the_catalog_lacks_is_not_offered() {
        assert!(!pick_allowed("max_fps", "0"));
        assert!(!pick_allowed("max_fps", "1000"));
        assert!(pick_allowed("max_fps", "60"));
        assert!(!pick_allowed("max_obj_dist", "auto"));
        assert!(pick_allowed("max_obj_dist", "1500"));
        assert!(!pick_allowed("mirror_refresh", "off"));
        assert!(pick_allowed("mirror_refresh", "eco"));
        assert!(!setting_in_omsi2("boarding", false));
    }

    #[test]
    fn the_defaults_are_the_catalogs() {
        assert_eq!(default_of("max_fps"), Some(json!(60)));
        assert_eq!(default_of("pax_density"), Some(json!(1.0)));
        assert_eq!(default_of("mirror_refresh"), Some(json!("eco")));
        assert_eq!(default_of("driver"), Some(json!(true)));
        assert_eq!(default_of("vr"), None);
    }

    #[test]
    fn depot_only_actions_are_left_out() {
        assert!(!action_in_omsi2("vr_recenter"));
        assert!(!action_in_omsi2("chat_open"));
        assert!(!action_in_omsi2("voice_radio"));
        assert!(!action_in_omsi2("doors_all"));
        assert!(action_in_omsi2("throttle"));
        assert!(action_in_omsi2("kw_something_toggle"));
    }
}
