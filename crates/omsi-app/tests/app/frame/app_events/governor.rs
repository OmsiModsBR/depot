use openomsi_game::testing::app_events::render_scale_step;

#[test]
fn cpu_stutters_do_not_reduce_picture_quality() {
    assert!(render_scale_step(35.0, 0.1) > 0.0);
    assert!(render_scale_step(35.0, 0.6) < 0.0);
    assert!(render_scale_step(60.0, 0.6) > 0.0);
}
