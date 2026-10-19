//! Traces of the AI drivers on the lane network (`traffic`), the safety net of its refactor
//! (Trilha S): a synthetic town - a main road of two lanes each way east, a side road
//! crossing it, the junction's paths with their priorities, a dead end - and the drivers
//! that wander on it, give way, wait at a light, follow a car that brakes, change lanes and
//! run out of road, written out frame by frame with what the network says of itself.
//!
//! The same contract as `physics_traces.rs`: with `OMSI_PHYSICS_TRACE_DIR` set, each scenario
//! writes `<dir>/<name>.trace`, and `scripts/physics-trace-diff.sh` runs them on a base commit
//! and on the change and compares the two. A change that only moves code leaves every trace
//! as it was, to the bit. Without it, each scenario runs twice and both runs must write the
//! same trace. The file uses only the crate's public API: the script copies it as it is, with
//! `support/`, into the commit it compares with.
//!
//! What the game decides around a driver (who it gives way to, the light it stops at, the car
//! ahead) is worked out here in a few plain rules, the same on both sides: the trace is of
//! what `traffic` makes of it.

mod support;

use glam::DVec3;
use omsi_sim::traffic::{
    arrival_time, AiState, Lane, LaneBuilder, LaneKey, LaneKind, Lead, Network,
    TrafficLightController,
};
use std::fmt::Write;
use support::trace::{check, dv3, n32, Trace};

// ---------------------------------------------------------------------------------------
// The town

/// Where the junction's paths begin and end on the main road (m either side of the side
/// road's middle line).
const J: f64 = 10.25;
/// The side road's lane, east of the line the main road's lanes are measured from.
const SIDE_X: f64 = 1.75;
/// The main road's second lane, south of the first (to the right going east).
const LANE_2: f64 = 3.5;

/// A street lane of spline `id` (`path` its lane, `offset` how far right of the spline).
fn spline(id: i64, path: u16, offset: f32, start: DVec3, heading: f64, length: f64) -> Lane {
    let mut l = LaneBuilder::arc(start, heading, length, 0.0, 0.0, LaneKind::Street, 3.0);
    l.key = Some(LaneKey {
        tile: (0, 0),
        id,
        path,
    });
    l.source = 1;
    l.offset = offset;
    l
}

/// A path of the junction object (`radius` > 0 a right turn, < 0 a left one).
fn junction(path: u16, start: DVec3, heading: f64, length: f64, radius: f64) -> Lane {
    let mut l = LaneBuilder::arc(start, heading, length, radius, 0.0, LaneKind::Street, 3.0);
    l.key = Some(LaneKey {
        tile: (0, 0),
        id: 100,
        path,
    });
    l.source = 2;
    l
}

/// The lanes of the town by name, in the order `town` puts them in the network.
mod lane {
    pub const WEST_1: usize = 0;
    pub const WEST_2: usize = 1;
    pub const SOUTH: usize = 2;
    pub const NORTH: usize = 3;
    pub const EAST_1: usize = 4;
    pub const EAST_2: usize = 5;
    pub const MAIN_1: usize = 6;
    pub const MAIN_2: usize = 7;
    pub const ACROSS: usize = 8;
    pub const RIGHT: usize = 9;
    pub const LEFT: usize = 10;
}

