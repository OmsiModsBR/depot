//! The run record: the lines a session's messages become, the cap and its last line.

use omsi_devtools_proto::{
    Command, Event, EventKind, Hello, PhysicsSnapshot, Snapshot, ToLauncher, VarValue,
    WheelSnapshot, PROTOCOL_VERSION,
};
use omsi_devtools_record::{RunRecord, RECORD_CAP_BYTES, TRUNCATED_LINE};
use serde_json::{json, Value};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn hello() -> Hello {
    Hello {
        protocol: PROTOCOL_VERSION,
        build: "0.3.0 (test)".into(),
    }
}

/// 22 November 2026, 10:15:00 UTC.
fn started() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_795_342_500)
}

fn snapshot(frame: u64) -> Snapshot {
    Snapshot {
        frame,
        sim_time_s: 85.5,
        vars: vec![
            ("throttle_in".into(), VarValue::Float(0.5)),
            ("ibis_line".into(), VarValue::Text("92".into())),
        ],
        events: vec![Event {
            frame: frame - 2,
            kind: EventKind::Macro,
            name: "M_door_open".into(),
        }],
        physics: Some(PhysicsSnapshot {
            position: [1.0, 2.0, 3.0],
            velocity: [0.0, 0.0, 10.0],
            wheels: vec![WheelSnapshot {
                load_n: 25000.0,
                slip: 0.25,
                suspension_travel_m: 0.5,
            }],
        }),
    }
}

/// The record's lines, each read back as JSON.
fn lines(record: RunRecord<Vec<u8>>) -> Vec<Value> {
    let text = String::from_utf8(record.finish().unwrap()).unwrap();
    assert!(text.ends_with('\n'));
    text.lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("{l}: {e}")))
        .collect()
}

fn record() -> RunRecord<Vec<u8>> {
    RunRecord::start(Vec::new(), RECORD_CAP_BYTES, &hello(), started()).unwrap()
}

#[test]
fn the_cap_is_50_mb() {
    assert_eq!(RECORD_CAP_BYTES, 50 * 1024 * 1024);
}

#[test]
fn the_first_line_says_the_protocol_the_build_and_the_start() {
    assert_eq!(
        lines(record()),
        vec![json!({
            "t": "run",
            "protocol": PROTOCOL_VERSION,
            "build": "0.3.0 (test)",
            "started": "2026-11-22T10:15:00Z",
        })]
    );
}

#[test]
fn a_snapshot_is_its_line_and_a_line_for_each_event() {
    let mut r = record();
    r.message(&ToLauncher::Snapshot(snapshot(5120))).unwrap();
    let l = lines(r);
    assert_eq!(l.len(), 3);
    assert_eq!(
        l[1],
        json!({
            "t": "snap",
            "frame": 5120,
            "sim_s": 85.5,
            "vars": { "throttle_in": 0.5, "ibis_line": "92" },
            "phys": {
                "position": [1.0, 2.0, 3.0],
                "velocity": [0.0, 0.0, 10.0],
                "wheels": [{ "load_n": 25000.0, "slip": 0.25, "suspension_travel_m": 0.5 }],
            },
        })
    );
    assert_eq!(
        l[2],
        json!({ "t": "ev", "frame": 5118, "kind": "macro", "name": "M_door_open" })
    );
}

#[test]
fn a_snapshot_with_nothing_changed_writes_only_its_events() {
    let mut r = record();
    let mut quiet = snapshot(7);
    quiet.vars.clear();
    quiet.physics = None;
    quiet.events = vec![
        Event {
            frame: 6,
            kind: EventKind::Trigger,
            name: "bus_doorfront0".into(),
        },
        Event {
            frame: 7,
            kind: EventKind::Toggle,
            name: "cp_light".into(),
        },
    ];
    r.message(&ToLauncher::Snapshot(quiet.clone())).unwrap();
    quiet.events.clear();
    r.message(&ToLauncher::Snapshot(quiet)).unwrap();
    let l = lines(r);
    assert_eq!(l.len(), 3);
    assert_eq!(l[1]["kind"], "trigger");
    assert_eq!(l[2]["kind"], "toggle");
}

#[test]
fn only_the_parts_a_snapshot_has_are_in_its_line() {
    let mut r = record();
    let mut vars_only = snapshot(9);
    vars_only.physics = None;
    vars_only.events.clear();
    r.message(&ToLauncher::Snapshot(vars_only)).unwrap();
    let l = lines(r);
    assert!(l[1].get("phys").is_none(), "{}", l[1]);
    assert_eq!(l[1]["vars"]["throttle_in"], 0.5);
}

