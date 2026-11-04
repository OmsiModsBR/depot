use openomsi_game::testing::humans::*;

/// A stop at either end of its pavement path leads somewhere, as one in the middle does.
#[test]
fn a_walk_from_a_stop_at_either_end_of_its_lane_goes_somewhere() {
    for (at, pick) in [
        (0.0, 0),
        (0.0, 1),
        (0.02, 0),
        (30.0, 0),
        (30.0, 1),
        (29.98, 1),
        (12.0, 0),
        (12.0, 1),
    ] {
        let next = pavement_continuation(
            Leg {
                lane: 3,
                a: at,
                b: at,
            },
            30.0,
            pick,
        );
        assert_eq!((next.lane, next.a), (3, at));
        assert!(
            next.len() > 1.0,
            "from {at} (pick {pick}) the leg is {} m",
            next.len()
        );
    }
    // from the start to the far end, from the far end to the start
    assert_eq!(
        pavement_continuation(
            Leg {
                lane: 0,
                a: 0.0,
                b: 0.0
            },
            30.0,
            0
        )
        .b,
        30.0
    );
    assert_eq!(
        pavement_continuation(
            Leg {
                lane: 0,
                a: 30.0,
                b: 30.0
            },
            30.0,
            0
        )
        .b,
        0.0
    );
    // a walk that arrived at an end turns round, one in the middle keeps its direction
    assert_eq!(
        pavement_continuation(
            Leg {
                lane: 0,
                a: 4.0,
                b: 30.0
            },
            30.0,
            0
        )
        .b,
        0.0
    );
    assert_eq!(
        pavement_continuation(
            Leg {
                lane: 0,
                a: 4.0,
                b: 10.0
            },
            30.0,
            0
        )
        .b,
        30.0
    );
}

/// Somebody standing looks for a pavement once a second, not every frame.
#[test]
fn somebody_standing_looks_for_a_pavement_once_a_second() {
    let dt = 1.0 / 60.0;
    let looks = (0..600)
        .filter(|k| look_for_pavement(*k as f32 * dt, dt))
        .count();
    assert!((9..=11).contains(&looks), "{looks} looks in 10 s");
}
