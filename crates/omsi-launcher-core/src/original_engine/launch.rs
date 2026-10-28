//! One start of the original OMSI 2: the launch description (a `LaunchSpec` file under
//! `~/.depot/omsi2/launch/`, never inside the OMSI 2 folder) and the command line made from
//! it, the saved settings and the enabled modules.

use super::{catalog, config};
use anyhow::{bail, Result};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum World {
    /// A new session on a map: `maps\Name\global.cfg` and the start point's index in the
    /// list OMSI 2 presents.
    New { map: String, entrypoint: u32 },
    /// A saved situation: `situations\...\file.osn`.
    Saved { situation: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launch {
    pub spec_file: PathBuf,
    pub args: Vec<String>,
}

/// `maps/Name/global.cfg` (as Depot names maps) in the form OmsiLaunch takes.
pub fn identity(path: &str) -> Result<String> {
    let p = path.trim().replace('/', "\\");
    let ok = !p.is_empty()
        && !p.starts_with('\\')
        && !p.contains(':')
        && p.split('\\')
            .all(|c| !c.is_empty() && c != "." && c != "..")
        && !p.chars().any(|c| c.is_control());
    if !ok {
        bail!("{path} is not a path inside the OMSI 2 folder");
    }
    Ok(p)
}

fn optional(v: Option<Value>) -> Value {
    match v {
        Some(v) => json!({"Presence": 1, "Value": v}),
        None => json!({"Presence": 0, "Value": null}),
    }
}

/// The launch description for `root`: the world and the usual behavior; the settings travel
/// as `/set:` arguments so the file stays the same shape for every start.
pub fn spec_json(root: &Path, world: &World) -> Result<Value> {
    let (mode, map, situation, entry) = match world {
        World::New { map, entrypoint } => (
            0,
            Some(json!(identity(map)?)),
            None,
            Some(json!(entrypoint)),
        ),
        World::Saved { situation } => (1, None, Some(json!(identity(situation)?)), None),
    };
    Ok(json!({
        "Installation": {"RootPath": root.to_string_lossy(), "ExpectedExecutableSha256": null},
        "World": {
            "Mode": mode,
            "MapIdentity": optional(map),
            "SituationIdentity": optional(situation),
            "PresentedEntrypointIndex": optional(entry),
            "EntrypointIdentity": optional(None)
        },
        "Date": {"Mode": 0, "Value": optional(None)},
        "Time": {"Mode": 0, "Value": optional(None)},
        "PlayerVehicle": optional(None),
        "Environment": {
            "General": {}, "Advanced": {}, "Graphics": {}, "AdvancedGraphics": {},
            "Sound": {}, "AiPassengers": {}, "Keyboard": {}, "Controllers": {}
        },
        "Behavior": {
            "RestoreConfiguration": true,
            "SuppressStaleClosecheckWarning": true,
            "StartupTimeoutSeconds": 180,
            "ShutdownTimeoutSeconds": 30
        },
        "Presentation": {
            "Splash": 1,
            "Language": optional(None),
            "CustomAssetDirectory": optional(None)
        },
        "InternetTextures": {"Mode": 0, "OverrideProfilePath": optional(None)}
    }))
}

fn word_ok(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

fn value_text(v: &Value) -> Option<String> {
    let t = match v {
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        _ => return None,
    };
    (t.len() <= 256 && !t.chars().any(|c| c.is_control())).then_some(t)
}

/// `/module:<id>` and `/module-setting:<id>.<key>=<value>` for every enabled module.
pub fn module_args(modules: &Map<String, Value>) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for (id, m) in modules {
        if !word_ok(id) {
            bail!("{id} is not a module name");
        }
        if !m.get("enabled").and_then(Value::as_bool).unwrap_or(false) {
            continue;
        }
        out.push(format!("/module:{id}"));
        if let Some(s) = m.get("settings").and_then(Value::as_object) {
            for (k, v) in s {
                let t = value_text(v).filter(|_| word_ok(k));
                match t {
                    Some(t) => out.push(format!("/module-setting:{id}.{k}={t}")),
                    None => bail!("{id}: the setting {k} cannot be sent"),
                }
            }
        }
    }
    Ok(out)
}

/// Writes the launch description for run `id` and returns it with the command line.
pub fn build(
    data: &Path,
    root: &Path,
    id: &str,
    world: &World,
    settings: &Map<String, Value>,
    modules: &Map<String, Value>,
) -> Result<Launch> {
    let spec = spec_json(root, world)?;
    let mut args = catalog::launch_args(settings)?;
    args.extend(module_args(modules)?);
    let file = config::launch_file_in(data, id);
    config::write_atomic(&file, &serde_json::to_vec_pretty(&spec)?)?;
    let mut all = vec![format!("/spec:{}", file.display())];
    // a saved situation or a new map is already in the file; `/new` and `/saved` pick the mode
    all.push(match world {
        World::New { .. } => "/new".into(),
        World::Saved { situation } => format!("/saved:{}", identity(situation)?),
    });
    all.extend(args);
    Ok(Launch {
        spec_file: file,
        args: all,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("depot-oe-launch-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn identities_are_converted_and_cannot_leave_the_folder() {
        assert_eq!(
            identity("maps/Grundorf/global.cfg").unwrap(),
            "maps\\Grundorf\\global.cfg"
        );
        for bad in [
            "../x",
            "maps/../../x",
            "/abs",
            "C:/x",
            "",
            "maps//x",
            "a\nb",
        ] {
            assert!(identity(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn new_map_spec_has_the_world_and_nothing_unset_is_invented() {
        let v = spec_json(
            Path::new("D:\\OMSI 2"),
            &World::New {
                map: "maps/Grundorf/global.cfg".into(),
                entrypoint: 2,
            },
        )
        .unwrap();
        assert_eq!(v["World"]["Mode"], 0);
        assert_eq!(
            v["World"]["MapIdentity"]["Value"],
            "maps\\Grundorf\\global.cfg"
        );
        assert_eq!(v["World"]["PresentedEntrypointIndex"]["Value"], 2);
        assert_eq!(v["Date"]["Mode"], 0);
        assert_eq!(v["PlayerVehicle"]["Presence"], 0);
        assert_eq!(v["Behavior"]["RestoreConfiguration"], true);
        let s = spec_json(
            Path::new("D:\\OMSI 2"),
            &World::Saved {
                situation: "situations/Linie 5.osn".into(),
            },
        )
        .unwrap();
        assert_eq!(s["World"]["Mode"], 1);
        assert_eq!(
            s["World"]["SituationIdentity"]["Value"],
            "situations\\Linie 5.osn"
        );
    }

    #[test]
    fn build_writes_the_file_outside_the_omsi_folder_and_sends_every_setting() {
        let data = tmp("d");
        let root = tmp("r");
        let mut modules = Map::new();
        modules.insert(
            "omsi-prefetch".into(),
            json!({"enabled": true, "settings": {"ramBudgetMb": 2048}}),
        );
        modules.insert(
            "openhud".into(),
            json!({"enabled": false, "settings": {"x": 1}}),
        );
        let l = build(
            &data,
            &root,
            "run-1",
            &World::New {
                map: "maps/G/global.cfg".into(),
                entrypoint: 0,
            },
            &Map::new(),
            &modules,
        )
        .unwrap();
        assert!(l.spec_file.starts_with(&data) && !l.spec_file.starts_with(&root));
        assert!(l.spec_file.is_file());
        assert_eq!(l.args[0], format!("/spec:{}", l.spec_file.display()));
        assert_eq!(l.args[1], "/new");
        assert!(l.args.contains(&"/set:graphics.maxFPS=60".to_string()));
        assert!(l.args.contains(&"/module:omsi-prefetch".to_string()));
        assert!(l
            .args
            .contains(&"/module-setting:omsi-prefetch.ramBudgetMb=2048".to_string()));
        assert!(!l.args.iter().any(|a| a.contains("openhud")));
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
        config::remove_launch_file(&l.spec_file);
        assert!(!l.spec_file.exists());
        let _ = std::fs::remove_dir_all(&data);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn module_arguments_cannot_carry_extra_flags() {
        let mut m = Map::new();
        m.insert("a b".into(), json!({"enabled": true}));
        assert!(module_args(&m).is_err());
        let mut m = Map::new();
        m.insert(
            "ok".into(),
            json!({"enabled": true, "settings": {"k": "v\n/new"}}),
        );
        assert!(module_args(&m).is_err());
        let mut m = Map::new();
        m.insert(
            "ok".into(),
            json!({"enabled": true, "settings": {"k=v": 1}}),
        );
        assert!(module_args(&m).is_err());
    }
}
