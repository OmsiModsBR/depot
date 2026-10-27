use openomsi_game::testing::lan_world::{relative_to_roots, PlayClock};
use std::path::{Path, PathBuf};

#[test]
fn a_file_is_named_relative_to_the_deepest_root_holding_it() {
    // the content folder first, the OMSI 2 folder inside it after (a server's layout)
    let roots = [
        PathBuf::from("/srv/depot"),
        PathBuf::from("/srv/depot/OMSI 2"),
    ];
    let golf = Path::new("/srv/depot/OMSI 2/Vehicles/VW_Golf_2/ai_vw_golf_2.bus");
    assert_eq!(
        relative_to_roots(golf, &roots).as_deref(),
        Some("Vehicles/VW_Golf_2/ai_vw_golf_2.bus")
    );
    // a mod in the content folder itself stays relative to that
    let m = Path::new("/srv/depot/Vehicles/Mod/mod.bus");
    assert_eq!(
        relative_to_roots(m, &roots).as_deref(),
        Some("Vehicles/Mod/mod.bus")
    );
    assert_eq!(
        relative_to_roots(Path::new("/elsewhere/x.bus"), &roots),
        None
    );
}

/// The host's bus to U Ruhleben (row 2) shows U Ruhleben on ours too, not Machandelweg
/// (row 1), whose sign has RUHLEBEN on its second line.
#[test]
fn a_hosts_timetable_bus_shows_the_row_it_was_given() {
    let t = |code: i32, id: &str, s: &[&str]| omsi_vehicle::hof::Terminus {
        code,
        texture_id: id.into(),
        strings: s.iter().map(|x| x.to_string()).collect(),
        ..Default::default()
    };
    let hof = omsi_vehicle::Hof {
        termini: vec![
            t(0, "Empty", &[""]),
            t(194, "Machandelweg", &["MACHANDELWEG", "RUHLEBEN"]),
            t(282, "U Ruhleben", &["RUHLEBEN", "U-BAHNHOF"]),
        ],
        ..Default::default()
    };
    let mut v = openomsi_game::testing::script_test_vehicle(
        "{frame}\n{end}\n",
        "AI_target_index\nIBIS_TerminusIndex\nIBIS_TerminusCode\n",
        "SetLineTo\n",
    );
    openomsi_game::testing::lan_world::show_car_destination(&mut v, Some(&hof), "5", "#2");
    assert_eq!(
        (v.var("AI_target_index"), v.var("IBIS_TerminusCode")),
        (Some(2.0), Some(282.0))
    );
    assert_eq!(v.str_var("SetLineTo"), "5");
    // no such row, or no depot file: nothing changes
    openomsi_game::testing::lan_world::show_car_destination(&mut v, Some(&hof), "5", "#7");
    openomsi_game::testing::lan_world::show_car_destination(&mut v, None, "5", "#1");
    assert_eq!(v.var("AI_target_index"), Some(2.0));
}

#[test]
fn the_moment_drawn_follows_a_changed_delay_smoothly() {
    let mut c = PlayClock::default();
    let mut now = 0.0;
    let mut t = c.step(now, now - 120.0, 500.0);
    // a slow frame of theirs: the delay grows by 70 ms at once
    let mut steps = Vec::new();
    for _ in 0..100 {
        now += 16.0;
        let n = c.step(now, now - 190.0, 500.0);
        steps.push(n - t);
        t = n;
    }
    // never back, never on more than a few per cent faster or slower than the clock
    assert!(
        steps.iter().all(|d| *d > 16.0 * 0.93 && *d < 16.0 * 1.07),
        "{steps:?}"
    );
    assert!(
        (t - (now - 190.0)).abs() < 1e-6,
        "caught up: {}",
        t - (now - 190.0)
    );
    // a new session: taken at once
    now += 16.0;
    assert_eq!(c.step(now, now + 5000.0, 500.0), now + 5000.0);
}
