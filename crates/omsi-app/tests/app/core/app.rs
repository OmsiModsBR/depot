use openomsi_game::testing::core::app::CamBlend;

fn blend(t: f32) -> f32 {
    CamBlend {
        key: None,
        from: None,
        shown: None,
        entering: false,
        t,
        carry: None,
    }
    .progress()
}

#[test]
fn glide_starts_fast_and_settles_softly() {
    assert_eq!(blend(0.0), 0.0);
    assert_eq!(blend(1.0), 1.0);
    assert!((blend(0.5) - 0.875).abs() < 1e-6);
    assert!(blend(0.2) > 0.4, "fast off the mark");
}
