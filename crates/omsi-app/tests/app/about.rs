use openomsi_game::testing::about::*;

#[test]
fn only_the_projects_own_pages_are_opened() {
    assert!(is_depot_url(REPO_URL));
    assert!(is_depot_url(RELEASES_URL));
    assert!(is_depot_url(
        "https://github.com/OmsiModsBR/depot/issues/new?title=a&body=b"
    ));
    assert!(is_depot_url("https://github.com/OmsiModsBR/depot#readme"));
    assert!(!is_depot_url("http://github.com/OmsiModsBR/depot"));
    assert!(!is_depot_url("https://github.com/OmsiModsBR/depot-evil"));
    assert!(!is_depot_url(
        "https://github.com/OmsiModsBR/depotx/releases"
    ));
    assert!(!is_depot_url(
        "https://github.com.evil.example/OmsiModsBR/depot"
    ));
    assert!(!is_depot_url(
        "https://github.com@evil.example/OmsiModsBR/depot"
    ));
    assert!(!is_depot_url("file:///etc/passwd"));
    assert!(!is_depot_url("javascript:alert(1)"));
    assert!(!is_depot_url(""));
}

#[test]
fn the_release_link_is_inside_the_repository() {
    assert!(RELEASES_URL.starts_with(REPO_URL));
    assert!(!current_version().is_empty());
}