/// The town: the main road comes from the west in two lanes, crosses the side road and goes
/// on east for 700 m; the side road comes from the south, crosses and ends 60 m north of
/// the junction (a dead end). From the south a car goes across or turns right into the main
/// road's second lane; from the main road's first lane it goes on or turns left into the
/// dead end. The main road has the way (192), the side road gives it (64). With `lights`,
/// light 0 of the junction stops the side road and light 1 the main road.
fn town(lights: bool) -> Network {
    let east = 90.0;
    let north = 0.0;
    let mut lanes = vec![
        spline(1, 0, 0.0, DVec3::new(-J - 300.0, 0.0, 0.0), east, 300.0),
        spline(
            1,
            1,
            LANE_2 as f32,
            DVec3::new(-J - 300.0, -LANE_2, 0.0),
            east,
            300.0,
        ),
        spline(
            2,
            0,
            0.0,
            DVec3::new(SIDE_X, -12.0 - 300.0, 0.0),
            north,
            300.0,
        ),
        spline(3, 0, 0.0, DVec3::new(SIDE_X, 12.0, 0.0), north, 60.0),
        spline(4, 0, 0.0, DVec3::new(J, 0.0, 0.0), east, 700.0),
        spline(
            4,
            1,
            LANE_2 as f32,
            DVec3::new(J, -LANE_2, 0.0),
            east,
            700.0,
        ),
        junction(0, DVec3::new(-J, 0.0, 0.0), east, 2.0 * J, 0.0),
        junction(1, DVec3::new(-J, -LANE_2, 0.0), east, 2.0 * J, 0.0),
        junction(2, DVec3::new(SIDE_X, -12.0, 0.0), north, 24.0, 0.0),
        junction(
            3,
            DVec3::new(SIDE_X, -12.0, 0.0),
            north,
            std::f64::consts::FRAC_PI_2 * 8.5,
            8.5,
        ),
        junction(
            4,
            DVec3::new(-J, 0.0, 0.0),
            east,
            std::f64::consts::FRAC_PI_2 * 12.0,
            -12.0,
        ),
    ];
    for (l, priority) in [
        (lane::MAIN_1, 192.0),
        (lane::MAIN_2, 192.0),
        (lane::ACROSS, 64.0),
        (lane::RIGHT, 64.0),
    ] {
        lanes[l].priority = priority;
    }
    lanes[lane::RIGHT].turn = 2;
    lanes[lane::LEFT].turn = 1;
    if lights {
        for l in [lane::ACROSS, lane::RIGHT] {
            lanes[l].traffic_light = Some((0, 0));
        }
        for l in [lane::MAIN_1, lane::MAIN_2, lane::LEFT] {
            lanes[l].traffic_light = Some((0, 1));
        }
    }
    let mut net = Network {
        lanes,
        ..Default::default()
    };
    net.link(1.5);
    net
}

/// The junction's lights: the side road red for 20 s, red and yellow for 1, green for 10 and
/// yellow for 3; the main road the other way round.
fn lights() -> TrafficLightController {
    TrafficLightController::new(
        vec![
            vec![(0, 20.0), (3, 1.0), (6, 10.0), (9, 3.0)],
            vec![(6, 16.0), (9, 3.0), (0, 1.0), (0, 13.0), (3, 1.0)],
        ],
        34.0,
    )
}

// ---------------------------------------------------------------------------------------
// The drivers around a driver

/// The distance from `car` to the place `s` of `lane`: on it, or on the lane it drives
/// onto next.
fn distance_to(net: &Network, car: &AiState, lane: usize, s: f32) -> Option<f32> {
    if car.lane == lane {
        return Some(s - car.s);
    }
    let mut d = net.lanes[car.lane].length() - car.s;
    for l in car.upcoming().take(3) {
        if l == lane {
            return Some(d + s);
        }
        d += net.lanes[l].length();
    }
    None
}

/// The car ahead on the way, nearest first.
fn lead(net: &Network, me: usize, cars: &[AiState]) -> Option<Lead> {
    let car = &cars[me];
    let mut best: Option<Lead> = None;
    for (k, other) in cars.iter().enumerate() {
        if k == me {
            continue;
        }
        let Some(d) = distance_to(net, car, other.lane, other.s) else {
            continue;
        };
        let gap = d - other.rear - car.front;
        if d > 0.0 {
            best = Lead::min(
                best,
                Some(Lead {
                    gap,
                    speed: other.speed,
                    acc: other.acc,
                }),
            );
        }
    }
    best
}

