use openomsi_game::testing::player::kept_indicator;

#[test]
fn a_kept_indicator_is_put_back_when_the_script_cancels_it() {
    // the script's own cancelling stands by default
    assert_eq!(kept_indicator(true, Some(1.0), Some(0.0)), None);
    // kept: left and right come back, the hazard lever and a change of side stay
    assert_eq!(kept_indicator(false, Some(1.0), Some(0.0)), Some(1.0));
    assert_eq!(kept_indicator(false, Some(2.0), Some(0.0)), Some(2.0));
    assert_eq!(kept_indicator(false, Some(1.0), Some(2.0)), None);
    assert_eq!(kept_indicator(false, Some(0.0), Some(0.0)), None);
    assert_eq!(kept_indicator(false, None, Some(0.0)), None);
}
