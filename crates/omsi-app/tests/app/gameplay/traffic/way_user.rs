use glam::DVec3;
use omsi_sim::traffic::{AiState, LaneKind, Network};
use omsi_sim::traffic::{Crossing, LaneBuilder};
use openomsi_game::testing::traffic::*;

fn street(start: DVec3, heading: f64, length: f64, radius: f64) -> omsi_sim::traffic::Lane {
    LaneBuilder::arc(start, heading, length, radius, 0.0, LaneKind::Street, 3.0)
}

/// A road north (lane 0) forking into straight on (1) and a left turn (2).
fn fork() -> Network {
    let a = street(DVec3::ZERO, 0.0, 50.0, 0.0);
    let b = street(a.end(), 0.0, 20.0, 0.0);
    let mut c = street(a.end(), 0.0, 15.7, -10.0);
    c.turn = 1;
    let mut net = Network {
        lanes: vec![a, b, c],
        ..Default::default()
    };
    net.link(1.5);
    net.build_grid();
    net
}

#[test]
fn the_players_bus_is_put_onto_the_lanes_it_may_take() {
    let net = fork();
    let bus: PlayerBox = (DVec3::new(0.0, 20.0, 0.0), 0.0, 6.0, 1.25, 10.0);
    let u = way_user_on(&net, &bus, 0, 0.0, false).expect("on the road");
    let lanes: Vec<usize> = u.lanes.iter().map(|l| l.0).collect();
    assert_eq!(lanes[0], 0);
    assert!((u.lanes[0].1 + 20.0).abs() < 0.1, "{:?}", u.lanes);
    // no indicator: either way (the cars cannot know)
    assert!(lanes.contains(&1) && lanes.contains(&2), "{lanes:?}");
    assert!(
        u.lanes
            .iter()
            .filter(|l| l.0 != 0)
            .all(|l| (l.1 - 30.0).abs() < 0.1),
        "{:?}",
        u.lanes
    );
    // indicating left: the left turn only; right (no such branch): both
    let left: Vec<usize> = way_user_on(&net, &bus, 1, 0.0, false)
        .unwrap()
        .lanes
        .iter()
        .map(|l| l.0)
        .collect();
    assert_eq!(left, vec![0, 2]);
    assert_eq!(
        way_user_on(&net, &bus, 2, 0.0, false).unwrap().lanes.len(),
        3
    );
    // reversing, or off the road: not on the lanes
    assert!(way_user_on(&net, &(bus.0, 0.0, 6.0, 1.25, -2.0), 0, 0.0, false).is_none());
    assert!(way_user_on(
        &net,
        &(DVec3::new(30.0, 20.0, 0.0), 0.0, 6.0, 1.25, 5.0),
        0,
        0.0,
        false
    )
    .is_none());
}

fn user(lanes: Vec<(usize, f32)>, speed: f32, still: f32) -> WayUser {
    WayUser {
        lanes,
        speed,
        half_len: 6.0,
        still,
        prio: false,
    }
}

/// A side road's car 15 m before the meeting place with the main road the bus drives on.
fn side_road_car(speed: f32) -> AiState {
    let mut st = AiState::new(0, 0.0, 1);
    st.speed = speed;
    st.accept_gap = 5.0;
    st
}

