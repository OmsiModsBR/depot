//! The fixture answers like the documented OmsiLaunch command line; later phases of the
//! original-engine mode test against it, so a change of the documented interface fails here.

use omsi_launcher_lib::original_engine::runner::{OriginalEngineRunner, ProcessRunner};
use std::path::PathBuf;

fn exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_fake-omsilaunch"))
}

fn run(scenario: &str, args: &[&str]) -> (Option<i32>, String) {
    // the scenario travels in the environment of this test process; tests in this file run
    // one after another (one #[test]) so the variable is not shared with another case
    std::env::set_var("FAKE_OL_SCENARIO", scenario);
    let a: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let o = ProcessRunner.run(&exe(), &a).unwrap();
    (o.code, o.stdout)
}

#[test]
fn documented_commands_and_exit_codes() {
    let (c, out) = run("ready", &["/version"]);
    assert_eq!(c, Some(0));
    let v: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(v["protocol_version"], "0.1");

    let (c, out) = run(
        "ready",
        &["/new", "/map:maps\\X\\global.cfg", "/plan", "--json"],
    );
    assert_eq!(c, Some(0));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(out.trim()).unwrap()["IsRunnable"],
        true
    );

    let (c, out) = run("not_runnable", &["/new", "/plan"]);
    assert_eq!(c, Some(1));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(out.trim()).unwrap()["IsRunnable"],
        false
    );

    assert_eq!(run("recovery_pending", &["/recovery-status"]).0, Some(0));
    assert!(run("recovery_pending", &["/recovery-status"])
        .1
        .contains("\"pending\":true"));
    assert_eq!(run("recovery_pending", &["/recover"]).0, Some(0));
    assert_eq!(run("recovery_failed", &["/recover"]).0, Some(8));
    assert_eq!(run("busy", &["/recovery-status"]).0, Some(7));
    assert!(run("ready", &["/list:RuntimeModules"])
        .1
        .contains("omsi-prefetch"));
    assert_eq!(run("ready", &["/spec:x.json"]).0, Some(0));
    assert_eq!(run("crash", &["/spec:x.json"]).0, Some(1));
    assert_eq!(run("ready", &["/nonsense"]).0, Some(2));
}
