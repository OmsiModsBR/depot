use openomsi_game::testing::launcher::ai_traffic::*;

#[test]
fn chart_steps_and_numbers_read_well() {
    assert_eq!(nice_step(0.3), 0.5);
    assert_eq!(nice_step(4.0), 5.0);
    assert_eq!(nice_step(1.5), 2.0);
    assert_eq!(trim_num(1.0), "1");
    assert_eq!(trim_num(0.25), "0.25");
    assert_eq!(trim_num(0.5), "0.5");
    assert_eq!(weight_text(4.0), "4");
    assert_eq!(weight_text(0.5), "0.5");
    assert_eq!(shown_name("", "Vehicles\\Car\\a.ovh"), "a.ovh");
    assert_eq!(shown_name("Maker Sedan", "x"), "Maker Sedan");
}
