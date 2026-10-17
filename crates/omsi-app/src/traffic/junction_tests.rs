//! The junction scenarios of the AI traffic, one per open report: a junction entered with no
//! room to leave it (#37), cars that wait at a green light (#173) and two emergency vehicles
//! that give way to each other (#192). They run `Traffic::tick` over the synthetic crossroads
//! of `junction_scenarios`.

use super::junction_scenarios::*;
use omsi_sim::traffic::{Aspect, TrafficLightController};

/// What a car did, for a failed assertion.
fn describe(w: &World, id: u64) -> String {
    let c = w.of(id);
    format!(
        "car {id} lane {} s {:.1} speed {:.2} stood {:.1} yielding {} light_hold {} wait_at {:?} lead {:?} why {:?}",
        c.state.lane, c.state.s, c.state.speed, c.stopped, c.yielding, c.light_hold, c.wait_at, c.lead_info, c.why
    )
}

/// The lanes a car takes through the junction from direction `d`: straight on (0), left (1) or
/// right (2), then the exit and the stretch beyond it.
fn via(w: &World, d: usize, kind: usize) -> Vec<usize> {
    let x = &w.x;
    let to = match kind {
        0 => d,
        1 => (d + 3) % 4,
        _ => (d + 1) % 4,
    };
    let path = match kind {
        0 => x.junction[d],
        1 => x.left[d].unwrap(),
        _ => x.right[d].unwrap(),
    };
    vec![x.approach[d], path, x.exit[to], x.beyond[to]]
}

/// #37: a queue stands in the exit of the junction (it waits at a red light further on). A
/// car coming to the junction, straight on or turning, with or without a light of its own,
/// must stop at its line, not drive in and stand across the crossing; it must go through once
/// the queue has moved off, and leave no claim on the junction behind.
#[test]
fn a_car_keeps_out_of_a_junction_whose_exit_is_full() {
    for kind in 0..3 {
        for signalled in [false, true] {
            let to = [NORTH, WEST, EAST][kind];
            let mut downstream = [None; 4];
            downstream[to] = Some((0, 0));
            let mut stop_line = [None; 4];
            stop_line[NORTH] = signalled.then_some((0, 1));
            let layout = Layout {
                exit_len: 16.0,
                turns: true,
                beyond_light: downstream,
                junction_light: stop_line,
            };
            // light 0 stays red, light 1 (the junction's own) stays green
            let mut w = World::new(
                layout,
                vec![TrafficLightController::new(
                    vec![vec![(0, 10_000.0)], vec![(6, 10_000.0)]],
                    10_000.0,
                )],
            );
            // three cars queue at the red light at the end of the exit: the last one's tail is
            // in the junction's exit, with no room left for another car
            let queue: Vec<u64> = (0..3)
                .map(|k| w.car(w.x.exit[to], 11.0 - 4.0 * k as f32, 3.0, 11 + k, false))
                .collect();
            w.run(20.0, |_| {});
            assert!(
                queue.iter().all(|&id| w.of(id).state.speed < 0.1),
                "the queue does not stand: {:#?}",
                queue.iter().map(|&id| describe(&w, id)).collect::<Vec<_>>()
            );
            let c = w.car(w.x.approach[NORTH], 55.0, 8.0, 99, false);
            w.route(c, via(&w, NORTH, kind));
            let mut entered = None;
            w.run(40.0, |w| {
                if entered.is_none() && w.in_junction(c) {
                    entered = Some(w.t.time);
                }
            });
            assert!(entered.is_none(), "kind {kind} signalled {signalled}: entered the junction at {entered:?} with the exit full: {}", describe(&w, c));
            // the light turns green: the queue goes, and so does the car
            w.t.lights[0] = TrafficLightController::new(
                vec![vec![(6, 10_000.0)], vec![(6, 10_000.0)]],
                10_000.0,
            );
            w.run(40.0, |_| {});
            assert!(w.through(c), "kind {kind} signalled {signalled}: did not get through after the queue moved off: {}", describe(&w, c));
            assert!(
                w.t.cars
                    .iter()
                    .all(|c| c.reserved.is_empty() || w.x.approach.contains(&c.state.lane)),
                "claims left on the junction: {:?}",
                w.t.cars
                    .iter()
                    .map(|c| (c.id, c.reserved.clone()))
                    .collect::<Vec<_>>()
            );
        }
    }
}