#[test]
fn an_applied_command_is_a_line_and_marks_the_run() {
    let mut r = record();
    assert_eq!(r.commands(), 0);
    for (frame, command) in [
        (
            10,
            Command::SetVariable {
                name: "throttle_in".into(),
                value: VarValue::Float(1.0),
            },
        ),
        (11, Command::Reload { file_index: 3 }),
        (
            12,
            Command::Subscribe {
                names: vec!["a".into(), "b".into()],
            },
        ),
    ] {
        r.message(&ToLauncher::Applied { frame, command }).unwrap();
    }
    assert_eq!(r.commands(), 3);
    let l = lines(r);
    assert_eq!(
        l[1],
        json!({ "t": "cmd", "frame": 10, "cmd": { "set": "throttle_in", "value": 1.0 } })
    );
    assert_eq!(
        l[2],
        json!({ "t": "cmd", "frame": 11, "cmd": { "reload": 3 } })
    );
    assert_eq!(
        l[3],
        json!({ "t": "cmd", "frame": 12, "cmd": { "subscribe": ["a", "b"] } })
    );
}

#[test]
fn a_hello_and_a_rejected_command_are_no_part_of_the_record() {
    let mut r = record();
    r.message(&ToLauncher::Hello(hello())).unwrap();
    r.message(&ToLauncher::Rejected {
        reason: "no such variable".into(),
    })
    .unwrap();
    assert_eq!(r.commands(), 0);
    assert_eq!(lines(r).len(), 1);
}

#[test]
fn a_name_with_quotes_or_a_line_break_stays_one_line() {
    let mut r = record();
    let mut odd = snapshot(3);
    odd.physics = None;
    odd.events.clear();
    odd.vars = vec![("a \"b\"\nc".into(), VarValue::Text("x\ny".into()))];
    r.message(&ToLauncher::Snapshot(odd)).unwrap();
    let l = lines(r);
    assert_eq!(l.len(), 2);
    assert_eq!(l[1]["vars"]["a \"b\"\nc"], "x\ny");
}

#[test]
fn a_number_that_is_none_is_written_as_null() {
    let mut r = record();
    let mut odd = snapshot(3);
    odd.physics = None;
    odd.events.clear();
    odd.vars = vec![("nan".into(), VarValue::Float(f32::NAN))];
    r.message(&ToLauncher::Snapshot(odd)).unwrap();
    assert_eq!(lines(r)[1]["vars"]["nan"], Value::Null);
}

#[test]
fn a_record_at_its_cap_ends_with_the_truncated_line_and_takes_no_more() {
    let cap = 600;
    let mut r = RunRecord::start(Vec::new(), cap, &hello(), started()).unwrap();
    for frame in 10..200 {
        // (a dropped line is no error)
        r.message(&ToLauncher::Snapshot(snapshot(frame))).unwrap();
        r.message(&ToLauncher::Applied {
            frame,
            command: Command::Reload { file_index: 1 },
        })
        .unwrap();
    }
    assert!(r.truncated());
    let kept_commands = r.commands();
    let text = String::from_utf8(r.finish().unwrap()).unwrap();
    assert!(text.len() as u64 <= cap, "{} bytes", text.len());
    assert!(text.ends_with(TRUNCATED_LINE), "{text}");
    assert_eq!(text.matches("\"truncated\"").count(), 1);
    // every line is whole, and the commands counted are the ones written
    let l: Vec<Value> = text
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(l.last().unwrap(), &json!({ "t": "truncated" }));
    assert_eq!(
        l.iter().filter(|v| v["t"] == "cmd").count() as u64,
        kept_commands
    );
}

#[test]
fn a_record_too_small_for_its_header_is_only_the_truncated_line() {
    let r = RunRecord::start(Vec::new(), 30, &hello(), started()).unwrap();
    assert!(r.truncated());
    assert_eq!(
        String::from_utf8(r.finish().unwrap()).unwrap(),
        TRUNCATED_LINE
    );
}

#[test]
fn every_line_starts_with_its_kind() {
    let mut r = record();
    r.message(&ToLauncher::Snapshot(snapshot(5120))).unwrap();
    r.message(&ToLauncher::Applied {
        frame: 5121,
        command: Command::Reload { file_index: 0 },
    })
    .unwrap();
    let text = String::from_utf8(r.finish().unwrap()).unwrap();
    let starts: Vec<&str> = text.lines().map(|l| &l[..l.find(',').unwrap()]).collect();
    assert_eq!(
        starts,
        [
            "{\"t\":\"run\"",
            "{\"t\":\"snap\"",
            "{\"t\":\"ev\"",
            "{\"t\":\"cmd\""
        ]
    );
}
