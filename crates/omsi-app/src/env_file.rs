//! `depot/env.txt` on an Android phone's shared storage: the few `OMSI_*` switches it may set.
//!
//! The file sits on storage every app with the "all files" permission can write, so it must
//! not be a way to inject arbitrary process environment: a path switch (`OMSI_ROOT`,
//! `OMSI_CONTENT`, `OMSI_BIN`, ...), a script input (`OMSI_INPUT`) or a server URL
//! (`OMSI_PRESENCE_URL`) would turn a file write into a change of what the program reads, runs
//! or talks to. Only the variables the
//! register marks `env_txt` ([`omsi_cfg::flags`]) - harmless rendering and diagnostic switches
//! whose values are numbers or short words - are taken; anything else is reported in the log
//! and skipped.

// (only a phone reads the file; the parser is plain code so that it is tested everywhere)
#![cfg_attr(not(target_os = "android"), allow(dead_code))]

/// Whether `env.txt` may set the variable `key`: the register marks it so.
fn settable(key: &str) -> bool {
    omsi_cfg::flags::ALL
        .iter()
        .any(|f| f.env_txt && f.name == key)
}

/// Longest value taken (the allowed ones are numbers and short words).
pub const MAX_VALUE: usize = 32;

/// Whether `key=value` may be set from `env.txt`.
pub fn allowed(key: &str, value: &str) -> bool {
    settable(key)
        && value.len() <= MAX_VALUE
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' || c == ',')
}

/// The `(name, value)` pairs of `env.txt`'s text that may be set, in order; `skipped` gets the
/// names of the lines that were refused (for the log).
pub fn parse(text: &str, skipped: &mut Vec<String>) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let (k, v) = (k.trim(), v.trim());
        if allowed(k, v) {
            out.push((k.to_string(), v.to_string()));
        } else {
            skipped.push(k.chars().take(40).collect());
        }
    }
    out
}
