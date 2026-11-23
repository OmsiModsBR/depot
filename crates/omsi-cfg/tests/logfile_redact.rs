//! The private names kept out of the logs, with made-up accounts on the three systems: the
//! user's folder becomes `USER_FOLDER`, the rest of a path stays, and the account's and the
//! computer's names do not show anywhere.

use omsi_cfg::logfile::{Redacting, Redactor};
use std::borrow::Cow;
use std::io::Write;

const ACCOUNT: &str = "zq7fakeaccount";
const COMPUTER: &str = "ZQ7-FAKE-PC";

fn windows() -> Redactor {
    Redactor::new(&[r"C:\Users\zq7fakeaccount"], Some(ACCOUNT), Some(COMPUTER))
}

fn linux() -> Redactor {
    Redactor::new(
        &["/home/zq7fakeaccount"],
        Some(ACCOUNT),
        Some("zq7-fake-pc"),
    )
}

fn macos() -> Redactor {
    Redactor::new(&["/Users/zq7fakeaccount/"], Some(ACCOUNT), None)
}

/// No spelling of the made-up account is left in `text`.
fn clean(text: &str) -> bool {
    !text.to_lowercase().contains(ACCOUNT)
}

#[test]
fn a_path_under_the_home_folder_keeps_its_rest_on_the_three_systems() {
    assert_eq!(
        windows().redact(r"loaded C:\Users\zq7fakeaccount\.depot\settings.cfg (912 bytes)"),
        r"loaded USER_FOLDER\.depot\settings.cfg (912 bytes)"
    );
    assert_eq!(
        linux().redact("loaded /home/zq7fakeaccount/.depot/settings.cfg (912 bytes)"),
        "loaded USER_FOLDER/.depot/settings.cfg (912 bytes)"
    );
    assert_eq!(
        macos().redact("loaded /Users/zq7fakeaccount/Library/x.cfg"),
        "loaded USER_FOLDER/Library/x.cfg"
    );
}

#[test]
fn both_slash_styles_and_any_drive_letter_case_are_the_same_folder() {
    let r = windows();
    for line in [
        r"c:\users\zq7fakeaccount\Documents\OMSI 2\maps\X\global.cfg",
        "C:/Users/zq7fakeaccount/Documents/OMSI 2/maps/X/global.cfg",
        r"C:\\Users\\zq7fakeaccount\\Documents\\OMSI 2\\maps\\X\\global.cfg",
        r"\\?\C:\Users\ZQ7FakeAccount\Documents\OMSI 2\maps\X\global.cfg",
        "file:///C:/Users/zq7fakeaccount/Documents/OMSI%202/maps/X/global.cfg",
    ] {
        let out = r.redact(line);
        assert!(out.contains("USER_FOLDER"), "{out}");
        assert!(clean(&out), "{out}");
        assert!(out.ends_with("global.cfg"), "{out}");
    }
}

