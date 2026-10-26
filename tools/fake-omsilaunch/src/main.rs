//! Stand-in for `OmsiLaunch.exe` / `OmsiLaunchW.exe` in tests. It speaks the documented
//! command line (`/version`, `/plan`, `/list:RuntimeModules`, `/recovery-status`, `/recover`,
//! a launch with `/spec:`) and the documented exit codes, and writes every call it gets to
//! the file named by `FAKE_OL_LOG` so a test can check the exact arguments Depot passed.
//!
//! `FAKE_OL_SCENARIO` picks the answer: `ready` (default), `not_runnable`, `busy`,
//! `recovery_pending`, `recovery_failed`, `crash` (a launch that ends with code 1).
//! `FAKE_OL_RUN_MS` is how long a launch "plays" (default 50).

use std::io::Write;

fn out(json: &str) {
    println!("{json}");
}

/// The documented `--json` envelope around a result (plans and statuses are printed bare).
fn envelope(args: &[String], command: &str, result: &str) {
    if args.iter().any(|a| a == "--json") {
        out(&format!(
            r#"{{"ok":true,"command":"{command}","protocol_version":"0.1","result":{result}}}"#
        ));
    } else {
        out(result);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Ok(path) = std::env::var("FAKE_OL_LOG") {
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(f, "{}", args.join("\u{1f}"));
        }
    }
    let scenario = std::env::var("FAKE_OL_SCENARIO").unwrap_or_else(|_| "ready".into());
    let has = |p: &str| {
        args.iter()
            .any(|a| a == p || a.starts_with(&format!("{p}:")))
    };
    let code = if has("/version") {
        envelope(
            &args,
            "version",
            r#"{"product":"OmsiLaunch","version":"0.1.0-beta4","protocol_version":"0.1","supported_family":"OMSI_2_3_004_COMMON"}"#,
        );
        0
    } else if has("/recovery-status") || has("/recover") {
        let recover = has("/recover");
        match scenario.as_str() {
            "busy" => {
                eprintln!("OL_E_INSTALLATION_BUSY");
                7
            }
            "recovery_pending" if !recover => {
                envelope(
                    &args,
                    "recover",
                    r#"{"pending":true,"recovered":false,"diagnostics":[]}"#,
                );
                0
            }
            "recovery_failed" if recover => {
                envelope(
                    &args,
                    "recover",
                    r#"{"pending":true,"recovered":false,"diagnostics":["OL_E_RECOVERY_BACKUP_CORRUPT"]}"#,
                );
                8
            }
            "recovery_pending" => {
                envelope(
                    &args,
                    "recover",
                    r#"{"pending":true,"recovered":true,"diagnostics":[]}"#,
                );
                0
            }
            _ => {
                envelope(
                    &args,
                    "recover",
                    r#"{"pending":false,"recovered":false,"diagnostics":[]}"#,
                );
                0
            }
        }
    } else if args
        .iter()
        .any(|a| a == "/list:RuntimeModules" || a.eq_ignore_ascii_case("/list:runtimemodules"))
    {
        envelope(
            &args,
            "runtime-modules.list",
            r#"{"modules":[{"id":"omsi-prefetch","version":"1.0.0","activation":"opt-in","valid":true,"settings":[{"key":"enabled","type":"bool","default":false,"flag":"prefetch"}]}]}"#,
        );
        0
    } else if has("/plan") || has("/validate") {
        match scenario.as_str() {
            "not_runnable" => {
                out(
                    r#"{"IsRunnable":false,"TouchedFiles":[],"PlannedMutations":[],"Diagnostics":[{"Code":"OL_E_MAP_NOT_FOUND","Message":"map not found"}]}"#,
                );
                1
            }
            "busy" => {
                eprintln!("OL_E_INSTALLATION_BUSY");
                7
            }
            _ => {
                out(
                    r#"{"IsRunnable":true,"TouchedFiles":["options.cfg"],"PlannedMutations":["overlay options.cfg"],"Diagnostics":[{"Code":"plugin.integrity.reference","Message":"manifest"}]}"#,
                );
                0
            }
        }
    } else if has("/new") || has("/saved") || has("/spec") || has("/last") {
        match scenario.as_str() {
            "busy" => {
                eprintln!("OL_E_INSTALLATION_BUSY");
                7
            }
            "not_runnable" => 1,
            _ => {
                let ms = std::env::var("FAKE_OL_RUN_MS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(50);
                std::thread::sleep(std::time::Duration::from_millis(ms));
                if scenario == "crash" {
                    1
                } else {
                    0
                }
            }
        }
    } else {
        eprintln!("Unknown argument");
        2
    };
    std::process::exit(code);
}
