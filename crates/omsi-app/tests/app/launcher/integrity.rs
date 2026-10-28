use omsi_integrity::Category;
use omsi_integrity::Progress;
use omsi_integrity::Report;
use omsi_integrity::Severity;
use openomsi_game::testing::launcher::integrity::*;

/// Cancel while the compared one is read: that read sees it and nothing is sent.
#[test]
fn cancel_stops_the_comparison_too() {
    let clicked = Progress::new();
    let seen = std::cell::Cell::new(false);
    let run = |t: &str, p: &Progress| {
        if t == "baseline" {
            clicked.cancel();
            seen.set(p.cancelled());
        }
        Report::default()
    };
    assert!(check_and_compare(run, "map", Some("baseline"), &clicked).is_none());
    assert!(seen.get(), "the comparison runs on its own progress");
    // without a comparison, or not cancelled: the report
    let p = Progress::new();
    let plain = |_: &str, _: &Progress| Report::default();
    assert!(check_and_compare(plain, "map", None, &p).is_some());
    assert!(check_and_compare(plain, "map", Some("other"), &p).is_some());
}

#[test]
fn the_file_name_holds_what_was_checked_and_when() {
    let d = Done {
        name: "Spandau: Nord/2.1".into(),
        target: "Spandau Nord".into(),
        compared: None,
        when: "2026-10-27 02:40".into(),
        report: Report::default(),
    };
    assert_eq!(
        file_name(&d),
        "Spandau_ Nord_2.1 - integrity 2026-10-27 0240.txt"
    );
}

#[test]
fn a_map_has_its_tabs_and_others_only_with_findings() {
    let mut r = Report::default();
    assert_eq!(
        tabs(0, &r),
        vec![
            Category::Tiles,
            Category::Objects,
            Category::Splines,
            Category::Models,
            Category::Textures,
            Category::AiLists,
            Category::Timetable,
            Category::Paths
        ]
    );
    r.findings.push(omsi_integrity::Finding {
        severity: Severity::Error,
        code: "map-unreadable",
        file: "Maps/x/global.cfg".into(),
        message: String::new(),
        detail: None,
        referenced_by: None,
        known: false,
    });
    assert_eq!(tabs(0, &r)[0], Category::Map);
    assert!(tabs(1, &Report::default()).contains(&Category::Sounds));
}

#[test]
fn rows_follow_the_tab_the_severities_and_the_text() {
    let f = |severity, code: &'static str, file: &str| omsi_integrity::Finding {
        severity,
        code,
        file: file.into(),
        message: "missing".into(),
        detail: None,
        referenced_by: None,
        known: false,
    };
    let r = Report {
        findings: vec![
            f(Severity::Error, "object-missing", "a/house.sco"),
            f(Severity::Warning, "texture-missing", "a/wall.bmp"),
            f(Severity::NotOptimized, "texture-npot", "a/roof.bmp"),
        ],
        ..Default::default()
    };
    let items = |rows: Vec<Row>| {
        rows.iter()
            .filter(|r| matches!(r, Row::Item { .. }))
            .count()
    };
    assert_eq!(items(rows_of(&r, None, [true; 3], "")), 3);
    assert_eq!(
        items(rows_of(&r, Some(Category::Textures), [true; 3], "")),
        2
    );
    assert_eq!(items(rows_of(&r, None, [true, true, false], "")), 2);
    assert_eq!(items(rows_of(&r, None, [true; 3], "ROOF")), 1);
}