/// #173: a signalled crossroads on a cycle, a stream of cars on each street. A car at the line
/// of a light that shows green must be on its way within a few seconds, whatever the time of
/// day the cycle started at.
#[test]
fn cars_wait_at_red_and_go_on_green() {
    for start in [
        0.0,
        6.0 * 3600.0,
        12.0 * 3600.0 + 17.0,
        22.0 * 3600.0 + 31.0,
    ] {
        let layout = Layout {
            junction_light: [Some((0, 0)), Some((0, 1)), Some((0, 0)), Some((0, 1))],
            ..Default::default()
        };
        // north-south: green 20 s, yellow 3 s, red 27 s; east-west the other way round
        let program = TrafficLightController::new(
            vec![
                vec![(6, 20.0), (9, 3.0), (0, 27.0)],
                vec![(0, 25.0), (6, 20.0), (9, 3.0), (0, 2.0)],
            ],
            50.0,
        );
        let mut w = World::new(layout, vec![program]);
        w.t.day_time = start;
        let mut ids = Vec::new();
        // per car: seconds it has stood at the line with green showing, first in its queue
        let mut waited = std::collections::HashMap::<u64, f32>::new();
        let mut worst = (0.0f32, String::new());
        for k in 0..12 {
            // a car on each street every 10 s, 70 m before its line
            for d in [NORTH, EAST] {
                ids.push(w.car(w.x.approach[d], 30.0, 8.0, 100 + 2 * k + d as u64, false));
            }
            w.run(10.0, |w| {
                for &id in &ids {
                    let Some(c) = w.t.cars.iter().find(|c| c.id == id) else {
                        continue;
                    };
                    let Some(d) = [NORTH, EAST]
                        .into_iter()
                        .find(|&d| w.x.approach[d] == c.state.lane)
                    else {
                        waited.remove(&id);
                        continue;
                    };
                    let line = w.t.net.lanes[c.state.lane].length() - c.state.s - c.state.front;
                    let green =
                        TrafficLightController::aspect(w.t.lights[0].state(d)) == Aspect::Green;
                    let first = c.lead_info.is_none_or(|(_, gap)| gap > 12.0);
                    if green && first && line < 8.0 && c.state.speed < 0.3 {
                        let t = waited.entry(id).or_default();
                        *t += DT;
                        if *t > worst.0 {
                            worst = (*t, describe(w, id));
                        }
                    } else {
                        waited.remove(&id);
                    }
                }
            });
        }
        assert!(
            worst.0 < 6.0,
            "start {start}: waited {:.1} s at a green light: {}",
            worst.0,
            worst.1
        );
        w.run(60.0, |_| {});
        let stuck: Vec<String> = ids
            .iter()
            .filter(|&&id| !w.through(id))
            .map(|&id| describe(&w, id))
            .collect();
        assert!(
            stuck.is_empty(),
            "start {start}: cars still before the junction: {stuck:#?}"
        );
    }
}

/// #192: two emergency vehicles come to the crossroads from different streets, each one going
/// straight on or turning, at several distances and speeds (standing too), with or without a
/// car behind. One of them must go in soon, and all must be through within a minute.
#[test]
fn two_emergency_vehicles_do_not_wait_for_each_other() {
    let mut n = 0;
    for (da, db) in [
        (NORTH, EAST),
        (EAST, SOUTH),
        (SOUTH, WEST),
        (WEST, NORTH),
        (NORTH, SOUTH),
        (EAST, WEST),
    ] {
        for (ka, kb) in [(0, 0), (1, 0), (0, 1), (1, 1), (2, 1), (2, 2)] {
            for (sa, sb) in [(40.0f32, 40.0f32), (92.0, 92.0), (92.0, 40.0), (60.0, 80.0)] {
                for (va, vb) in [(0.0f32, 0.0f32), (8.0, 8.0), (12.0, 3.0)] {
                    n += 1;
                    let mut w = World::new(
                        Layout {
                            turns: true,
                            ..Default::default()
                        },
                        Vec::new(),
                    );
                    let a = w.car(w.x.approach[da], sa, va, 21, true);
                    let b = w.car(w.x.approach[db], sb, vb, 22, true);
                    w.route(a, via(&w, da, ka));
                    w.route(b, via(&w, db, kb));
                    // a car behind each, when there is room for it
                    let mut all = vec![a, b];
                    for (d, k, s, seed) in [(da, ka, sa, 23), (db, kb, sb, 24)] {
                        if s > 30.0 {
                            let id = w.car(w.x.approach[d], s - 12.0, 0.0, seed, false);
                            w.route(id, via(&w, d, k));
                            all.push(id);
                        }
                    }
                    w.run(60.0, |_| {});
                    let stuck: Vec<String> = all
                        .iter()
                        .filter(|&&id| !w.through(id))
                        .map(|&id| describe(&w, id))
                        .collect();
                    assert!(stuck.is_empty(), "{da} {ka} at {sa} m {va} m/s against {db} {kb} at {sb} m {vb} m/s: still before the junction after a minute: {stuck:#?}");
                }
            }
        }
    }
    assert_eq!(n, 6 * 6 * 4 * 3);
}
