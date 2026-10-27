use openomsi_game::testing::api_limits::*;

#[test]
fn each_option_is_in_the_table_once_with_a_reason() {
    for (i, l) in D3D9.iter().enumerate() {
        assert!(!l.reason.is_empty(), "{} has no reason", l.key);
        assert!(
            D3D9[i + 1..].iter().all(|o| o.key != l.key),
            "{} is listed twice",
            l.key
        );
        assert_eq!(l.text().is_some(), l.shown().is_some());
        assert!(!l.is_row() || l.text().is_none());
    }
    assert!(d3d9("windy_trees")
        .and_then(|l| l.text())
        .is_some_and(|t| t.starts_with("Off - ")));
    // (anti-aliasing and anisotropic filtering are OMSI 2's own options: drawn, not limited)
    assert!(d3d9("msaa").is_none() && d3d9("anisotropy").is_none());
    assert!(d3d9("ssao").is_some_and(|l| l.text().is_none()));
    assert!(d3d9("view_distance").is_none());
    let sizes = d3d9("mirror_size").unwrap();
    assert!(sizes.is_row() && sizes.allows("256") && !sizes.allows("1024"));
    let refresh = d3d9("mirror_refresh").unwrap();
    assert!(refresh.allows("full") && !refresh.allows("eco"));
    assert_eq!(sizes.fit("64"), "128");
    assert_eq!(sizes.fit("1024"), "512");
    assert_eq!(sizes.fit("0"), "0");
    assert_eq!(refresh.fit("eco"), "full");
}
