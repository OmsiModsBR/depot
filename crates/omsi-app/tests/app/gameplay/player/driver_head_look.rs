use openomsi_game::testing::player::driver_head_look;

#[test]
fn head_pitch_adjusts_only_non_vr_driver_view() {
    assert_eq!(
        driver_head_look((4.0, 2.0), "driver", 10.0, false),
        (4.0, 12.0)
    );
    assert_eq!(
        driver_head_look((4.0, 2.0), "driver", 10.0, true),
        (4.0, 2.0)
    );
    assert_eq!(driver_head_look((4.0, 2.0), "pax", 10.0, false), (4.0, 2.0));
    assert_eq!(
        driver_head_look((4.0, 2.0), "outside", 10.0, false),
        (4.0, 2.0)
    );
}
