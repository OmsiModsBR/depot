#[test]
fn a_full_beam_reaches_further_than_the_low_beam() {
    // the stock buses: low beam 100, full beam 500 (#941)
    assert_eq!(
        openomsi_game::testing::lights::spot_reach(100.0, 45.0),
        45.0
    );
    assert_eq!(
        openomsi_game::testing::lights::spot_reach(500.0, 45.0),
        225.0
    );
    assert_eq!(
        openomsi_game::testing::lights::spot_reach(500.0, 60.0),
        300.0
    );
    // short ranges as declared, as before
    assert_eq!(openomsi_game::testing::lights::spot_reach(30.0, 45.0), 30.0);
    assert_eq!(openomsi_game::testing::lights::spot_reach(2.0, 45.0), 2.0);
    assert_eq!(openomsi_game::testing::lights::spot_reach(0.1, 45.0), 0.5);
    assert_eq!(
        openomsi_game::testing::lights::spot_reach(5000.0, 45.0),
        225.0
    );
}

/// The enhanced picture lights a `[spotlight]` as a low beam below the full beam's range
/// (the stock 100, the Grand Paris-Moulon Citaro's 200) and as a full beam from it on.
#[test]
fn a_spotlight_is_a_low_or_a_full_beam_by_its_range() {
    let beam = |range: f32| {
        let vals = [
            0.0, 6.5, 0.76, 0.0, 1.0, -0.3, 255.0, 255.0, 233.0, range, 30.0, 80.0,
        ];
        let mut lights = Vec::new();
        openomsi_game::testing::lights::push_spot(
            &mut lights,
            glam::DVec3::ZERO,
            glam::Vec3::Y,
            &vals,
            0.5,
            1.0,
        );
        lights
            .iter()
            .find(|l| l.mode == omsi_render::LightMode::Enhanced)
            .map(|l| l.beam)
    };
    assert_eq!(
        [100.0, 200.0, 450.0, 500.0].map(beam),
        [Some(1.0), Some(1.0), Some(-1.0), Some(-1.0)]
    );
    // a lamp over a door, pointing down, keeps its cone
    let vals = [
        0.0, 6.5, 2.5, 0.0, 0.0, -1.0, 255.0, 255.0, 233.0, 100.0, 30.0, 80.0,
    ];
    let mut lights = Vec::new();
    openomsi_game::testing::lights::push_spot(
        &mut lights,
        glam::DVec3::ZERO,
        glam::Vec3::NEG_Z,
        &vals,
        0.5,
        1.0,
    );
    assert_eq!(
        lights
            .iter()
            .find(|l| l.mode == omsi_render::LightMode::Enhanced)
            .map(|l| l.beam),
        Some(0.0)
    );
}

/// `[spotlight_2]`: a pair mirrored across the axis sharing the light, or one lamp, as
/// bright as its variable says, and nothing while that is off.
#[test]
fn a_spotlight_2_is_a_mirrored_pair_or_one_lamp() {
    use glam::Vec3;
    let mut sp = omsi_model::Spotlight2 {
        values: [
            0.9, 5.9, 0.65, 0.2, 1.0, -0.3, 255.0, 255.0, 233.0, 200.0, 30.0, 80.0,
        ],
        variable: "lights_fern".into(),
        mirrored: true,
    };
    let pair = openomsi_game::testing::lights::spotlight_2_lamps(&sp, 1.0);
    assert_eq!(
        pair,
        [
            (Vec3::new(0.9, 5.9, 0.65), Vec3::new(0.2, 1.0, -0.3), 0.5),
            (Vec3::new(-0.9, 5.9, 0.65), Vec3::new(-0.2, 1.0, -0.3), 0.5),
        ]
    );
    assert!(openomsi_game::testing::lights::spotlight_2_lamps(&sp, 0.0).is_empty());
    assert!(openomsi_game::testing::lights::spotlight_2_lamps(&sp, -1.0).is_empty());
    sp.mirrored = false;
    assert_eq!(
        openomsi_game::testing::lights::spotlight_2_lamps(&sp, 0.5),
        [(Vec3::new(0.9, 5.9, 0.65), Vec3::new(0.2, 1.0, -0.3), 0.5)]
    );
    assert_eq!(
        openomsi_game::testing::lights::spotlight_2_lamps(&sp, 3.0)[0].2,
        1.0
    );
}
