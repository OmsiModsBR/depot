//! Reading the convention's two files.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::ini;
use crate::domain::convention::{Convention, ConventionError, ConventionParams, RegistryEntry};
use crate::interface::output::{find_in_roots, vfs};

/// The global parameter file, relative to a content root.
pub const PARAMS_FILE: &str = "Plugins/TrolleyWirePlugin.cfg";
/// The vehicle registry, relative to a content root.
pub const REGISTRY_FILE: &str = "Plugins/TrolleyWireVehicles.ini";

const GLOBAL: &str = "global";

/// Read the convention from the content roots (Depot's content folder first, then the
/// OMSI 2 folder: the first root that has the file wins, each file on its own).
///
/// `Ok(None)`: no parameter file anywhere, so the session has no trolleybus content. A
/// registry that is missing or unreadable leaves the entries empty (the network still
/// builds; no vehicle is a trolleybus).
pub fn load_convention() -> Result<Option<Convention>, ConventionError> {
    let Some((_, params_path)) = find_in_roots(PARAMS_FILE) else {
        return Ok(None);
    };
    let params = parse_params(&read(&params_path)?)?;
    let (entries, registry_path) = match find_in_roots(REGISTRY_FILE) {
        Some((_, p)) => match read(&p) {
            Ok(text) => (parse_registry(&text), Some(p)),
            Err(e) => {
                log::warn!("trolleybus: {e}");
                (Vec::new(), None)
            }
        },
        None => (Vec::new(), None),
    };
    Ok(Some(Convention {
        params,
        entries,
        params_path,
        registry_path,
    }))
}

fn read(path: &Path) -> Result<String, ConventionError> {
    vfs::read_text(path).map_err(|e| ConventionError::Io {
        path: PathBuf::from(path),
        message: e.to_string(),
    })
}

/// The global parameters from the text of the parameter file.
pub fn parse_params(text: &str) -> Result<ConventionParams, ConventionError> {
    let mut values = BTreeMap::new();
    for s in ini::parse(text) {
        let section = s.name.to_ascii_lowercase();
        for (k, v) in s.entries {
            values.insert(format!("{section}.{k}"), v);
        }
    }
    let text = |key: &str| values.get(&format!("{GLOBAL}.{key}")).map(String::as_str);
    let raw = |key: &str| required(GLOBAL, key, text(key));
    let number = |key: &str| read_number(GLOBAL, key, text(key));
    let spline_keywords = raw("spline_keywords")?
        .split(',')
        .map(|k| k.trim().to_ascii_lowercase())
        .filter(|k| !k.is_empty())
        .collect();
    Ok(ConventionParams {
        spline_keywords,
        wire_height: number("wire_height")?,
        wire_x_left: number("wire_x_left")?,
        wire_x_right: number("wire_x_right")?,
        values,
    })
}

/// The text `key` of `section` holds (missing when there is none).
pub(crate) fn required<'a>(
    section: &str,
    key: &str,
    text: Option<&'a str>,
) -> Result<&'a str, ConventionError> {
    text.ok_or_else(|| ConventionError::Missing {
        section: section.into(),
        key: key.into(),
    })
}

/// The number `key` of `section` holds: missing when there is none, not a number when it does
/// not read as a finite one.
pub(crate) fn read_number(
    section: &str,
    key: &str,
    text: Option<&str>,
) -> Result<f64, ConventionError> {
    let v = required(section, key, text)?;
    number(v).ok_or_else(|| ConventionError::NotANumber {
        section: section.into(),
        key: key.into(),
        value: v.into(),
    })
}

/// A finite number, spaces around it allowed.
pub(crate) fn number(v: &str) -> Option<f64> {
    v.trim().parse::<f64>().ok().filter(|x| x.is_finite())
}

/// The `[vehicle:<name>]` sections of the registry, in file order. Other sections are
/// ignored.
pub fn parse_registry(text: &str) -> Vec<RegistryEntry> {
    ini::parse(text)
        .into_iter()
        .filter_map(|s| {
            let (prefix, name) = s.name.split_once(':')?;
            if !prefix.trim().eq_ignore_ascii_case("vehicle") {
                return None;
            }
            let values: BTreeMap<String, String> = s.entries.into_iter().collect();
            let enabled = values.get("enabled").is_none_or(|v| v.trim() != "0");
            Some(RegistryEntry {
                name: name.trim().to_string(),
                enabled,
                values,
            })
        })
        .collect()
}
