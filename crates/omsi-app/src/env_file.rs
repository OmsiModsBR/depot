//! `depot/env.txt` on an Android phone's shared storage: the few `OMSI_*` switches it may set.
//!
//! The file sits on storage every app with the "all files" permission can write, so it must
//! not be a way to inject arbitrary process environment: a path switch (`OMSI_ROOT`,
//! `OMSI_CONTENT`, `OMSI_BIN`, ...), a script input (`OMSI_INPUT`), a server URL
//! (`OMSI_PRESENCE_URL`) or a program to run (`OMSI_WINE`, `OMSI_CLOUDFLARED`) would turn a file
//! write into a change of what the program reads, runs or talks to. Only the variables the
//! register marks `env_txt` ([`omsi_cfg::flags`]) - harmless rendering and diagnostic switches
//! whose values are numbers or short words - are taken; anything else is reported in the log
//! and skipped.

// (only a phone reads the file; the parser is plain code so that it is tested everywhere)
#![cfg_attr(not(target_os = "android"), allow(dead_code))]

/// Whether `env.txt` may set the variable `key`: the register marks it so.
fn settable(key: &str) -> bool {
    omsi_cfg::flags::ALL.iter().any(|f| f.env_txt && f.name == key)
}

/// Longest value taken (the allowed ones are numbers and short words).
const MAX_VALUE: usize = 32;

/// Whether `key=value` may be set from `env.txt`.
pub(crate) fn allowed(key: &str, value: &str) -> bool {
    settable(key) && value.len() <= MAX_VALUE && value.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' || c == ',')
}

/// The `(name, value)` pairs of `env.txt`'s text that may be set, in order; `skipped` gets the
/// names of the lines that were refused (for the log).
pub(crate) fn parse(text: &str, skipped: &mut Vec<String>) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        let (k, v) = (k.trim(), v.trim());
        if allowed(k, v) {
            out.push((k.to_string(), v.to_string()));
        } else {
            skipped.push(k.chars().take(40).collect());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_settable_switches_are_the_reviewed_ones() {
        let mut got: Vec<&str> = omsi_cfg::flags::ALL.iter().filter(|f| f.env_txt).map(|f| f.name).collect();
        got.sort_unstable();
        let mut want = vec![
            "OMSI_TOUCH", "OMSI_MOBILE", "OMSI_LAUNCHER_SIZE",
            "OMSI_BACKEND", "OMSI_SAFE_GPU", "OMSI_FULL_GPU", "OMSI_GPU_LIMITS", "OMSI_BASIC_PIPELINES", "OMSI_GRAPHICS",
            "OMSI_NO_SHADOWS", "OMSI_NO_CORONAS", "OMSI_NO_ENVMAP", "OMSI_NO_BUMP", "OMSI_NO_CULL", "OMSI_NO_SURF",
            "OMSI_NO_BC", "OMSI_NO_TEXCOMPRESS", "OMSI_NO_FXAA", "OMSI_NO_AO", "OMSI_NO_RT", "OMSI_NO_RT_GRADE",
            "OMSI_NO_CLOUDS", "OMSI_NO_ENHANCED", "OMSI_NO_PLUGINS",
            "OMSI_MAX_FPS", "OMSI_TEXTURE_MEMORY", "OMSI_MUTE", "OMSI_SEED",
            "OMSI_PROFILE", "OMSI_GPU_TIMERS", "OMSI_DEBUG_NAV", "OMSI_DEBUG_STARTUP", "OMSI_DEBUG_MISSING",
        ];
        want.sort_unstable();
        // (a variable marked `env_txt` in the register, or unmarked, changes what a file other
        // apps can write may set: look at it, then change this list)
        assert_eq!(got, want);
    }

    #[test]
    fn only_harmless_switches_are_taken() {
        let mut skipped = Vec::new();
        let got = parse("OMSI_NO_SHADOWS=1\n# a comment\nOMSI_MAX_FPS = 30 \nOMSI_BACKEND=gl\n", &mut skipped);
        assert_eq!(got, vec![("OMSI_NO_SHADOWS".into(), "1".into()), ("OMSI_MAX_FPS".into(), "30".into()), ("OMSI_BACKEND".into(), "gl".into())]);
        assert!(skipped.is_empty());
    }

    #[test]
    fn paths_programs_urls_and_scripts_are_refused() {
        let mut skipped = Vec::new();
        let text = "OMSI_ROOT=/sdcard/evil\nOMSI_CONTENT=/sdcard/evil\nOMSI_CONTENT_ZIP=/sdcard/e.zip\nOMSI_BIN=/data/x\nOMSI_WINE=/bin/sh\nOMSI_CLOUDFLARED=/bin/sh\n\
OMSI_PRESENCE_URL=https://evil.example\nOMSI_INPUT=t=1 key F1\nOMSI_LAUNCHER_INPUT=t=1 click 1,1\nLD_PRELOAD=/sdcard/x.so\nPATH=/sdcard\nHOME=/sdcard\n";
        assert!(parse(text, &mut skipped).is_empty());
        assert_eq!(skipped.len(), 12, "{skipped:?}");
    }

    #[test]
    fn an_allowed_switch_with_a_strange_value_is_refused() {
        assert!(allowed("OMSI_MAX_FPS", "60"));
        assert!(!allowed("OMSI_MAX_FPS", "60; rm -rf"));
        assert!(!allowed("OMSI_BACKEND", "/etc/passwd"));
        assert!(!allowed("OMSI_BACKEND", &"a".repeat(MAX_VALUE + 1)));
        assert!(!allowed("omsi_no_shadows", "1"));
    }
}
