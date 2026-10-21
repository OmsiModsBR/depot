//! The trace harnesses' writer: numbers written so that equal text is equal bits, a trace
//! frame by frame, and the run of a scenario.
//!
//! Numbers are written as `{:?}` writes them - the shortest decimal that reads back to the
//! same bits -, NaN with its bits.

use glam::{DVec3, Quat, Vec3};
use std::fmt::Write;
use std::path::PathBuf;

pub fn n32(x: f32) -> String {
    if x.is_nan() {
        format!("NaN:{:08x}", x.to_bits())
    } else {
        format!("{x:?}")
    }
}

pub fn n64(x: f64) -> String {
    if x.is_nan() {
        format!("NaN:{:016x}", x.to_bits())
    } else {
        format!("{x:?}")
    }
}

pub fn v3(v: Vec3) -> String {
    format!("{} {} {}", n32(v.x), n32(v.y), n32(v.z))
}

pub fn dv3(v: DVec3) -> String {
    format!("{} {} {}", n64(v.x), n64(v.y), n64(v.z))
}

pub fn quat(q: Quat) -> String {
    format!("{} {} {} {}", n32(q.x), n32(q.y), n32(q.z), n32(q.w))
}

pub fn list32(xs: &[f32]) -> String {
    xs.iter().map(|x| n32(*x)).collect::<Vec<_>>().join(" ")
}

/// One scenario's trace: a line for each frame with what went in, then the state. Each
/// harness adds what it writes of the state in its own `impl Trace`.
#[derive(Default)]
pub struct Trace {
    pub text: String,
    pub frame: usize,
}

impl Trace {
    pub fn frame(&mut self, dt: f32, inputs: &str) {
        let _ = writeln!(self.text, "frame {} dt {} {inputs}", self.frame, n32(dt));
        self.frame += 1;
    }

    pub fn line(&mut self, s: &str) {
        let _ = writeln!(self.text, "{s}");
    }
}

/// Writes the scenario's trace where `OMSI_PHYSICS_TRACE_DIR` says; without it, runs it
/// twice and compares (and looks for a state that left the numbers).
pub fn check(name: &str, scenario: fn() -> String) {
    let first = scenario();
    assert!(first.lines().count() > 10, "{name}: no trace");
    // (the inputs may be no number, the state never; a `known` line writes down a value that
    // is no number today, as it is)
    let lost = |l: &&str| {
        !l.starts_with("frame")
            && !l.starts_with("known")
            && l.split_whitespace()
                .any(|w| w == "inf" || w == "-inf" || w.starts_with("NaN"))
    };
    if let Some(l) = first.lines().find(lost) {
        panic!("{name}: the state left the numbers:\n{l}");
    }
    match std::env::var_os("OMSI_PHYSICS_TRACE_DIR") {
        Some(dir) => {
            let dir = PathBuf::from(dir);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(format!("{name}.trace")), first).unwrap();
        }
        None => {
            let second = scenario();
            if first != second {
                let (k, (a, b)) = first
                    .lines()
                    .zip(second.lines())
                    .enumerate()
                    .find(|(_, (a, b))| a != b)
                    .unwrap_or((0, ("", "")));
                panic!("{name}: two runs differ at line {}:\n{a}\n{b}", k + 1);
            }
        }
    }
}
