use openomsi_game::testing::screenshot_name::*;

#[test]
fn only_screenshot_notices_are_recognised() {
    let (text, _) = notice(std::path::Path::new("a.png"));
    assert!(is_notice(&text));
    assert!(is_notice("Screenshot could not be saved: disk full"));
    assert!(!is_notice("Mouse steering off"));
}

#[test]
fn the_notice_names_the_file_and_lasts_a_few_seconds() {
    let (text, secs) = notice(std::path::Path::new("Screenshots/a.png"));
    assert_eq!(text, "Screenshot: Screenshots/a.png");
    assert!(secs > 0.0);
}

#[test]
fn map_and_vehicle_follow_the_time() {
    assert_eq!(
        build(1760000000, "Grundorf", Some("MB O305")),
        "omsi_1760000000__Grundorf__MB_O305.png"
    );
}

#[test]
fn on_foot_or_free_camera_names_only_the_map() {
    assert_eq!(build(5, "Berlin", None), "omsi_5__Berlin.png");
}

#[test]
fn an_unusable_part_is_left_out() {
    assert_eq!(build(5, "???", Some("Bus")), "omsi_5__Bus.png");
    assert_eq!(build(5, "", None), "omsi_5.png");
}

#[test]
fn unsafe_characters_never_reach_the_file_name() {
    let n = build(1, "a/b\\c:d*e?\"f<g>h|i", Some("../../x\u{7}y"));
    assert!(
        !n.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|', '\u{7}']),
        "{n}"
    );
    assert_eq!(sanitize_part("..hidden. "), "hidden");
    assert_eq!(sanitize_part("a   b__c"), "a_b_c");
}

#[test]
fn windows_device_names_are_defused() {
    assert_eq!(sanitize_part("CON"), "CON_");
    assert_eq!(sanitize_part("lpt1"), "lpt1_");
    assert_eq!(sanitize_part("Console"), "Console");
}

#[test]
fn long_parts_are_cut_by_characters_not_bytes() {
    let long = "é".repeat(200);
    assert_eq!(sanitize_part(&long).chars().count(), MAX_PART);
    assert!(build(1, &long, Some(&long)).len() < 255);
}

#[test]
fn what_is_built_is_read_back() {
    let n = build(42, "Grundorf", Some("MB O305"));
    assert_eq!(
        parse(&n),
        Some((42, Some("Grundorf".into()), Some("MB_O305".into())))
    );
    assert_eq!(
        parse("omsi_42__Grundorf.png"),
        Some((42, Some("Grundorf".into()), None))
    );
}

#[test]
fn old_captures_are_still_read() {
    assert_eq!(parse("omsi_1760000000.png"), Some((1760000000, None, None)));
    assert_eq!(parse("holiday.png"), None);
    assert_eq!(parse("omsi_x.png"), None);
}

#[test]
fn folder_names_come_from_content_paths() {
    assert_eq!(
        folder_of("maps/Grundorf/Global.cfg").as_deref(),
        Some("Grundorf")
    );
    assert_eq!(
        folder_of("Vehicles\\MB_O305\\MB_O305.bus").as_deref(),
        Some("MB_O305")
    );
    assert_eq!(folder_of("single.cfg").as_deref(), Some("single.cfg"));
}
