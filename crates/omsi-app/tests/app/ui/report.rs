use openomsi_game::testing::ui::report::*;

#[test]
fn the_report_waits_for_the_doors_at_the_last_stop() {
    // standing there with a door open: the report
    assert_eq!(watch_outcome(5.0, true, false), Some(true));
    // there with the doors shut, the duty on the next trip already: still waiting
    assert_eq!(watch_outcome(5.0, false, false), None);
    // on the way to the stand, doors shut: waiting
    assert_eq!(watch_outcome(40.0, false, false), None);
    // gone without opening: no report
    assert_eq!(watch_outcome(80.0, false, false), Some(false));
    // still on that trip's end (a terminus with a layover far off): waiting
    assert_eq!(watch_outcome(80.0, false, true), None);
}

/// A late arrival: the duty is on the next trip before the doors open, and the report
/// still opens, once.
#[test]
fn a_late_arrival_still_gets_its_report_once() {
    let key: TripKey = ("5".into(), "1".into(), 3);
    let stop = glam::DVec3::new(100.0, 0.0, 0.0);
    let near = glam::DVec3::new(95.0, 0.0, 0.0);
    let mut watch = None;
    // stopped at the last stop, doors shut: the trip is done
    assert_eq!(
        watch_step(&mut watch, None, Some((key.clone(), 2, stop)), near, false),
        None
    );
    // the duty goes on to the next trip (it is due): the doors open
    assert_eq!(watch_step(&mut watch, None, None, near, false), None);
    assert_eq!(
        watch_step(&mut watch, None, None, near, true),
        Some((key.clone(), 2))
    );
    assert!(watch.is_none());
    // a trip whose report has opened is not watched again
    assert_eq!(
        watch_step(
            &mut watch,
            Some(&key),
            Some((key.clone(), 2, stop)),
            near,
            true
        ),
        None
    );
    assert!(watch.is_none());
    // driven off without opening: no report later
    let next: TripKey = ("5".into(), "1".into(), 4);
    assert_eq!(
        watch_step(
            &mut watch,
            Some(&key),
            Some((next.clone(), 3, stop)),
            near,
            false
        ),
        None
    );
    assert_eq!(
        watch_step(
            &mut watch,
            Some(&key),
            None,
            glam::DVec3::new(200.0, 0.0, 0.0),
            false
        ),
        None
    );
    assert!(watch.is_none());
    assert_eq!(watch_step(&mut watch, Some(&key), None, near, true), None);
}
