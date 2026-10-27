//! The mode flag and the files the original-engine mode keeps under `~/.depot/omsi2/`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

/// Which engine the launcher starts games with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    #[default]
    Depot,
    Omsi2,
}

/// The keys of `launcher.json`: everything is off until the player turns it on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct OriginalEngineConfig {
    /// Setup flag "also open the original OMSI 2 engine with Depot".
    pub enabled: bool,
    /// The engine chosen with the switch (only honoured while OmsiLaunch is installed).
    pub mode: Engine,
    /// The engine used before the last change, so a removed OmsiLaunch falls back to it.
    pub last_mode: Engine,
}

impl OriginalEngineConfig {
    /// The engine games start with: the switch only counts while the flag is on.
    pub fn effective(&self, omsilaunch_installed: bool) -> Engine {
        if self.enabled && omsilaunch_installed {
            self.mode
        } else {
            Engine::Depot
        }
    }

    pub fn set_mode(&mut self, mode: Engine) {
        if self.mode != mode {
            self.last_mode = self.mode;
            self.mode = mode;
        }
    }
}

/// `~/.depot/omsi2/` under the given data folder.
pub fn omsi2_dir_in(data: &Path) -> PathBuf {
    data.join("omsi2")
}

pub fn settings_path_in(data: &Path) -> PathBuf {
    omsi2_dir_in(data).join("settings.json")
}

pub fn modules_path_in(data: &Path) -> PathBuf {
    omsi2_dir_in(data).join("modules.json")
}

/// Where one run's launch description is written; never inside the OMSI 2 folder.
pub fn launch_file_in(data: &Path, id: &str) -> PathBuf {
    let safe: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    omsi2_dir_in(data)
        .join("launch")
        .join(format!("{safe}.json"))
}

/// Writes through a temporary file in the same folder, so a crash never leaves half a file.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path.parent().context("path without a folder")?;
    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let name = path
        .file_name()
        .context("path without a name")?
        .to_string_lossy();
    let tmp = dir.join(format!(".{name}.tmp"));
    std::fs::write(&tmp, bytes).with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("replacing {}", path.display()))?;
    Ok(())
}

fn read_object(path: &Path) -> Map<String, Value> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(t.trim_start_matches('\u{feff}')).ok())
        .and_then(|v| match v {
            Value::Object(m) => Some(m),
            _ => None,
        })
        .unwrap_or_default()
}

/// The player's OMSI 2 mode settings (`settings.json`): the single source of truth sent to
/// OmsiLaunch at every start. A missing or damaged file reads as empty.
pub fn load_settings_in(data: &Path) -> Map<String, Value> {
    read_object(&settings_path_in(data))
}

pub fn save_settings_in(data: &Path, settings: &Map<String, Value>) -> Result<()> {
    write_atomic(
        &settings_path_in(data),
        &serde_json::to_vec_pretty(settings)?,
    )
}

/// Enabled modules and their settings (`modules.json`): `{ "<id>": { "enabled": bool,
/// "settings": { ... } } }`.
pub fn load_modules_in(data: &Path) -> Map<String, Value> {
    read_object(&modules_path_in(data))
}

pub fn save_modules_in(data: &Path, modules: &Map<String, Value>) -> Result<()> {
    write_atomic(&modules_path_in(data), &serde_json::to_vec_pretty(modules)?)
}

/// Removes a finished run's launch description.
pub fn remove_launch_file(path: &Path) {
    let _ = std::fs::remove_file(path);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("depot-oe-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn defaults_are_off_and_depot() {
        let c = OriginalEngineConfig::default();
        assert!(!c.enabled);
        assert_eq!(c.effective(true), Engine::Depot);
    }

    #[test]
    fn older_launcher_json_without_the_key_reads_as_default() {
        let c: OriginalEngineConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(c, OriginalEngineConfig::default());
        let c: OriginalEngineConfig =
            serde_json::from_str(r#"{"enabled":true,"mode":"omsi2","future":1}"#).unwrap();
        assert_eq!(c.mode, Engine::Omsi2);
    }

    #[test]
    fn the_switch_counts_only_with_flag_and_install() {
        let mut c = OriginalEngineConfig {
            enabled: true,
            ..Default::default()
        };
        c.set_mode(Engine::Omsi2);
        assert_eq!(c.last_mode, Engine::Depot);
        assert_eq!(c.effective(false), Engine::Depot);
        assert_eq!(c.effective(true), Engine::Omsi2);
        c.enabled = false;
        assert_eq!(c.effective(true), Engine::Depot);
    }

    #[test]
    fn settings_round_trip_and_damage_reads_empty() {
        let d = tmp("settings");
        assert!(load_settings_in(&d).is_empty());
        let mut s = Map::new();
        s.insert("graphics.maxFPS".into(), Value::from(60));
        save_settings_in(&d, &s).unwrap();
        assert_eq!(load_settings_in(&d), s);
        std::fs::write(settings_path_in(&d), "{ not json").unwrap();
        assert!(load_settings_in(&d).is_empty());
        // no temporary file is left behind
        let left: Vec<_> = std::fs::read_dir(omsi2_dir_in(&d))
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(left.len(), 1);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn launch_file_names_cannot_escape_the_folder() {
        let p = launch_file_in(Path::new("data"), "../../x y");
        assert_eq!(
            p.parent().unwrap(),
            Path::new("data").join("omsi2").join("launch")
        );
        assert!(!p
            .file_name()
            .unwrap()
            .to_string_lossy()
            .contains(['/', '\\', ' ']));
    }
}
