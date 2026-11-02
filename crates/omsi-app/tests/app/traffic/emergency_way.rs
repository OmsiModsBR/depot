//! Deliberate improvement: ordinary AI cars make way for an emergency vehicle coming up behind
//! them. They keep to the kerb side of their lane and come to a gentle stop on an open road, so
//! that the emergency vehicle can go round them; they carry on through a junction rather than
//! stop in it, they give up after a while when the emergency vehicle cannot get by, and without
//! an emergency vehicle behind them nothing changes. Over the synthetic crossroads of
//! `junction_scenarios` (one lane each way, no oncoming lane to go round by).

use super::junction_scenarios::*;

fn world() -> World {
    World::new(Layout::default(), Vec::new())
}

fn describe(w: &World, id: u64) -> String {
    let c = w.of(id);
    format!(
        "car {id} lane {} s {:.1} speed {:.2} lateral {:.2}/{:.2} stood {:.1} why {:?}",
        c.state.lane,
        c.state.s,
        c.state.speed,
        c.state.lateral,
        c.state.lateral_target,
        c.stopped,
        c.why
    )
}

/// A car 50 m ahead of an emergency vehicle on the same lane moves to the kerb side and
/// stops; the same car with an ordinary car behind it does neither.
#[test]
fn a_car_ahead_of_an_emergency_vehicle_moves_to_the_kerb_and_stops() {
    for emergency in [false, true] {
        let mut w = world();
        let a = w.x.approach[NORTH];
        let behind = w.car(a, 10.0, 14.0, 51, emergency);
        let ahead = w.car(a, 62.0, 8.0, 52, false);
        w.route(
            behind,
            vec![a, w.x.junction[NORTH], w.x.exit[NORTH], w.x.beyond[NORTH]],
        );
        w.route(
            ahead,
            vec![a, w.x.junction[NORTH], w.x.exit[NORTH], w.x.beyond[NORTH]],
        );
        let (mut stopped, mut aside) = (false, 0.0f32);
        w.run(9.0, |w| {
            let c = w.of(ahead);
            stopped |= c.state.speed < 0.3;
            aside = aside.max(c.state.lateral_target);
        });
        if emergency {
            assert!(
                aside > 0.3,
                "it did not move aside: {}",
                describe(&w, ahead)
            );
            assert!(aside <= 0.61, "it moved too far: {aside}");
            assert!(stopped, "it did not stop: {}", describe(&w, ahead));
            let gap = w.of(ahead).state.s - w.of(behind).state.s;
            assert!(gap > 2.0, "the emergency vehicle ran into it: {gap:.1} m");
        } else {
            assert_eq!(aside, 0.0, "{}", describe(&w, ahead));
            assert!(!stopped, "{}", describe(&w, ahead));
        }
    }
}

/// A car behind the emergency vehicle, or one far ahead of it, keeps to its lane and drives on.
#[test]
fn only_the_cars_in_front_of_it_make_way() {
    let mut w = world();
    let a = w.x.approach[NORTH];
    let route = vec![a, w.x.junction[NORTH], w.x.exit[NORTH], w.x.beyond[NORTH]];
    let em = w.car(a, 30.0, 12.0, 61, true);
    let behind = w.car(a, 8.0, 12.0, 62, false);
    w.route(em, route.clone());
    w.route(behind, route);
    w.run(4.0, |w| {
        let c = w.of(behind);
        assert_eq!(c.state.lateral_target, 0.0, "{}", describe(w, behind));
    });
}

/// A car close to a junction does not stop short of it for an emergency vehicle: it drives
/// through (it would stand across the junction otherwise), and does not stand in it.
#[test]
fn a_car_at_a_junction_clears_it_instead_of_stopping_for_an_emergency_vehicle() {
    let mut w = world();
    let a = w.x.approach[NORTH];
    let route = vec![a, w.x.junction[NORTH], w.x.exit[NORTH], w.x.beyond[NORTH]];
    let em = w.car(a, 30.0, 14.0, 71, true);
    let ahead = w.car(a, 86.0, 9.0, 72, false);
    w.route(em, route.clone());
    w.route(ahead, route);
    let mut stood_in_junction = false;
    w.run(15.0, |w| {
        stood_in_junction |= w.in_junction(ahead) && w.of(ahead).state.speed < 0.3;
    });
    assert!(w.through(ahead), "{}", describe(&w, ahead));
    assert!(!stood_in_junction, "it stood in the junction");
}

/// When the emergency vehicle cannot get by (nowhere to go round), the car does not stand in
/// front of it for good: it drives on after ten seconds.
#[test]
fn a_car_gives_up_making_way_when_the_emergency_vehicle_cannot_get_by() {
    let mut w = world();
    let a = w.x.approach[NORTH];
    let route = vec![a, w.x.junction[NORTH], w.x.exit[NORTH], w.x.beyond[NORTH]];
    let em = w.car(a, 5.0, 14.0, 81, true);
    let ahead = w.car(a, 50.0, 8.0, 82, false);
    w.route(em, route.clone());
    w.route(ahead, route);
    let (mut stopped_at, mut moved_after) = (None, false);
    w.run(30.0, |w| {
        // (it leaves the road at the end of the network once it drives on)
        let Some(c) = w.t.cars.iter().find(|c| c.id == ahead) else {
            moved_after |= stopped_at.is_some();
            return;
        };
        if c.state.speed < 0.3 && stopped_at.is_none() {
            stopped_at = Some(w.t.time);
        }
        if stopped_at.is_some_and(|t| w.t.time > t + 1.0) && c.state.speed > 2.0 {
            moved_after = true;
        }
    });
    assert!(
        stopped_at.is_some(),
        "it never stopped: {}",
        describe(&w, ahead)
    );
    assert!(moved_after, "it stood for good: {}", describe(&w, ahead));
}
