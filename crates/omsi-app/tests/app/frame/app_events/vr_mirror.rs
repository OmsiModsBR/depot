use openomsi_game::testing::app_events::vr_mirror_updates;

#[test]
fn every_frame_updates_all_mirrors_even_at_low_game_fps() {
    let mut budget = 0.75;
    for dt in [1.0 / 90.0, 1.0 / 30.0, 0.5] {
        assert_eq!(vr_mirror_updates(&mut budget, dt, -1.0, 8), 8);
        assert_eq!(budget, 0.0);
    }
}

#[test]
fn a_high_budget_is_not_limited_to_two_mirrors_per_frame() {
    let mut budget = 0.0;
    assert_eq!(vr_mirror_updates(&mut budget, 1.0 / 60.0, 240.0, 4), 4);
    assert_eq!(vr_mirror_updates(&mut budget, 0.5, 360.0, 4), 4);
    assert!(budget <= 0.5);
}

#[test]
fn fractional_credit_preserves_the_selected_total_rate() {
    for fps in [30, 60, 90] {
        let mut budget = 0.0;
        let updates: usize = (0..fps * 10)
            .map(|_| vr_mirror_updates(&mut budget, 1.0 / fps as f32, 16.0, 4))
            .sum();
        assert!((159..=160).contains(&updates), "fps={fps}: {updates}");
    }
}

#[test]
fn off_and_no_mirrors_discard_old_credit() {
    let mut budget = 2.5;
    assert_eq!(vr_mirror_updates(&mut budget, 0.1, 0.0, 4), 0);
    assert_eq!(budget, 0.0);
    assert_eq!(vr_mirror_updates(&mut budget, 0.1, -1.0, 0), 0);
    assert_eq!(vr_mirror_updates(&mut budget, 0.1, 360.0, 0), 0);
    assert_eq!(budget, 0.0);
}
