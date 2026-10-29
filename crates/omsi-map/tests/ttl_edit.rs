use omsi_map::ttl_edit::{TtlDoc, TtlEditError};
use omsi_timetable::Line;

const SAMPLE: &str = "-----\r\nTime Table Line File\r\n-----\r\n\r\n[userallowed]\r\n\r\n[priority]\r\n1\r\n\
    ------\r\n[newtour]\r\n1\r\nBusse\r\n\r\n------\r\n\r\n  Dep.: 5:12:0\r\n[addtrip]\r\nA\r\n0\r\n312.000\r\n\r\n\
    [newtour]\r\n2\r\nBusse_Gelenk\r\n0\r\n[addtrip]\r\nB\r\n0\r\n400.000\r\n";

fn groups_of(bytes: &[u8]) -> Vec<String> {
    let f = omsi_cfg::CfgFile::from_bytes("x.ttl", bytes);
    Line::parse(&f)
        .tours
        .iter()
        .map(|t| t.ai_group.trim().to_string())
        .collect()
}

#[test]
fn a_tour_changes_its_group_line_only() {
    let mut d = TtlDoc::from_bytes(SAMPLE.as_bytes());
    assert_eq!(d.to_bytes(), SAMPLE.as_bytes());
    assert_eq!(d.tours()[1].ai_group, "Busse_Gelenk");
    d.set_group(1, "Busse").unwrap();
    assert_eq!(
        String::from_utf8(d.to_bytes()).unwrap(),
        SAMPLE.replace("Busse_Gelenk", "Busse")
    );
    // the timetable reads the same groups
    assert_eq!(groups_of(&d.to_bytes()), ["Busse", "Busse"]);
}

#[test]
fn refused_changes_leave_the_file() {
    let mut d = TtlDoc::from_bytes(SAMPLE.as_bytes());
    assert_eq!(d.set_group(5, "x"), Err(TtlEditError::NoTour));
    assert_eq!(d.set_group(0, " "), Err(TtlEditError::BadGroup));
    assert_eq!(d.set_group(0, "a\tb"), Err(TtlEditError::BadGroup));
    assert_eq!(d.to_bytes(), SAMPLE.as_bytes());
    let mut cut = TtlDoc::from_bytes(b"[newtour]\r\n7");
    assert_eq!(cut.set_group(0, "x"), Err(TtlEditError::Truncated));
}

#[test]
fn tour_numbers_are_those_of_the_timetables_own_reading() {
    // a keyword in upper case is not one of the timetable's: it is no tour here either
    let text = "[NEWTOUR]\r\n9\r\nIgnored\r\n0\r\n[newtour]\r\n1\r\nBusse\r\n0\r\n";
    let mut d = TtlDoc::from_bytes(text.as_bytes());
    assert_eq!(d.tours().len(), 1);
    d.set_group(0, "Other").unwrap();
    assert_eq!(groups_of(&d.to_bytes()), ["Other"]);
    assert!(String::from_utf8(d.to_bytes()).unwrap().contains("Ignored"));
}
