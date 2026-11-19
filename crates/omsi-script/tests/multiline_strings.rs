//! A string literal may run over several lines of a script: a pmon's display texts are written
//! a line of the screen a line of the file, `@` between them. Depot ended the string at the
//! line break and read the rest of the text as commands ("unknown token" for each word).
//! Synthetic scripts only.

use omsi_script::{compile, CompileInput, Host, NameId, Program, Stacks, State, Vm};
use std::path::PathBuf;

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "omsi_multiline_strings_{}_{tag}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn build(tag: &str, src: &str) -> Program {
    let dir = scratch(tag);
    std::fs::write(dir.join("t.osc"), src).unwrap();
    std::fs::write(dir.join("v.txt"), "n\n").unwrap();
    std::fs::write(dir.join("s.txt"), "txt\nafter\n").unwrap();
    compile(&CompileInput {
        varlists: vec![dir.join("v.txt")],
        stringvarlists: vec![dir.join("s.txt")],
        scripts: vec![dir.join("t.osc")],
        ..Default::default()
    })
}

struct Quiet;
impl Host for Quiet {
    fn sys_var(&mut self, _: omsi_script::SysVar) -> f32 {
        0.0
    }
    fn callback(&mut self, _: &str, _: NameId, _: &mut Stacks, _: &mut State) {}
    fn sound_trigger(&mut self, _: &str, _: NameId) {}
}

/// Runs `{init}` and gives the string variables `txt` and `after`.
fn run(p: &Program) -> (String, String) {
    let mut st = State::new(p);
    Vm::new().run_init(p, &mut st, &mut Quiet);
    let get = |n: &str| st.str_vars[p.str_var(n).unwrap() as usize].clone();
    (get("txt"), get("after"))
}

#[test]
fn a_string_over_several_lines_is_one_string() {
    let p = build(
        "pmon",
        "{init}\n\
         \"+Display settings 1/5\n\
         @ Units\n\
         @ Time/Date\n\
         @@@@@\" (S.$.txt)\n\
         \"ok\" (S.$.after)\n\
         {end}\n",
    );
    assert!(p.errors.is_empty(), "{:?}", p.errors);
    let (txt, after) = run(&p);
    assert_eq!(txt, "+Display settings 1/5\n@ Units\n@ Time/Date\n@@@@@");
    assert_eq!(after, "ok");
}

#[test]
fn quotes_and_comments_inside_the_text_stay_text() {
    let p = build(
        "quotes",
        "{init}\n\
         \"line one 'not a comment\n\
         line two\" (S.$.txt) ' a comment \"with a quote\n\
         \"ok\" (S.$.after)\n\
         {end}\n",
    );
    assert!(p.errors.is_empty(), "{:?}", p.errors);
    let (txt, after) = run(&p);
    assert_eq!(txt, "line one 'not a comment\nline two");
    assert_eq!(after, "ok");
}

/// A quote nothing closes before the next block is the file's mistake: it ends at its own
/// line, as before, and the block after it still compiles.
#[test]
fn an_unclosed_quote_does_not_swallow_the_next_block() {
    let p = build(
        "unclosed",
        "{init}\n\
         \"broken (S.$.txt)\n\
         {end}\n\
         {frame}\n\
         \"ok\" (S.$.after)\n\
         {end}\n",
    );
    let mut st = State::new(&p);
    let mut vm = Vm::new();
    vm.run_init(&p, &mut st, &mut Quiet);
    vm.run_frame(&p, &mut st, &mut Quiet);
    let after = &st.str_vars[p.str_var("after").unwrap() as usize];
    assert_eq!(after, "ok");
}
