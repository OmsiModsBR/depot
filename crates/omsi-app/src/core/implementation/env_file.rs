//! `depot/env.txt`, read from the content folder: each allowed `NAME=value` is set in the
//! process environment, the refused names are logged.
//!
//! (Only a phone calls it; the reader is plain code so that it is built everywhere.)

#![cfg_attr(not(target_os = "android"), allow(dead_code))]

use super::super::domain::env_file::parse;
use std::path::Path;

/// Reads `env.txt` in `content` (if there is one) and sets the switches it allows.
pub(crate) fn apply(content: &Path) {
    if let Ok(t) = std::fs::read_to_string(content.join("env.txt")) {
        let mut skipped = Vec::new();
        for (k, v) in parse(&t, &mut skipped) {
            log::info!("env.txt: {k}");
            std::env::set_var(k, v);
        }
        if !skipped.is_empty() {
            log::warn!("env.txt: not allowed, ignored: {}", skipped.join(", "));
        }
    }
}