#[test]
fn a_car_on_the_side_road_gives_way_to_the_players_bus() {
    let c = Crossing {
        other: 1,
        at: 2.0,
        other_at: 2.0,
        merge: false,
        before: 1.5,
        after: 1.5,
        other_before: 1.5,
        other_after: 1.5,
    };
    let st = side_road_car(5.0);
    let point = 17.5; // the car's origin to the meeting point
                      // the bus 50 m off on the main road at 11 m/s (there in about four seconds)
    let coming = user(vec![(7, -10.0), (1, 40.0)], 11.0, 0.0);
    assert_eq!(
        way_user_verdict(&st, &coming, 40.0, &c, point, false, false, false, true, 1.0).0,
        Verdict::Ruled
    );
    // the car has the right of way, or has claimed the junction already: it goes
    assert_eq!(
        way_user_verdict(&st, &coming, 40.0, &c, point, false, false, false, false, 1.0).0,
        Verdict::Free
    );
    assert_eq!(
        way_user_verdict(&st, &coming, 40.0, &c, point, false, true, false, true, 1.0).0,
        Verdict::Free
    );
    // far off (a gap any driver takes), or standing at a stop: it goes
    let far = user(vec![(1, 140.0)], 11.0, 0.0);
    assert_eq!(
        way_user_verdict(&st, &far, 140.0, &c, point, false, false, false, true, 1.0).0,
        Verdict::Free
    );
    let standing = user(vec![(1, 20.0)], 0.0, 30.0);
    assert_eq!(
        way_user_verdict(&st, &standing, 20.0, &c, point, false, false, false, true, 1.0).0,
        Verdict::Free
    );
    // in the meeting place: whoever has the right of way, the car waits
    let there = user(vec![(1, -6.0)], 0.0, 30.0);
    assert_eq!(
        way_user_verdict(&st, &there, -6.0, &c, point, false, false, false, false, 1.0).0,
        Verdict::Hard
    );
    // in the junction and on its way through, there first: the car waits
    let crossing = user(vec![(1, -1.0)], 8.0, 0.0);
    assert_eq!(
        way_user_verdict(&st, &crossing, -1.0, &c, 4.0, false, false, false, false, 1.0).0,
        Verdict::Hard
    );
    // through already
    let through = user(vec![(1, -20.0)], 8.0, 0.0);
    assert_eq!(
        way_user_verdict(&st, &through, -20.0, &c, point, false, false, false, true, 1.0).0,
        Verdict::Free
    );
}

/// Two lanes running into one (lane 0 from the south, lane 1 slanting in from the
/// south-east) and on as lane 2.
fn joint() -> Network {
    let main = LaneBuilder::polyline(
        vec![DVec3::ZERO, DVec3::new(0.0, 50.0, 0.0)],
        LaneKind::Street,
        3.0,
    );
    let side = LaneBuilder::polyline(
        vec![DVec3::new(30.0, 20.0, 0.0), DVec3::new(0.0, 50.0, 0.0)],
        LaneKind::Street,
        3.0,
    );
    let on = LaneBuilder::polyline(
        vec![DVec3::new(0.0, 50.0, 0.0), DVec3::new(0.0, 120.0, 0.0)],
        LaneKind::Street,
        3.0,
    );
    let mut net = Network {
        lanes: vec![main, side, on],
        ..Default::default()
    };
    net.link(1.5);
    net.build_grid();
    net
}

#[test]
fn a_car_merging_in_keeps_behind_the_players_bus() {
    let net = joint();
    let side_len = net.lanes[1].length();
    // the car 20 m before the joint at 8 m/s
    let mut me = AiState::new(1, side_len - 20.0, 1);
    me.speed = 8.0;
    me.planned_next = Some(2);
    // the bus 20 m before the joint at 12 m/s, its way on through it: there first
    let bus = user(vec![(0, -30.0), (2, 20.0)], 12.0, 0.0);
    let l = merging_lead(&net, &me, &[bus]).expect("the bus goes first");
    assert!(l.gap > 0.0 && l.gap < 20.0, "{l:?}");
    // the bus far back, or standing: the car goes
    assert!(merging_lead(&net, &me, &[user(vec![(0, 0.0), (2, 50.0)], 12.0, 0.0)]).is_none());
    assert!(merging_lead(&net, &me, &[user(vec![(0, -30.0), (2, 20.0)], 0.0, 9.0)]).is_none());
    // a bus whose way does not go on into the lane the car takes: nothing to do with it
    assert!(merging_lead(&net, &me, &[user(vec![(0, -30.0)], 12.0, 0.0)]).is_none());
}
