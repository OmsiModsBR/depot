//! The Setup page's folder checks and disk usage chart.

use omsi_launcher_lib as core;
use omsi_launcher_lib::disk::Node;
use omsi_launcher_ui::kit::Status;
use omsi_launcher_ui::window::*;

fn tree() -> Node {
    let leaf = |n: &str, b: u64| Node {
        name: n.into(),
        bytes: b,
        kids: Vec::new(),
    };
    let part = |n: &str, kids: Vec<Node>| Node {
        name: n.into(),
        bytes: kids.iter().map(|k| k.bytes).sum(),
        kids,
    };
    part(
        "content",
        vec![part(
            "OMSI 2 folder",
            vec![
                part(
                    "Vehicles",
                    vec![part("Bus", vec![leaf("Textures", 60), leaf("Models", 20)])],
                ),
                part(
                    "Maps",
                    vec![part("Town", vec![leaf("Settings and text", 20)])],
                ),
            ],
        )],
    )
}

#[test]
fn a_path_that_no_longer_fits_is_cut_back() {
    let t = tree();
    let mut path = vec![0, 0, 0];
    let (n, ty) = node_at(&t, &mut path);
    assert_eq!((n.name.as_str(), ty), ("Bus", Some("Vehicles")));
    let mut gone = vec![0, 5, 0];
    let (n, _) = node_at(&t, &mut gone);
    assert_eq!(n.name, "OMSI 2 folder");
    assert_eq!(gone, [0]);
}

#[test]
fn the_content_folder_is_never_the_omsi_folder() {
    let base = std::env::temp_dir().join(format!("depot-setup-{}", std::process::id()));
    std::fs::create_dir_all(&base).unwrap();
    let root = base.to_string_lossy().to_string();
    assert_eq!(check(2, &root, &root, "0.3.1").0, Status::Error);
    assert_eq!(
        check(2, &base.join("depot").to_string_lossy(), &root, "0.3.1").0,
        Status::Error
    );
    let beside = base.with_extension("content");
    assert_eq!(
        check(2, &beside.to_string_lossy(), &root, "0.3.1").0,
        Status::Warning
    );
    assert_eq!(check(2, "", &root, "0.3.1").0, Status::Found);
    assert!(core::content_folder_problem(&base, &base).is_some());
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn sizes_read_as_the_chart_says_them() {
    assert_eq!(fmt_size(100 * 1_073_741_824), "100 GB");
    assert_eq!(fmt_size(3 * 536_870_912), "1.5 GB");
}

#[test]
fn the_game_program_line_shows_the_running_version() {
    // (a program beside the launcher: the line names the version it was given, not this
    // crate's own package number)
    let exe = std::env::current_exe().unwrap();
    let (st, text) = check(1, &exe.to_string_lossy(), "", "9.8.7");
    assert_eq!(st, Status::Found);
    assert!(text.contains("9.8.7"), "{text}");
    assert!(!text.contains("0.1.0"), "{text}");
}
