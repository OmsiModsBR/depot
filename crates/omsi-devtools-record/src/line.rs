//! The lines of a run record: one JSON object each, its kind in `t`.

use omsi_devtools_proto::{Command, Event, EventKind, Hello, PhysicsSnapshot, Snapshot, VarValue};
use serde_json::{json, Map, Value};
use std::time::{SystemTime, UNIX_EPOCH};

/// The last line of a record that reached its cap.
pub const TRUNCATED_LINE: &str = "{\"t\":\"truncated\"}\n";

/// The first line of a record: the protocol and the build the game said in its hello, and
/// when the record started.
pub(crate) fn header_line(game: &Hello, started: SystemTime) -> String {
    finish(
        "run",
        json!({
            "protocol": game.protocol,
            "build": game.build,
            "started": rfc3339(started),
        }),
    )
}

/// A snapshot's line: the variables that changed, by name, and the physics when the
/// snapshot has it. None for a snapshot with neither (its events have lines of their own).
pub(crate) fn snapshot_line(snapshot: &Snapshot) -> Option<String> {
    if snapshot.vars.is_empty() && snapshot.physics.is_none() {
        return None;
    }
    let mut line = Map::new();
    line.insert("frame".into(), json!(snapshot.frame));
    line.insert("sim_s".into(), json!(snapshot.sim_time_s));
    if !snapshot.vars.is_empty() {
        let vars: Map<String, Value> = snapshot
            .vars
            .iter()
            .map(|(name, value)| (name.clone(), value_json(value)))
            .collect();
        line.insert("vars".into(), Value::Object(vars));
    }
    if let Some(physics) = &snapshot.physics {
        line.insert("phys".into(), physics_json(physics));
    }
    Some(finish("snap", Value::Object(line)))
}

/// The line of a macro, trigger or toggle that fired, with the frame it fired in.
pub(crate) fn event_line(event: &Event) -> String {
    finish(
        "ev",
        json!({
            "frame": event.frame,
            "kind": match event.kind {
                EventKind::Macro => "macro",
                EventKind::Trigger => "trigger",
                EventKind::Toggle => "toggle",
            },
            "name": event.name,
        }),
    )
}

/// The line of a command the game applied at `frame`.
pub(crate) fn command_line(frame: u64, command: &Command) -> String {
    let cmd = match command {
        Command::Subscribe { names } => json!({ "subscribe": names }),
        Command::SetVariable { name, value } => {
            json!({ "set": name, "value": value_json(value) })
        }
        Command::Reload { file_index } => json!({ "reload": file_index }),
    };
    finish("cmd", json!({ "frame": frame, "cmd": cmd }))
}

/// A variable's value: a number or a text. A number that is none (not finite) is `null`.
fn value_json(value: &VarValue) -> Value {
    match value {
        VarValue::Float(number) => json!(number),
        VarValue::Text(text) => json!(text),
    }
}

fn physics_json(physics: &PhysicsSnapshot) -> Value {
    let wheels: Vec<Value> = physics
        .wheels
        .iter()
        .map(|w| {
            json!({
                "load_n": w.load_n,
                "slip": w.slip,
                "suspension_travel_m": w.suspension_travel_m,
            })
        })
        .collect();
    json!({
        "position": physics.position,
        "velocity": physics.velocity,
        "wheels": wheels,
    })
}

/// The fields `fields` (an object) as one line of the kind `kind`: `t` comes first, so that
/// a line says what it is where it starts.
fn finish(kind: &str, fields: Value) -> String {
    let fields = fields.to_string();
    // (`{...}` without its opening brace, after the kind; an object with no field is `{}`)
    let rest = fields.strip_prefix('{').unwrap_or("}");
    let separator = if rest == "}" { "" } else { "," };
    format!("{{\"t\":\"{kind}\"{separator}{rest}\n")
}

/// `time` as RFC 3339 in UTC, to the second (`2026-11-22T10:15:00Z`); a time before 1970
/// reads as 1970.
pub(crate) fn rfc3339(time: SystemTime) -> String {
    let (y, mo, d, h, mi, s) = civil(time);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// `time` in UTC as year, month, day, hour, minute and second.
pub(crate) fn civil(time: SystemTime) -> (i64, u32, u32, u32, u32, u32) {
    let secs = time
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (days, rest) = ((secs / 86_400) as i64, (secs % 86_400) as u32);
    // (days since 1970 to a date of the Gregorian calendar, by eras of 400 years that start
    // on 1 March, so that a leap day is the last day of its year)
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * shifted_month + 2) / 5 + 1) as u32;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    } as u32;
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day, rest / 3_600, rest % 3_600 / 60, rest % 60)
}