#[test]
fn the_home_folder_alone_and_at_the_end_of_a_sentence_is_replaced() {
    assert_eq!(linux().redact("/home/zq7fakeaccount"), "USER_FOLDER");
    assert_eq!(
        linux().redact("working folder: /home/zq7fakeaccount."),
        "working folder: USER_FOLDER."
    );
    assert_eq!(
        windows().redact(r#"root "C:\Users\zq7fakeaccount", map x"#),
        r#"root "USER_FOLDER", map x"#
    );
}

#[test]
fn a_path_that_is_not_under_a_private_folder_is_left_as_it_is() {
    for line in [
        r"loaded D:\OMSI 2\Vehicles\MAN_SD200\MAN_SD80.bus (5120 bytes)",
        "loaded /opt/omsi/maps/Grundorf/global.cfg",
        // another account's folder that only starts the same way
        "/home/zq7fakeaccountant/x.cfg",
        r"C:\Users\zq7fakeaccount2\x.cfg",
    ] {
        for r in [windows(), linux(), macos()] {
            assert!(matches!(r.redact(line), Cow::Borrowed(_)), "{line}");
        }
    }
}

#[test]
fn the_account_and_the_computer_do_not_show_in_other_paths_either() {
    assert_eq!(
        linux().redact("mounted /run/media/zq7fakeaccount/DISK/OMSI 2"),
        "mounted /run/media/USER_NAME/DISK/OMSI 2"
    );
    assert_eq!(
        windows().redact(r"D:\Profiles\ZQ7FAKEACCOUNT\x.cfg"),
        r"D:\Profiles\USER_NAME\x.cfg"
    );
    assert_eq!(
        windows().redact(r"\\ZQ7-FAKE-PC\share\OMSI 2\x.cfg"),
        r"\\COMPUTER_NAME\share\OMSI 2\x.cfg"
    );
    // a word that only holds the name is another word
    assert_eq!(
        linux().redact("/x/xzq7fakeaccount/ and /x/zq7fakeaccount_2/"),
        "/x/xzq7fakeaccount/ and /x/zq7fakeaccount_2/"
    );
}

/// An account called like an ordinary word must not take that word out of every line: a
/// name is replaced where it is a piece of a path, not in running text.
#[test]
fn an_account_named_like_a_word_leaves_the_text_readable() {
    let r = Redactor::new(&["/root"], Some("root"), Some("omsi"));
    assert_eq!(
        r.redact("the OMSI 2 installation root is /opt/omsi/x, found by --root"),
        "the OMSI 2 installation root is /opt/COMPUTER_NAME/x, found by --root"
    );
    assert_eq!(
        r.redact("loaded /root/.depot/x.cfg"),
        "loaded USER_FOLDER/.depot/x.cfg"
    );
}

#[test]
fn a_short_account_name_is_only_replaced_as_the_home_folder() {
    let r = Redactor::new(&["/home/al"], Some("al"), None);
    assert_eq!(
        r.redact("al fresco: /home/al/x.cfg, all else"),
        "al fresco: USER_FOLDER/x.cfg, all else"
    );
}

#[test]
fn a_root_folder_is_nobodys_own() {
    let r = Redactor::new(&["/", r"C:\", ""], None, None);
    for line in ["/usr/share/x", r"C:\OMSI 2\x.cfg"] {
        assert!(matches!(r.redact(line), Cow::Borrowed(_)), "{line}");
    }
}

#[test]
fn names_in_other_alphabets_are_replaced_whole() {
    let r = Redactor::new(&[r"C:\Users\Léo Çağla"], Some("Léo Çağla"), None);
    assert_eq!(
        r.redact(r"ünïcode C:\users\LÉO ÇAĞLA\.depot\game.log ok"),
        r"ünïcode USER_FOLDER\.depot\game.log ok"
    );
}

#[test]
fn a_whole_run_of_log_lines_holds_no_trace_of_the_account() {
    let lines = [
        r"[2026-11-22T10:00:00Z INFO  openomsi_game] command line: C:\Users\zq7fakeaccount\Depot\depot.exe --root D:\OMSI --log-level debug",
        r"[2026-11-22T10:00:00Z INFO  openomsi_game] working folder: C:\Users\zq7fakeaccount\Depot",
        r#"[2026-11-22T10:00:01Z DEBUG omsi_cfg] loaded "C:\\Users\\zq7fakeaccount\\.depot\\settings.cfg" (912 bytes)"#,
        "[2026-11-22T10:00:02Z WARN  omsi_content] c:/users/ZQ7FAKEACCOUNT/Documents/x.sco: no [mesh]",
        r"[2026-11-22T10:00:03Z INFO  omsi_cfg] content folder \\ZQ7-FAKE-PC\Users\zq7fakeaccount\OMSI",
    ];
    let mut log = Redacting::new(Vec::new(), Box::leak(Box::new(windows())));
    for line in lines {
        log.write_all(format!("{line}\n").as_bytes()).unwrap();
    }
    let text = String::from_utf8(log.into_inner()).unwrap();
    assert!(clean(&text), "{text}");
    assert!(!text.contains(COMPUTER), "{text}");
    assert_eq!(text.lines().count(), lines.len());
    assert_eq!(text.matches("USER_FOLDER").count(), 4);
    // what is not private stays readable
    assert!(text.contains(r"--root D:\OMSI --log-level debug"), "{text}");
    assert!(text.contains(r"USER_FOLDER\Depot\depot.exe"), "{text}");
}

/// Windows gives a long folder name a short form too (`ZQ7FAK~1`), and the temporary folder
/// often comes with it: that form is the user's folder as well.
#[test]
fn the_short_windows_form_of_the_user_folder_is_replaced() {
    let r = Redactor::new(
        &[r"C:\Users\Zq7fake Account"],
        Some("Zq7fake Account"),
        Some(COMPUTER),
    );
    for (line, want) in [
        (
            r"loaded C:\Users\ZQ7FAK~1\AppData\Local\Temp\x.tmp (3 bytes)",
            r"loaded USER_FOLDER\AppData\Local\Temp\x.tmp (3 bytes)",
        ),
        ("c:/users/zq7fak~1/a.cfg", "USER_FOLDER/a.cfg"),
        (r"C:\\Users\\ZQ7FAK~2\\a", r"USER_FOLDER\\a"),
        // (many folders alike: two characters, a checksum, the number)
        (r"C:\Users\ZQ3F2A~1\a", r"USER_FOLDER\a"),
        (
            r"no folder C:\Users\ZQ7FAK~1 was found",
            "no folder USER_FOLDER was found",
        ),
    ] {
        assert_eq!(r.redact(line), want, "{line}");
    }
    // another account's short name, and a short name elsewhere, stay
    for line in [
        r"C:\Users\OTHERS~1\a.cfg",
        r"D:\OMSI2~1\Vehicles\a.bus",
        r"C:\PROGRA~1\Depot\depot.exe",
    ] {
        assert!(matches!(r.redact(line), Cow::Borrowed(_)), "{line}");
    }
}

/// A space of the folder written as `%20` (a `file:` address) is the same folder.
#[test]
fn a_folder_with_a_space_is_replaced_in_a_file_address() {
    let r = Redactor::new(&[r"C:\Users\Zq7fake Account"], None, None);
    assert_eq!(
        r.redact("file:///C:/Users/Zq7fake%20Account/a.cfg"),
        "file:///USER_FOLDER/a.cfg"
    );
    assert_eq!(
        r.redact(r"C:\Users\Zq7fake Account\a.cfg"),
        r"USER_FOLDER\a.cfg"
    );
}

/// The user's folder in the middle of another path is that other path: the placeholder does
/// not stick to the folder before it, and the account's name in it still goes.
#[test]
fn a_folder_inside_another_path_is_not_the_user_folder() {
    assert_eq!(
        linux().redact("/mnt/home/zq7fakeaccount/x"),
        "/mnt/home/USER_NAME/x"
    );
    assert_eq!(
        linux().redact("Z:/home/zq7fakeaccount/x"),
        "Z:USER_FOLDER/x"
    );
    assert_eq!(linux().redact("=/home/zq7fakeaccount"), "=USER_FOLDER");
}

/// Bytes that are no text do not pass as they came: they are made text and go through the
/// replacement like any line.
#[test]
fn a_line_that_is_no_text_is_still_redacted() {
    static OWN: std::sync::OnceLock<Redactor> = std::sync::OnceLock::new();
    let r = OWN.get_or_init(linux);
    let mut out = Redacting::new(Vec::new(), r);
    out.write_all(b"loaded /home/zq7fakeaccount/a\xff.cfg\n")
        .unwrap();
    let text = String::from_utf8(out.into_inner()).unwrap();
    assert!(clean(&text), "{text}");
    assert!(text.starts_with("loaded USER_FOLDER/a"), "{text}");
}
