use omsi_net::{Aboard, Pose, Walker, FLAG_VEHICLE};
use openomsi_game::testing::server::player_info;

fn bus(id: u32, x: f64, y: f64) -> Pose {
    Pose {
        id,
        name: format!("p{id}"),
        bus: "Vehicles/MAN_SD200/MAN_SD77.bus".into(),
        line: "37".into(),
        x,
        y,
        heading: 90.0,
        speed_kmh: 40.0,
        flags: FLAG_VEHICLE,
        ..Default::default()
    }
}

#[test]
fn a_player_is_where_it_is() {
    let none = |_| None;
    // driving: the bus
    let d = player_info(&bus(2, 100.0, 200.0), none).unwrap();
    assert_eq!(
        (d.x, d.y, d.speed_kmh, d.on_foot, d.aboard, d.line.as_str()),
        (100.0, 200.0, 40.0, false, None, "37")
    );
    // on foot: the walker, not the zeroed vehicle fields
    let mut w = Pose {
        id: 3,
        name: "p3".into(),
        bus: "Vehicles/MAN_SD200/MAN_SD77.bus".into(),
        ..Default::default()
    };
    w.walker = Some(Walker {
        x: 10.0,
        y: 20.0,
        heading: 45.0,
        speed: 1.5,
        ..Default::default()
    });
    let f = player_info(&w, none).unwrap();
    assert_eq!(
        (f.x, f.y, f.heading, f.on_foot, f.bus.as_str()),
        (10.0, 20.0, 45.0, true, "")
    );
    assert!((f.speed_kmh - 5.4).abs() < 1e-4);
    // aboard player 2's bus: placed with it
    let driver = bus(2, 100.0, 200.0);
    w.walker = Some(Walker {
        x: 90.0,
        y: 190.0,
        aboard: Some(Aboard {
            owner: 2,
            ..Default::default()
        }),
        ..Default::default()
    });
    let a = player_info(&w, |id| (id == 2).then_some(&driver)).unwrap();
    assert_eq!(
        (a.x, a.y, a.speed_kmh, a.on_foot, a.aboard),
        (100.0, 200.0, 40.0, true, Some(2))
    );
    // aboard a bus that is gone: its own point
    let g = player_info(&w, none).unwrap();
    assert_eq!((g.x, g.y, g.aboard), (90.0, 190.0, None));
    // got out of its own bus: the state keeps FLAG_VEHICLE and the parked bus's place, and
    // has the walker - the walker wins
    let mut out = bus(5, 100.0, 200.0);
    out.walker = Some(Walker {
        x: 104.0,
        y: 197.0,
        heading: 180.0,
        speed: 1.0,
        ..Default::default()
    });
    assert!(out.has_vehicle());
    let o = player_info(&out, none).unwrap();
    assert_eq!(
        (
            o.x,
            o.y,
            o.heading,
            o.on_foot,
            o.aboard,
            o.bus.as_str(),
            o.line.as_str()
        ),
        (104.0, 197.0, 180.0, true, None, "", "")
    );
    assert!((o.speed_kmh - 3.6).abs() < 1e-4);
    // and sitting in another player's bus after getting out of its own: with that bus
    out.walker = Some(Walker {
        x: 1.0,
        y: 1.0,
        aboard: Some(Aboard {
            owner: 2,
            ..Default::default()
        }),
        ..Default::default()
    });
    let oa = player_info(&out, |id| (id == 2).then_some(&driver)).unwrap();
    assert_eq!(
        (oa.x, oa.y, oa.on_foot, oa.aboard),
        (100.0, 200.0, true, Some(2))
    );
    // neither a bus nor a walker: not listed
    assert!(player_info(
        &Pose {
            id: 4,
            ..Default::default()
        },
        none
    )
    .is_none());
}