/// Where the car has to stop, from its origin: at the end of its lane, when the junction path
/// it drives onto next is red or crosses a path of the way with a car coming on it in less
/// than the gap the driver accepts.
fn stop(
    net: &Network,
    me: usize,
    cars: &[AiState],
    lights: Option<&TrafficLightController>,
) -> Option<f32> {
    let car = &cars[me];
    let next = car.planned_next?;
    let lane = &net.lanes[next];
    if lane.source != 2 || net.lanes[car.lane].source == 2 {
        return None;
    }
    let rest = net.lanes[car.lane].length() - car.s;
    if let (Some(lights), Some((_, light))) = (lights, lane.traffic_light) {
        if !TrafficLightController::allows_go(lights.state(light)) {
            return Some(rest);
        }
    }
    let mine = arrival_time(rest, car.speed, car.accel, 50.0 / 3.6);
    for c in &net.crossings[next] {
        if !net.must_yield(next, c.other) {
            continue;
        }
        for (k, other) in cars.iter().enumerate() {
            if k == me {
                continue;
            }
            let Some(d) = distance_to(net, other, c.other, c.other_at) else {
                continue;
            };
            if d > -(c.other_after + other.rear)
                && arrival_time(d.max(0.0), other.speed, other.accel, 50.0 / 3.6)
                    < mine + car.accept_gap
            {
                return Some(rest);
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------------------
// Writing the state

fn opt(x: Option<f32>) -> String {
    x.map(n32).unwrap_or_else(|| "-".into())
}

/// The network as `link` made it.
fn network(t: &mut Trace, net: &Network) {
    for (i, l) in net.lanes.iter().enumerate() {
        t.line(&format!(
            "lane {i} start {} end {} heading {} {} length {} next {:?} prev {:?} left {:?} right {:?} reach {}",
            dv3(l.start()),
            dv3(l.end()),
            n32(l.start_heading()),
            n32(l.end_heading()),
            n32(l.length()),
            l.next,
            net.prev[i],
            l.left,
            l.right,
            n32(net.reach[i]),
        ));
        t.line(&format!("  conflicts {:?}", net.conflicts[i]));
        for c in &net.crossings[i] {
            t.line(&format!(
                "  crossing other {} at {} other_at {} merge {} before {} after {} other_before {} other_after {} yield {}",
                c.other,
                n32(c.at),
                n32(c.other_at),
                c.merge,
                n32(c.before),
                n32(c.after),
                n32(c.other_before),
                n32(c.other_after),
                net.must_yield(i, c.other),
            ));
        }
        for s in [0.0, l.length() * 0.5, l.length()] {
            let (p, h) = l.at(s);
            t.line(&format!(
                "  at {} p {} heading {} curvature {}",
                n32(s),
                dv3(p),
                n32(h),
                n32(l.curvature_at(s))
            ));
        }
    }
}

/// A driver as the traffic reads it.
fn car(t: &mut Trace, net: &Network, i: usize, c: &AiState) {
    let change = c.change.map(|c| {
        format!(
            "to {} t {} length {} s_to {} dir {} wait {} bypass {}",
            c.to,
            n32(c.t),
            n32(c.length),
            n32(c.s_to),
            c.dir,
            n32(c.wait),
            c.bypass
        )
    });
    let _ = writeln!(
        t.text,
        "  car {i} lane {} s {} speed {} acc {} braking {} held {} start {} blinker {} signal {} next {:?} ahead {:?} prev {:?} route {} change [{}] plan {:?} lateral {} {} odometer {} yield {} cooldown {}",
        c.lane,
        n32(c.s),
        n32(c.speed),
        n32(c.acc),
        c.braking,
        c.held,
        n32(c.start_timer),
        c.blinker,
        c.signal,
        c.planned_next,
        c.ahead,
        c.prev_lane,
        c.route_index,
        change.unwrap_or_default(),
        c.change_plan,
        n32(c.lateral),
        n32(c.lateral_target),
        n32(c.odometer),
        n32(c.yield_time),
        n32(c.change_cooldown),
    );
    let _ = writeln!(
        t.text,
        "    way {} bend {} lateral_ahead {} turn {:?} change_due {:?}",
        dv3(c.way_point(net, 10.0)),
        n32(c.curve_speed(net)),
        n32(c.lateral_ahead(5.0)),
        c.turn_ahead(net, 60.0).map(|(d, s)| (d, n32(s))),
        c.route_change_due(net),
    );
}

fn light(t: &mut Trace, lights: &TrafficLightController) {
    for i in 0..lights.lights.len() {
        let _ = writeln!(
            t.text,
            "  light {i} state {} change {} go {} remaining {} time {} held {}",
            lights.state(i),
            opt(lights.time_to_change(i)),
            opt(lights.time_until_go(i)),
            n32(lights.remaining(i)),
            support::trace::n64(lights.time),
            lights.held,
        );
    }
}

// ---------------------------------------------------------------------------------------
// Running the town

/// A driver on `lane` at `s`, moving at `kmh`.
fn driver(net: &Network, lane: usize, s: f32, kmh: f32, seed: u64) -> AiState {
    let mut c = AiState::new(lane, s, seed);
    c.speed = kmh / 3.6;
    c.plan_next(net);
    c
}

/// What a frame does besides driving: before the drivers move, with the frame's number.
type Hook<'a> = &'a mut dyn FnMut(usize, &Network, &mut [AiState]);

/// `frames` frames of `dt` (a frame in seven a third longer): the lights run on, each driver
/// sees the others where the frame found them and drives, and the trace takes everything.
fn run(
    t: &mut Trace,
    net: &Network,
    cars: &mut Vec<AiState>,
    mut lights: Option<TrafficLightController>,
    frames: usize,
    dt: f32,
    hook: Hook,
) {
    let mut gone = vec![false; cars.len()];
    for f in 0..frames {
        let dt = if f % 7 == 3 { dt * 4.0 / 3.0 } else { dt };
        hook(f, net, cars.as_mut_slice());
        if let Some(lights) = lights.as_mut() {
            lights.request.iter_mut().for_each(|r| *r = false);
            lights.advance(dt);
        }
        t.frame(dt, &format!("cars {}", cars.len()));
        let seen = cars.clone();
        for i in 0..cars.len() {
            if gone[i] {
                continue;
            }
            let lead = lead(net, i, &seen);
            let stop = stop(net, i, &seen, lights.as_ref());
            let _ = writeln!(
                t.text,
                "  sees {i} lead {:?} stop {}",
                lead.map(|l| (n32(l.gap), n32(l.speed), n32(l.acc))),
                opt(stop)
            );
            if !cars[i].drive(net, dt, lead, stop) {
                gone[i] = true;
                t.line(&format!("  car {i} ran out of road"));
                continue;
            }
            cars[i].update_blinker(net);
        }
        if let Some(lights) = lights.as_ref() {
            light(t, lights);
        }
        for (i, c) in cars.iter().enumerate() {
            if !gone[i] {
                car(t, net, i, c);
            }
        }
    }
}

macro_rules! scenarios {
    ($($name:ident),* $(,)?) => {
        $(
            #[test]
            fn $name() {
                check(stringify!($name), scenarios::$name);
            }
        )*
    };
}

scenarios!(
    n01_the_town_as_linked,
    n02_the_town_asked,
    r01_side_road_gives_way,
    r02_waits_at_a_red_light,
    r03_follows_a_car_that_brakes,
    r04_changes_lanes,
    r05_pulls_out_round_a_standing_car,
    r06_wanders_and_keeps_off_the_dead_end,
    r07_runs_out_of_road,
);

mod scenarios {
    use super::*;

    pub fn n01_the_town_as_linked() -> String {
        let mut t = Trace::default();
        network(&mut t, &town(false));
        t.line("lights");
        network(&mut t, &town(true));
        t.text
    }

    pub fn n02_the_town_asked() -> String {
        let net = town(false);
        let mut t = Trace::default();
        for (a, b) in [
            (lane::WEST_1, lane::NORTH),
            (lane::SOUTH, lane::EAST_2),
            (lane::SOUTH, lane::EAST_1),
            (lane::WEST_2, lane::NORTH),
        ] {
            t.line(&format!("path {a} {b} {:?}", net.shortest_path(a, b)));
        }
        for p in [
            DVec3::new(-50.0, -1.0, 0.0),
            DVec3::new(0.0, -6.0, 0.0),
            DVec3::new(SIDE_X + 0.4, 40.0, 0.0),
            DVec3::new(200.0, -3.0, 0.0),
        ] {
            t.line(&format!(
                "near {} lane {:?} starting {:?}",
                dv3(p),
                net.nearest_lane(p, LaneKind::Street).map(|(l, s, d)| (
                    l,
                    n32(s),
                    support::trace::n64(d)
                )),
                net.lanes_starting_near(p, 20.0),
            ));
        }
        for l in [lane::ACROSS, lane::EAST_1, lane::NORTH] {
            t.line(&format!(
                "upstream {l} {:?}",
                net.upstream(l, 5.0, 80.0, 8)
                    .into_iter()
                    .map(|(l, d, via)| (l, n32(d), via))
                    .collect::<Vec<_>>()
            ));
        }
        for (a, b) in [
            (lane::WEST_1, lane::WEST_2),
            (lane::EAST_2, lane::EAST_1),
            (lane::WEST_1, lane::SOUTH),
        ] {
            t.line(&format!(
                "beside {a} {b} parallel {} s {} delta {}",
                net.parallel(a, b),
                n32(net.beside_s(a, b, 120.0)),
                n32(net.beside_delta(a, b)),
            ));
        }
        t.line(&format!(
            "on route {:?}",
            net.project_on_route(
                &[lane::WEST_1, lane::MAIN_1, lane::EAST_1],
                DVec3::new(30.0, 1.0, 0.0)
            )
            .map(|(l, s)| (l, n32(s)))
        ));
        t.text
    }

    /// A car from the south to go across, a car on the main road that passes first, and one
    /// behind it far enough back for the first to go before it.
    pub fn r01_side_road_gives_way() -> String {
        let net = town(false);
        let mut t = Trace::default();
        let mut across = AiState::new(lane::SOUTH, 240.0, 3);
        across.speed = 40.0 / 3.6;
        across.set_route(&net, vec![lane::SOUTH, lane::ACROSS, lane::NORTH], 240.0);
        let mut cars = vec![
            across,
            driver(&net, lane::WEST_1, 230.0, 50.0, 5),
            driver(&net, lane::WEST_1, 60.0, 50.0, 7),
        ];
        run(
            &mut t,
            &net,
            &mut cars,
            None,
            420,
            1.0 / 30.0,
            &mut |_, _, _| {},
        );
        t.text
    }

    /// Cars on the side road and the main road meet the junction's lights.
    pub fn r02_waits_at_a_red_light() -> String {
        let net = town(true);
        let mut t = Trace::default();
        let mut cars = vec![
            driver(&net, lane::SOUTH, 250.0, 50.0, 11),
            driver(&net, lane::SOUTH, 200.0, 50.0, 13),
            driver(&net, lane::WEST_2, 200.0, 50.0, 17),
        ];
        let mut lights = lights();
        lights.start(0.0);
        run(
            &mut t,
            &net,
            &mut cars,
            Some(lights),
            900,
            1.0 / 20.0,
            &mut |_, _, _| {},
        );
        t.text
    }

    /// The car ahead brakes hard to a stop and drives off again; the one behind keeps its
    /// distance.
    pub fn r03_follows_a_car_that_brakes() -> String {
        let net = town(false);
        let mut t = Trace::default();
        let mut cars = vec![
            driver(&net, lane::WEST_2, 120.0, 50.0, 19),
            driver(&net, lane::WEST_2, 95.0, 50.0, 23),
        ];
        run(
            &mut t,
            &net,
            &mut cars,
            None,
            360,
            1.0 / 30.0,
            &mut |f, _, cars| {
                // the first car meets something on the road at frame 60 and it is gone at 180
                if (60..180).contains(&f) {
                    cars[0].speed = (cars[0].speed - 7.0 / 30.0).max(0.0);
                    cars[0].acc = -7.0;
                }
            },
        );
        t.text
    }

    /// A car on a route that goes on in the second lane changes over as the route asks; a
    /// wanderer in the second lane moves to the first.
    pub fn r04_changes_lanes() -> String {
        let net = town(false);
        let mut t = Trace::default();
        let mut routed = AiState::new(lane::WEST_1, 80.0, 29);
        routed.speed = 40.0 / 3.6;
        routed.change_cooldown = 0.0;
        routed.set_route(
            &net,
            vec![lane::WEST_1, lane::WEST_2, lane::MAIN_2, lane::EAST_2],
            80.0,
        );
        let mut wanderer = driver(&net, lane::WEST_2, 40.0, 45.0, 31);
        wanderer.change_cooldown = 0.0;
        let mut cars = vec![routed, wanderer];
        run(
            &mut t,
            &net,
            &mut cars,
            None,
            420,
            1.0 / 30.0,
            &mut |f, net, cars| {
                if let Some((to, dir)) = cars[0].route_change_due(net) {
                    cars[0].start_route_change(net, to, dir);
                }
                if f == 45 {
                    if let Some(to) = net.lanes[cars[1].lane].left {
                        cars[1].start_change(net, to, 1);
                    }
                }
            },
        );
        t.text
    }

    /// A car stops behind one standing in its lane and pulls out round it into the lane
    /// beside.
    pub fn r05_pulls_out_round_a_standing_car() -> String {
        let net = town(false);
        let mut t = Trace::default();
        let mut c = driver(&net, lane::WEST_2, 100.0, 40.0, 37);
        let standing = 160.0;
        for f in 0..360 {
            let dt = if f % 7 == 3 { 0.04 } else { 0.03 };
            t.frame(dt, "");
            if c.change.is_none() && c.lane == lane::WEST_2 && c.speed < 0.5 && f > 30 {
                if let Some(to) = net.lanes[c.lane].left {
                    c.start_bypass(&net, to, 1);
                }
            }
            let obstacle = (c.lane == lane::WEST_2 && c.change.is_none()).then_some(standing - c.s);
            if !c.advance(&net, dt, obstacle.filter(|d| *d > 0.0), None) {
                t.line("  ran out of road");
                break;
            }
            c.update_blinker(&net);
            car(&mut t, &net, 0, &c);
        }
        t.text
    }

    /// Drivers of many seeds come up to the junction in the first lane and from the south:
    /// each chooses its way on, and the dead end to the north is never one of them while the
    /// road east goes on.
    pub fn r06_wanders_and_keeps_off_the_dead_end() -> String {
        let net = town(false);
        let mut t = Trace::default();
        for start in [lane::WEST_1, lane::SOUTH] {
            for seed in 1..=24u64 {
                let mut c = driver(&net, start, 250.0, 50.0, seed * 7919);
                for _ in 0..6 {
                    c.drive(&net, 0.25, None, None);
                }
                t.frame(0.25, &format!("lane {start} seed {seed}"));
                car(&mut t, &net, 0, &c);
            }
        }
        t.text
    }

    /// A car sent into the dead end drives it to its end and leaves the road there.
    pub fn r07_runs_out_of_road() -> String {
        let net = town(false);
        let mut t = Trace::default();
        let mut c = AiState::new(lane::WEST_1, 260.0, 41);
        c.speed = 30.0 / 3.6;
        c.set_route(&net, vec![lane::WEST_1, lane::LEFT, lane::NORTH], 260.0);
        let mut cars = vec![c];
        run(
            &mut t,
            &net,
            &mut cars,
            None,
            540,
            1.0 / 30.0,
            &mut |_, _, _| {},
        );
        t.text
    }
}
