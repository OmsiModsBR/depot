//! Characterization tests for the script VM's failure modes (issue #47): scripts the
//! compiler accepts must run without a panic, and a panic that does come from a block is
//! named in the log. Synthetic scripts only.
//!
//! No test here changes what the VM does; the ones that record a failure that exists today
//! say so, and are the first to change when it is fixed.

use omsi_script::{compile, CompileInput, Host, NameId, Program, Stacks, State, Vm};
use std::path::PathBuf;
use std::sync::{Mutex, Once};

/// Collects every log record, so a test can look for the VM's diagnostics.
struct Capture;
static RECORDS: Mutex<Vec<String>> = Mutex::new(Vec::new());
static INIT: Once = Once::new();

impl log::Log for Capture {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }
    fn log(&self, r: &log::Record) {
        RECORDS
            .lock()
            .unwrap()
            .push(format!("{}: {}", r.level(), r.args()));
    }
    fn flush(&self) {}
}

fn capture_logs() {
    INIT.call_once(|| {
        log::set_logger(&Capture).unwrap();
        log::set_max_level(log::LevelFilter::Trace);
    });
}

fn scratch(tag: &str) -> PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "omsi_vm_robustness_{}_{tag}_{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn build(tag: &str, src: &str) -> Program {
    let dir = scratch(tag);
    std::fs::write(dir.join("t.osc"), src).unwrap();
    std::fs::write(dir.join("v.txt"), "a\nb\nresult\n").unwrap();
    std::fs::write(dir.join("s.txt"), "text\n").unwrap();
    compile(&CompileInput {
        varlists: vec![dir.join("v.txt")],
        stringvarlists: vec![dir.join("s.txt")],
        scripts: vec![dir.join("t.osc")],
        ..Default::default()
    })
}

/// Run init, a frame and every trigger once, the way the game drives a script.
fn drive(p: &Program, st: &mut State, vm: &mut Vm, host: &mut dyn Host) {
    vm.run_init(p, st, host);
    vm.run_frame(p, st, host);
    let mut names: Vec<String> = p.triggers.keys().cloned().collect();
    names.sort();
    for n in names {
        vm.run_trigger(p, &n, st, host);
    }
}

struct Quiet;
impl Host for Quiet {
    fn sys_var(&mut self, _: omsi_script::SysVar) -> f32 {
        0.0
    }
    fn callback(&mut self, _: &str, _: NameId, _: &mut Stacks, _: &mut State) {}
    fn sound_trigger(&mut self, _: &str, _: NameId) {}
}

/// A host whose callback panics, as input-handling code can.
struct PanicsInCallback;
impl Host for PanicsInCallback {
    fn sys_var(&mut self, _: omsi_script::SysVar) -> f32 {
        0.0
    }
    fn callback(&mut self, _: &str, _: NameId, _: &mut Stacks, _: &mut State) {
        panic!("callback failed");
    }
    fn sound_trigger(&mut self, _: &str, _: NameId) {}
}

#[test]
fn string_operators_survive_extreme_numbers() {
    // 1e38 1e38 * is infinity; infinity minus itself is NaN: both reach the counts and the
    // formats of the string operators.
    let nums = [
        "0", "-1", "1", "2.5", "-2.5", "1e9", "-1e9", "1e30", "-1e30", "3.4e38", "(L.L.a)",
    ];
    let strs = [
        "\"\"",
        "\"x\"",
        "\"héllo wörld\"",
        "\"0\"",
        "\"-5\"",
        "\"1e999\"",
        "\"##\"",
    ];
    let ops1 = [
        "$cutBegin",
        "$cutEnd",
        "$SetLengthL",
        "$SetLengthR",
        "$SetLengthC",
        "$*",
        "$IntToStrEnh",
    ];
    let ops0 = [
        "$IntToStr",
        "$StrToFloat",
        "$length",
        "$RemoveSpaces",
        "$d",
        "$__DigitsFirst",
    ];
    let mut src = String::from(
        "{init}\n3.4e38 3.4e38 * (S.L.a)\n(L.L.a) (L.L.a) - (S.L.b)\n{end}\n{frame}\n",
    );
    for s in strs {
        for o in ops0 {
            src += &format!("{s} {o}\n");
        }
        for o in ops1 {
            for n in nums {
                src += &format!("{s} {n} {o}\n");
            }
        }
        // number-first forms: the operators take their text from the string stack and
        // their count from the float stack, in whichever order a script leaves them
        for o in ops1 {
            for n in nums {
                src += &format!("{n} {s} {o}\n");
            }
        }
        src += &format!("{s} {s} $+\n{s} {s} $=\n{s} {s} $<\n{s} {s} $>=\n");
    }
    src += "{end}\n";
    let p = build("strings", &src);
    assert!(p.errors.is_empty(), "{:?}", p.errors);
    let mut st = State::new(&p);
    let mut vm = Vm::new();
    drive(&p, &mut st, &mut vm, &mut Quiet);
}

/// Words a generated script is made of: operators, variables, branches, strings, macros.
const WORDS: &[&str] = &[
    "0",
    "1",
    "-1",
    "2.5",
    "1e9",
    "3.4e38",
    "+",
    "-",
    "*",
    "/",
    "%",
    "=",
    "<",
    ">",
    "<=",
    ">=",
    "&&",
    "||",
    "!",
    "/-/",
    "d",
    "pi",
    "random",
    "sin",
    "arcsin",
    "arctan",
    "min",
    "max",
    "exp",
    "sqrt",
    "sqr",
    "sgn",
    "abs",
    "trunc",
    "l0",
    "l9",
    "s0",
    "s9",
    "(L.L.a)",
    "(S.L.a)",
    "(L.L.b)",
    "(S.L.b)",
    "(S.L.result)",
    "(M.L.nothere)",
    "{if}",
    "{else}",
    "{endif}",
    "\"ab\"",
    "\"\"",
    "$IntToStr",
    "$IntToStrEnh",
    "$+",
    "$*",
    "$cutBegin",
    "$cutEnd",
    "$SetLengthC",
    "$length",
    "$StrToFloat",
    "(S.$.text)",
    "(L.$.text)",
    "%stackdump%",
];

