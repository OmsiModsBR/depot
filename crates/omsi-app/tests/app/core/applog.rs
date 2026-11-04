use openomsi_game::testing::core::applog::profile_window;
use std::collections::BTreeMap;

/// A profile window reports what each stage cost a frame between two snapshots of the
/// stage totals, the most costly first, leaving out what barely ran.
#[test]
fn a_profile_window_lists_the_stages_costliest_first_since_its_start() {
    let before: BTreeMap<&'static str, f64> = [("traffic", 1.0), ("render", 2.0), ("hud", 0.5)]
        .into_iter()
        .collect();
    // 100 frames later: traffic +0.8 s, render +0.4 s, hud +0.000001 s, audio new with 0.2 s
    let now: BTreeMap<&'static str, f64> = [
        ("traffic", 1.8),
        ("render", 2.4),
        ("hud", 0.500001),
        ("audio", 0.2),
    ]
    .into_iter()
    .collect();
    let w = profile_window(&before, &now, 100);
    let names: Vec<&str> = w.iter().map(|(k, _)| *k).collect();
    assert_eq!(names, ["traffic", "render", "audio"]);
    assert!((w[0].1 - 8.0).abs() < 1e-9, "{w:?}");
    assert!((w[1].1 - 4.0).abs() < 1e-9, "{w:?}");
    assert!((w[2].1 - 2.0).abs() < 1e-9, "{w:?}");
}

/// A window with no frames does not divide by zero.
#[test]
fn a_profile_window_without_frames_stays_finite() {
    let now: BTreeMap<&'static str, f64> = [("traffic", 0.01)].into_iter().collect();
    let w = profile_window(&BTreeMap::new(), &now, 0);
    assert!(w.iter().all(|(_, ms)| ms.is_finite()));
}
