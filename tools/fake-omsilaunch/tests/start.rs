//! Starting a session as the launcher does: the plan first, nothing started when it is not
//! runnable, the launch description gone whenever nothing runs. One test: the fake is steered
//! by environment variables.

use omsi_launcher_lib::original_engine::client::Client;
use omsi_launcher_lib::original_engine::errors::Fix;
use omsi_launcher_lib::original_engine::launch::World;
use omsi_launcher_lib::original_engine::runner::ProcessRunner;
use omsi_launcher_lib::original_engine::session::Outcome;
use omsi_launcher_lib::original_engine::start::{start, StartError};
use serde_json::{json, Map};
use std::path::PathBuf;

fn world() -> World {
    World::New {
        map: "maps/Grundorf M/global.cfg".into(),
        entrypoint: 2,
    }
}

fn leftovers(data: &std::path::Path) -> usize {
    std::fs::read_dir(data.join("omsi2").join("launch"))
        .map(|d| d.count())
        .unwrap_or(0)
}

#[test]
fn a_start_plans_first_and_leaves_nothing_behind() {
    let dir = std::env::temp_dir().join(format!("depot-fake-start-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let data = dir.join("data");
    let root = dir.join("omsi 2 ünï");
    std::fs::create_dir_all(&root).unwrap();
    let log = dir.join("calls.log");
    std::env::set_var("FAKE_OL_LOG", &log);
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_fake-omsilaunch"));
    let client = Client {
        runner: &ProcessRunner,
        exe: exe.clone(),
        windowed: exe,
    };
    let none = Map::new();

    // a runnable plan starts the game; its description is removed when the game ends
    std::env::set_var("FAKE_OL_SCENARIO", "ready");
    std::env::set_var("FAKE_OL_RUN_MS", "80");
    let mut run = start(&client, &data, &root, "s1", &world(), &none, &none).unwrap();
    assert_eq!(leftovers(&data), 1);
    let outcome = loop {
        if let Some(o) = run.poll() {
            break o;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    assert_eq!(outcome, Outcome::Completed);
    assert_eq!(leftovers(&data), 0);
    let calls = std::fs::read_to_string(&log).unwrap();
    let lines: Vec<&str> = calls.lines().collect();
    // the plan came before the start, both with the same arguments
    let plan = lines.iter().position(|l| l.contains("/plan")).unwrap();
    let go = lines
        .iter()
        .rposition(|l| l.contains("/new") && !l.contains("/plan"))
        .unwrap();
    assert!(plan < go);

    // a plan that is not runnable says why and starts nothing
    std::env::set_var("FAKE_OL_SCENARIO", "not_runnable");
    let before = std::fs::read_to_string(&log).unwrap().lines().count();
    let Err(StartError::NotRunnable(problems)) =
        start(&client, &data, &root, "s2", &world(), &none, &none)
    else {
        panic!("a plan that is not runnable must refuse");
    };
    assert_eq!(problems[0].code, "OL_E_MAP_NOT_FOUND");
    assert_eq!(leftovers(&data), 0);
    let after = std::fs::read_to_string(&log).unwrap();
    assert!(after.lines().skip(before).all(|l| l.contains("/plan")));

    // another session holding the folder offers the right fix
    std::env::set_var("FAKE_OL_SCENARIO", "busy");
    let Err(StartError::NotRunnable(problems)) =
        start(&client, &data, &root, "s3", &world(), &none, &none)
    else {
        panic!("a busy installation must refuse");
    };
    assert_eq!(problems[0].fix, Fix::CloseOtherSession);

    // a path outside the OMSI 2 folder and a value OMSI 2 does not take never reach the program
    std::env::set_var("FAKE_OL_SCENARIO", "ready");
    let before = std::fs::read_to_string(&log).unwrap().lines().count();
    let outside = World::New {
        map: "../x/global.cfg".into(),
        entrypoint: 0,
    };
    assert!(matches!(
        start(&client, &data, &root, "s4", &outside, &none, &none),
        Err(StartError::Invalid(_))
    ));
    let mut bad = Map::new();
    bad.insert("graphics.maxFPS".into(), json!(100000));
    assert!(matches!(
        start(&client, &data, &root, "s5", &world(), &bad, &none),
        Err(StartError::Invalid(_))
    ));
    let mut hostile = Map::new();
    hostile.insert("general.alternateView".into(), json!("true\" /x"));
    assert!(matches!(
        start(&client, &data, &root, "s6", &world(), &hostile, &none),
        Err(StartError::Invalid(_))
    ));
    assert_eq!(std::fs::read_to_string(&log).unwrap().lines().count(), before);
    assert_eq!(leftovers(&data), 0);
    let _ = std::fs::remove_dir_all(&dir);
}
