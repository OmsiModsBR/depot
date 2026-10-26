//! The typed client, the launch builder and the supervision against the fake OmsiLaunch.

use omsi_launcher_lib::original_engine::client::Client;
use omsi_launcher_lib::original_engine::errors::Fix;
use omsi_launcher_lib::original_engine::launch::{self, World};
use omsi_launcher_lib::original_engine::runner::ProcessRunner;
use omsi_launcher_lib::original_engine::session::{Outcome, Running};
use serde_json::Map;
use std::path::PathBuf;

fn client() -> Client<'static, ProcessRunner> {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_fake-omsilaunch"));
    Client {
        runner: &ProcessRunner,
        exe: exe.clone(),
        windowed: exe,
    }
}

#[test]
fn client_session_and_recovery_against_the_fake() {
    let dir = std::env::temp_dir().join(format!("depot-fake-client-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("calls.log");
    std::env::set_var("FAKE_OL_LOG", &log);
    let c = client();

    std::env::set_var("FAKE_OL_SCENARIO", "ready");
    let v = c.version().unwrap();
    assert_eq!(
        (v.version.as_str(), v.protocol.as_str()),
        ("0.1.0-beta4", "0.1")
    );
    assert!(c.plan(&["/new".into()]).unwrap().runnable);
    let mods = c.modules().unwrap();
    assert_eq!(mods.len(), 1);
    assert_eq!(mods[0].id, "omsi-prefetch");
    assert!(!c.recovery_status().unwrap().pending);

    std::env::set_var("FAKE_OL_SCENARIO", "not_runnable");
    let p = c.plan(&["/new".into()]).unwrap();
    assert!(!p.runnable);
    assert_eq!(p.problems[0].code, "OL_E_MAP_NOT_FOUND");

    std::env::set_var("FAKE_OL_SCENARIO", "busy");
    assert!(c.recovery_status().unwrap().busy);
    let p = c.plan(&["/new".into()]).unwrap();
    assert_eq!(p.problems[0].fix, Fix::CloseOtherSession);

    std::env::set_var("FAKE_OL_SCENARIO", "recovery_pending");
    assert!(c.recovery_status().unwrap().pending);
    let r = c.recover().unwrap();
    assert!(r.recovered && !r.failed);
    std::env::set_var("FAKE_OL_SCENARIO", "recovery_failed");
    assert!(c.recover().unwrap().failed);

    // a start: the description is written outside the OMSI folder, the command line carries
    // every setting, and the description is gone once the session has ended
    std::env::set_var("FAKE_OL_SCENARIO", "ready");
    std::env::set_var("FAKE_OL_RUN_MS", "100");
    let data = dir.join("data");
    let root = dir.join("omsi");
    std::fs::create_dir_all(&root).unwrap();
    let l = launch::build(
        &data,
        &root,
        "t1",
        &World::New {
            map: "maps/G/global.cfg".into(),
            entrypoint: 1,
        },
        &Map::new(),
        &Map::new(),
    )
    .unwrap();
    let child = c.start(&l.args).unwrap();
    let mut run = Running::new(child, l.spec_file.clone());
    assert!(run.poll().is_none());
    let outcome = loop {
        if let Some(o) = run.poll() {
            break o;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    assert_eq!(outcome, Outcome::Completed);
    assert!(!l.spec_file.exists());
    let calls = std::fs::read_to_string(&log).unwrap();
    let last = calls.lines().last().unwrap();
    assert!(
        last.contains("/new")
            && last.contains("/set:graphics.maxFPS=60")
            && last.contains("/spec:")
    );

    std::env::set_var("FAKE_OL_SCENARIO", "crash");
    let l = launch::build(
        &data,
        &root,
        "t2",
        &World::Saved {
            situation: "situations/a.osn".into(),
        },
        &Map::new(),
        &Map::new(),
    )
    .unwrap();
    let mut run = Running::new(c.start(&l.args).unwrap(), l.spec_file.clone());
    let outcome = loop {
        if let Some(o) = run.poll() {
            break o;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    assert!(matches!(outcome, Outcome::Failed { code: Some(1), .. }));
    let _ = std::fs::remove_dir_all(&dir);
}