#[test]
fn generated_scripts_never_panic() {
    let mut x: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = move |n: usize| {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        (x % n as u64) as usize
    };
    let mut compiled = 0;
    for round in 0..40 {
        let mut src = String::new();
        for kind in [
            "{macro:m1}",
            "{macro:m2}",
            "{init}",
            "{frame}",
            "{trigger:t1}",
            "{trigger:t2_off}",
        ] {
            src += kind;
            src += "\n";
            for _ in 0..(5 + next(60)) {
                // the top-level blocks call the two macros; the macros call nothing, since
                // macros that call each other grow exponentially (a known open item)
                if !kind.starts_with("{macro") && next(12) == 0 {
                    src += if next(2) == 0 {
                        "(M.L.m1) "
                    } else {
                        "(M.L.m2) "
                    };
                }
                src += WORDS[next(WORDS.len())];
                src += if next(6) == 0 { "\n" } else { " " };
            }
            src += "\n{end}\n";
        }
        let p = build(&format!("gen{round}"), &src);
        compiled += 1;
        let mut st = State::new(&p);
        let mut vm = Vm::new();
        // two passes: the second runs on the variables the first left behind
        drive(&p, &mut st, &mut vm, &mut Quiet);
        drive(&p, &mut st, &mut vm, &mut Quiet);
    }
    assert_eq!(compiled, 40);
}

#[test]
fn unbalanced_branches_compile_and_run() {
    for body in [
        "{if}",
        "{endif}",
        "{else}",
        "{if} {if} {endif}",
        "{endif} {if}",
        "{else} {else}",
        "1 {if} 2 {else} 3 {else} 4 {endif} {endif}",
    ] {
        let p = build(
            "branches",
            &format!("{{init}}\n{body}\n{{end}}\n{{frame}}\n{body}\n{{end}}\n"),
        );
        let mut st = State::new(&p);
        let mut vm = Vm::new();
        drive(&p, &mut st, &mut vm, &mut Quiet);
    }
}

#[test]
fn macro_recursion_is_cut_not_a_panic() {
    capture_logs();
    let p = build(
        "recursion",
        "{macro:m1}\n(M.L.m1)\n{end}\n{init}\n(M.L.m1)\n{end}\n",
    );
    let mut st = State::new(&p);
    let mut vm = Vm::new();
    vm.run_init(&p, &mut st, &mut Quiet);
    assert!(RECORDS
        .lock()
        .unwrap()
        .iter()
        .any(|l| l.contains("recursion too deep")));
}

#[test]
fn a_panic_in_a_callback_names_the_blocks_it_passed_through() {
    capture_logs();
    let p = build(
        "named",
        "{macro:inner_macro}\n(M.V.boom)\n{end}\n{macro:outer_macro}\n(M.L.inner_macro)\n{end}\n\
         {trigger:press_key}\n(M.L.outer_macro)\n{end}\n",
    );
    let mut st = State::new(&p);
    let mut vm = Vm::new();
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        vm.run_trigger(&p, "press_key", &mut st, &mut PanicsInCallback);
    }));
    assert!(r.is_err(), "the panic must still propagate");
    let log = RECORDS.lock().unwrap().join("\n");
    let at = |needle: &str| {
        log.find(needle)
            .unwrap_or_else(|| panic!("{needle} not in log:\n{log}"))
    };
    // innermost first, then outwards
    assert!(at("'inner_macro'") < at("'outer_macro'"), "{log}");
    assert!(at("'outer_macro'") < at("'press_key'"), "{log}");
}

/// Today a panic that is caught outside the VM leaves the machine's macro depth counter
/// raised, and after 64 of them every block of that vehicle is refused as "too deep". This
/// records it; whoever fixes #47 (the cause or the state kept after a panic) should
/// flip the assertion.
#[test]
fn a_caught_panic_leaves_the_depth_counter_raised_today() {
    let p = build(
        "depth",
        "{trigger:boom}\n(M.V.boom)\n{end}\n{trigger:ok}\n1 (S.L.result)\n{end}\n",
    );
    let mut st = State::new(&p);
    let mut vm = Vm::new();
    for _ in 0..64 {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            vm.run_trigger(&p, "boom", &mut st, &mut PanicsInCallback);
        }));
    }
    vm.run_trigger(&p, "ok", &mut st, &mut Quiet);
    let result = p.var("result").unwrap();
    assert_eq!(
        st.get(result),
        0.0,
        "the trigger was refused: the machine is stuck"
    );
}

/// The state of one program run with another program's blocks reads past its variables.
/// One candidate for the report in #47 (a vehicle's state kept across a reload); not
/// confirmed as the cause there.
#[test]
fn state_of_a_smaller_program_panics_today() {
    let small = build("small", "{init}\n1 (S.L.a)\n{end}\n");
    let mut big_src = String::from("{init}\n");
    big_src += "1 (S.L.result)\n{end}\n";
    let big = build("big", &big_src);
    assert!(big.var("result").unwrap() > small.var("a").unwrap());
    let mut st = State::new(&small);
    st.vars.truncate(1);
    let mut vm = Vm::new();
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        vm.run_init(&big, &mut st, &mut Quiet);
    }));
    assert!(r.is_err());
}
