//! String literals that span several lines of a script. Synthetic scripts only.

use omsi_script::{compile, CompileInput, NullHost, Program, State, Vm};
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
    std::fs::write(dir.join("s.txt"), "text\nafter\n").unwrap();
    compile(&CompileInput {
        stringvarlists: vec![dir.join("s.txt")],
        scripts: vec![dir.join("t.osc")],
        ..Default::default()
    })
}

fn run_init(p: &Program) -> State {
    let mut st = State::new(p);
    Vm::new().run_init(p, &mut st, &mut NullHost);
    st
}

fn str_var(p: &Program, st: &State, name: &str) -> String {
    st.str_vars[p.str_var(name).unwrap() as usize].clone()
}

#[test]
fn a_string_goes_on_over_the_next_lines() {
    // the shape of a menu screen in the Volvo pmon: one string, several lines
    let p = build(
        "menu",
        "{init}\n\t\"+Settings 1/5\n\t@ Units\n\t@ Time/Date\n\t@@@@@\" (S.$.text)\n\t\"x\" (S.$.after)\n{end}\n",
    );
    assert!(p.errors.is_empty(), "{:?}", p.errors);
    let st = run_init(&p);
    assert_eq!(
        str_var(&p, &st, "text"),
        "+Settings 1/5\n\t@ Units\n\t@ Time/Date\n\t@@@@@"
    );
    assert_eq!(str_var(&p, &st, "after"), "x");
}

#[test]
fn a_comment_after_the_closing_quote_is_still_a_comment() {
    let p = build(
        "comment",
        "{init}\n\"a\nb\" (S.$.text) ' \"not a string\n\"c\" (S.$.after)\n{end}\n",
    );
    assert!(p.errors.is_empty(), "{:?}", p.errors);
    let st = run_init(&p);
    assert_eq!(str_var(&p, &st, "text"), "a\nb");
    assert_eq!(str_var(&p, &st, "after"), "c");
}

#[test]
fn a_string_open_at_the_end_of_the_file_is_reported() {
    // the lines are read one by one as before, and the error names the line it opens on
    let p = build(
        "unclosed",
        "{init}\n\"x\" (S.$.after)\n{end}\n\"never closed\n",
    );
    assert_eq!(p.errors.len(), 1, "{:?}", p.errors);
    assert_eq!(p.errors[0].line, 4);
    assert!(p.errors[0].message.contains("not closed"));
    let st = run_init(&p);
    assert_eq!(str_var(&p, &st, "after"), "x");
}
