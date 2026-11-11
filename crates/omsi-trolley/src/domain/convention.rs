//! The trolleybus content convention as Depot reads it: the global parameters
//! (`TrolleyWirePlugin.cfg`) and the vehicle registry (`TrolleyWireVehicles.ini`).
//!
//! Values are kept as the files write them; each phase of the feature takes the keys it
//! uses (see `.ai/specs/003-native-trolleybus/contracts/content-convention.md`).

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

/// The global parameters the wire network needs, plus every key of the file
/// (`"section.key"`, lower case) for the later phases.
#[derive(Debug, Clone, PartialEq)]
pub struct ConventionParams {
    /// `[global] spline_keywords`, lower case: a spline is a wire spline when its file path
    /// contains one of them.
    pub spline_keywords: Vec<String>,
    /// `[global] wire_height`: height of the wires above the wire spline, metres.
    pub wire_height: f64,
    /// `[global] wire_x_left` / `wire_x_right`: lateral offset of each wire from the wire
    /// spline's centre line, metres, right positive.
    pub wire_x_left: f64,
    pub wire_x_right: f64,
    pub values: BTreeMap<String, String>,
}

impl ConventionParams {
    /// The raw value of `key` in `section` (both case-insensitive).
    pub fn get(&self, section: &str, key: &str) -> Option<&str> {
        self.values
            .get(&format!(
                "{}.{}",
                section.to_ascii_lowercase(),
                key.to_ascii_lowercase()
            ))
            .map(String::as_str)
    }

    /// Is `file` (a spline's `.sli` path as the map writes it) a wire spline?
    pub fn is_wire_spline(&self, file: &str) -> bool {
        let f = file.to_ascii_lowercase();
        self.spline_keywords.iter().any(|k| f.contains(k.as_str()))
    }
}

/// One `[vehicle:<name>]` section of the registry.
#[derive(Debug, Clone, PartialEq)]
pub struct RegistryEntry {
    /// The text after `vehicle:`.
    pub name: String,
    /// `enabled` (absent counts as enabled).
    pub enabled: bool,
    /// Every key of the section, lower case.
    pub values: BTreeMap<String, String>,
}

impl RegistryEntry {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values
            .get(&key.to_ascii_lowercase())
            .map(String::as_str)
    }
}

/// What was read for the session: the parameters, the registry entries and where they came
/// from.
#[derive(Debug, Clone, PartialEq)]
pub struct Convention {
    pub params: ConventionParams,
    pub entries: Vec<RegistryEntry>,
    pub params_path: PathBuf,
    pub registry_path: Option<PathBuf>,
}

/// Why the convention, or one of its entries, could not be used.
#[derive(Debug, Clone, PartialEq)]
pub enum ConventionError {
    /// The file could not be read.
    Io { path: PathBuf, message: String },
    /// A key the network needs is absent.
    Missing { section: String, key: String },
    /// A key that must be a number is not one.
    NotANumber {
        section: String,
        key: String,
        value: String,
    },
}

impl fmt::Display for ConventionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConventionError::Io { path, message } => {
                write!(f, "{} could not be read: {message}", path.display())
            }
            ConventionError::Missing { section, key } => {
                write!(f, "[{section}] has no {key}")
            }
            ConventionError::NotANumber {
                section,
                key,
                value,
            } => write!(f, "[{section}] {key}={value} is not a number"),
        }
    }
}

impl std::error::Error for ConventionError {}
