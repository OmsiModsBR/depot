use omsi_sim::testing::traffic::TrafficLightController;

#[test]
fn remaining_green_of_a_pedestrian_light() {
    let at = |mut c: TrafficLightController, t: f64| {
        c.time = t;
        c
    };
    // Einm_Stresow_Obermeier "Main_Ped": green 8 s, then red to the end of a 38 s cycle
    let ped = TrafficLightController::new(vec![vec![(6, 8.0), (0, 0.0)]], 38.0);
    let c = at(ped.clone(), 3.0);
    assert_eq!(c.state(0), 6);
    assert!((c.remaining(0) - 5.0).abs() < 1e-4);
    let c = at(ped, 20.0);
    assert_eq!(c.state(0), 0);
    assert!((c.remaining(0) - 18.0).abs() < 1e-4);
    // red running over the end of the cycle into a red start
    let d = at(
        TrafficLightController::new(
            vec![vec![(0, 19.0), (3, 2.0), (6, 11.0), (9, 3.0), (0, 0.0)]],
            38.0,
        ),
        36.0,
    );
    assert!((d.remaining(0) - 21.0).abs() < 1e-4, "{}", d.remaining(0));
    assert!(
        at(TrafficLightController::new(vec![vec![(6, 5.0)]], 0.0), 1.0)
            .remaining(0)
            .is_infinite()
    );
}
