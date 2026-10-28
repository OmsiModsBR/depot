use omsi_timetable::Tour;
use omsi_timetable::TourTrip;
use openomsi_game::testing::launcher::timetable::*;

#[test]
fn a_tour_every_twenty_minutes_until_eight() {
    let base = Tour {
        number: "1".into(),
        ai_group: "Busses".into(),
        extra: String::new(),
        trips: vec![
            TourTrip {
                trip: "a".into(),
                profile: 0,
                departure: 6.0 * 60.0,
            },
            TourTrip {
                trip: "b".into(),
                profile: 0,
                departure: 6.5 * 60.0,
            },
        ],
    };
    let mut tours = vec![base.clone()];
    let made = repeat_tour(&mut tours, &base, 20.0, 8.0 * 60.0);
    // 6:20, 6:40 … 8:00
    assert_eq!(made, 6);
    assert_eq!(tours.last().unwrap().trips[0].departure, 8.0 * 60.0);
    assert_eq!(tours.last().unwrap().trips[1].departure, 8.5 * 60.0);
    assert_eq!(tours.last().unwrap().number, "7");
}
