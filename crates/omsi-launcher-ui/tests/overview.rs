//! The Diagnostics page's Overview: the report link.

#[test]
fn a_report_body_is_encoded_for_the_issue_link() {
    assert_eq!(
        omsi_launcher_ui::window::url_encode("a b\n(c)"),
        "a%20b%0A%28c%29"
    );
    assert_eq!(
        omsi_launcher_ui::window::url_encode("Depot-0.2_x.~"),
        "Depot-0.2_x.~"
    );
}
