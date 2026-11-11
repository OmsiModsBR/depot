//! Shoes through switches and crossings, laid out as the content lays them: the wire stops
//! short of the object and goes on past it, and the object draws the wire across the gap.

use glam::{DMat3, DVec2, DVec3};
use omsi_trolley::testing::{object, spline, tile};
use omsi_trolley::{
    build_tile_wires, declaration_from, parse_params, parse_registry, BodyPose, ConventionParams,
    PoleInputs, PolePhase, PoleSim, TrolleyDeclaration, WireNetwork,
};

const WIRE: &str = "Splines\\Net\\wire_x.sli";
const Y: &str = "Sceneryobjects\\Net\\Troll_strelka_out_02_Y.sco";
const RIGHT: &str = "Sceneryobjects\\Net\\chave_segmento_direita_depot.sco";
const LEFT: &str = "Sceneryobjects\\Net\\chave_segmento_esquerda_depot.sco";
const CROSSING: &str = "Sceneryobjects\\Net\\Troll_strelka_OnKrest.sco";

const PARAMS: &str = "\
[global]
spline_keywords=wire_x
wire_height=5.8
wire_x_left=-0.255
wire_x_right=0.255
manual_capture_distance=0.45
lost_reset_frames=90
max_pitch=55
switch_throttle_threshold=0.05
[physics]
pole_raise_duration=1.5
pole_lower_duration=1.0
spring_nominal_force_n=82
spring_reference_elevation_deg=25
spring_gain_n_per_deg=1.15
contact_force_max_n=160
complex_speed_limit_kmh=25
complex_overspeed_grace=0.16
contact_force_dewire_n=12
contact_force_dewire_time=0.18
contact_reach_release_error_m=0.10
contact_reach_release_time=0.12
dewire_yaw_limit_deg=87
dewire_free_rise_deg_s=42
dewire_catcher_delay=0.55
dewire_catcher_lower_deg_s=34
dewire_lateral_force_n=260
dewire_lateral_force_time=0.20
";

const REGISTRY: &str = "\
[vehicle:rigid]
bus_file=Vehicles\\T\\rigid.bus
pole_base_left_x=-0.25
pole_base_right_x=0.25
pole_base_forward=2.24
pole_base_height=3.455
contact_left_x=-0.255
contact_right_x=0.255
contact_forward=-3.93
contact_height=5.41
";

fn params() -> ConventionParams {
    parse_params(PARAMS).unwrap()
}

fn decl() -> TrolleyDeclaration {
    declaration_from(&parse_registry(REGISTRY)[0], &params()).unwrap()
}

fn net(
    splines: Vec<omsi_map::tile::MapSpline>,
    objects: Vec<omsi_map::tile::MapObject>,
) -> WireNetwork {
    let mut n = WireNetwork::default();
    n.insert_tile(build_tile_wires(
        (0, 0),
        DVec2::ZERO,
        &tile(splines, objects),
        &params(),
    ));
    n
}

fn body(x: f64, y: f64, heading: f64) -> BodyPose {
    BodyPose {
        position: DVec3::new(x, y, 0.0),
        rotation: DMat3::from_rotation_z(-heading.to_radians()),
    }
}

/// Raise both poles at (x, y) heading north, then drive north to `to_y` with the switch
/// demand `demand`; the spline ids the two shoes end on.
fn drive_north(
    n: &WireNetwork,
    x: f64,
    from_y: f64,
    to_y: f64,
    demand: f64,
) -> (PoleSim, [i64; 2]) {
    let d = decl();
    let mut sim = PoleSim::default();
    let at = body(x, from_y, 0.0);
    let press = PoleInputs {
        grab: [true, true],
        ..Default::default()
    };
    sim.step(&d, n, &at, press, 1.0 / 60.0);
    for _ in 0..100 {
        sim.step(&d, n, &at, PoleInputs::default(), 1.0 / 60.0);
    }
    assert!(
        sim.poles.iter().all(|p| p.phase == PolePhase::OnWire),
        "raised"
    );
    let mut y = from_y;
    while y < to_y {
        y += 0.2;
        let o = sim.step(
            &d,
            n,
            &body(x, y, 0.0),
            PoleInputs {
                switch_demand: demand,
                ..Default::default()
            },
            1.0 / 60.0,
        );
        assert!(
            o.wire_present,
            "on the wire at y = {y:.1}: {:?}",
            sim.events
        );
        assert!(
            o.poles[0].shoe_pos.x < o.poles[1].shoe_pos.x,
            "sides kept at y = {y:.1}"
        );
    }
    let ids = [0, 1].map(|i| n.span(sim.poles[i].cursor.unwrap().span).unwrap().spline_id);
    (sim, ids)
}

/// A Y at y = 51: the line from the south ends at y = 50, two branches start at y = 52.85,
/// half a metre either side, and part 10 degrees each way.
fn y_switch() -> WireNetwork {
    net(
        vec![
            spline(1, WIRE, [0.0, 0.0, 0.0], 0.0, 50.0, 0.0),
            spline(2, WIRE, [-0.5, 52.85, 0.0], -10.0, 40.0, 0.0),
            spline(3, WIRE, [0.5, 52.85, 0.0], 10.0, 40.0, 0.0),
        ],
        vec![object(9, Y, [0.0, 51.4, 0.0], 0.0)],
    )
}

#[test]
fn a_y_switch_is_bridged_and_the_entry_has_two_ways_on() {
    let n = y_switch();
    let at_entry: Vec<_> = n.joints_at(((0, 0), 0), 1).collect();
    assert_eq!(at_entry.len(), 2);
    assert!(at_entry
        .iter()
        .all(|j| j.via.is_some() && (j.gap - 2.89).abs() < 0.05));
}

#[test]
fn without_demand_a_y_turns_right_and_with_demand_left() {
    let n = y_switch();
    let (_, coasting) = drive_north(&n, 0.0, 20.0, 64.0, 0.0);
    assert_eq!(coasting, [3, 3], "both shoes into the right branch");
    let (sim, powered) = drive_north(&n, 0.0, 20.0, 64.0, 1.0);
    assert_eq!(powered, [2, 2], "both shoes into the left branch");
    // (each shoe took the fork once, and the second took the way the first set)
    let _ = sim;
}

/// A right branch switch at y = 52: the line ends at y = 50, the straight goes on at
/// y = 54 and the branch leaves at y = 54, 0.72 m to the right, 20 degrees off.
fn right_branch_switch() -> WireNetwork {
    net(
        vec![
            spline(1, WIRE, [0.0, 0.0, 0.0], 0.0, 50.0, 0.0),
            spline(2, WIRE, [0.0, 54.0, 0.0], 0.0, 40.0, 0.0),
            spline(3, WIRE, [0.72, 54.0, 0.0], 20.0, 40.0, 0.0),
        ],
        vec![object(9, RIGHT, [0.0, 52.0, 0.0], 0.0)],
    )
}

#[test]
fn every_switch_takes_its_left_way_with_demand_and_its_right_way_without() {
    // on a right branch switch the left way is the straight one, the right way the branch
    let n = right_branch_switch();
    let (_, powered) = drive_north(&n, 0.0, 20.0, 62.0, 1.0);
    assert_eq!(powered, [2, 2]);
    let (_, coasting) = drive_north(&n, 0.0, 20.0, 62.0, 0.0);
    assert_eq!(coasting, [3, 3]);
}

#[test]
fn faster_than_the_limit_through_a_switch_the_poles_leave_the_wire() {
    let n = right_branch_switch();
    let d = decl();
    for (kmh, dewires) in [(20.0, false), (40.0, true)] {
        let mut sim = PoleSim::default();
        let at = body(0.0, 20.0, 0.0);
        sim.step(
            &d,
            &n,
            &at,
            PoleInputs {
                grab: [true, true],
                ..Default::default()
            },
            1.0 / 60.0,
        );
        for _ in 0..100 {
            sim.step(&d, &n, &at, PoleInputs::default(), 1.0 / 60.0);
        }
        let (mut y, mut limited, mut left) = (20.0, false, None);
        while y < 70.0 && left.is_none() {
            y += kmh / 3.6 / 60.0;
            let o = sim.step(
                &d,
                &n,
                &body(0.0, y, 0.0),
                // (with demand: the straight way, which the bus follows)
                PoleInputs {
                    speed_kmh: kmh,
                    switch_demand: 1.0,
                    ..Default::default()
                },
                1.0 / 60.0,
            );
            limited |= o.speed_limit_active;
            left = sim.poles.iter().find_map(|p| p.dewire);
        }
        assert!(limited, "the limit applied in the switch");
        assert_eq!(left.is_some(), dewires, "{kmh} km/h");
        if dewires {
            assert_eq!(left, Some(omsi_trolley::DewireReason::Overspeed));
        }
    }
}

#[test]
fn two_switches_in_a_row_each_take_their_exit() {
    // a left branch switch, then 30 m on a Y
    let n = net(
        vec![
            spline(1, WIRE, [0.0, 0.0, 0.0], 0.0, 50.0, 0.0),
            spline(2, WIRE, [0.0, 54.0, 0.0], 0.0, 30.0, 0.0),
            spline(3, WIRE, [-0.72, 54.0, 0.0], -20.0, 30.0, 0.0),
            spline(4, WIRE, [-0.5, 86.85, 0.0], -10.0, 30.0, 0.0),
            spline(5, WIRE, [0.5, 86.85, 0.0], 10.0, 30.0, 0.0),
        ],
        vec![
            object(8, LEFT, [0.0, 52.0, 0.0], 0.0),
            object(9, Y, [0.0, 85.4, 0.0], 0.0),
        ],
    );
    let (_, ids) = drive_north(&n, 0.0, 20.0, 98.0, 0.0);
    assert_eq!(ids, [5, 5], "straight through the first, right at the Y");
}

#[test]
fn coming_out_of_a_branch_the_shoes_join_the_common_wire() {
    // the Y driven the other way: south down its right branch into the single line
    let n = y_switch();
    let d = decl();
    let mut sim = PoleSim::default();
    // on the right branch, heading 190 (south-south-west), 30 m along it
    let h = 190.0f64;
    let start = DVec2::new(0.5, 52.85)
        + DVec2::new(10f64.to_radians().sin(), 10f64.to_radians().cos()) * 30.0;
    let at = body(start.x, start.y, h);
    sim.step(
        &d,
        &n,
        &at,
        PoleInputs {
            grab: [true, true],
            ..Default::default()
        },
        1.0 / 60.0,
    );
    for _ in 0..100 {
        sim.step(&d, &n, &at, PoleInputs::default(), 1.0 / 60.0);
    }
    assert!(sim.poles.iter().all(|p| p.phase == PolePhase::OnWire));
    // drive down the branch and on south along the line
    let dir = DVec2::new(-(10f64.to_radians().sin()), -(10f64.to_radians().cos()));
    let mut p = start;
    for _ in 0..200 {
        p += dir * 0.2;
        let (x, y, hd) = if p.y > 52.0 {
            (p.x, p.y, h)
        } else {
            (0.0, p.y, 180.0)
        };
        if p.y <= 52.0 {
            p.x = 0.0;
        }
        let o = sim.step(&d, &n, &body(x, y, hd), PoleInputs::default(), 1.0 / 60.0);
        assert!(o.wire_present, "on the wire at {p:?}: {:?}", sim.events);
    }
    let ids = [0, 1].map(|i| n.span(sim.poles[i].cursor.unwrap().span).unwrap().spline_id);
    assert_eq!(ids, [1, 1]);
}

#[test]
fn a_neighbouring_line_beside_a_switch_never_takes_a_shoe() {
    // the Y, and another line 1.9 m to its left all the way
    let mut splines = vec![
        spline(1, WIRE, [0.0, 0.0, 0.0], 0.0, 50.0, 0.0),
        spline(2, WIRE, [-0.5, 52.85, 0.0], -10.0, 40.0, 0.0),
        spline(3, WIRE, [0.5, 52.85, 0.0], 10.0, 40.0, 0.0),
    ];
    splines.push(spline(7, WIRE, [-1.9, 0.0, 0.0], 0.0, 120.0, 0.0));
    let n = net(splines, vec![object(9, Y, [0.0, 51.4, 0.0], 0.0)]);
    let (_, ids) = drive_north(&n, 0.0, 20.0, 60.0, 0.0);
    assert_eq!(ids, [3, 3]);
}

#[test]
fn a_crossing_is_passed_straight_on() {
    // the line cut by a crossing at y = 50 (gap 2.9 m), and the crossing line east-west
    let n = net(
        vec![
            spline(1, WIRE, [0.0, 0.0, 0.0], 0.0, 48.55, 0.0),
            spline(2, WIRE, [0.0, 51.45, 0.0], 0.0, 40.0, 0.0),
            spline(3, WIRE, [-40.0, 50.0, 0.0], 90.0, 38.55, 0.0),
            spline(4, WIRE, [1.45, 50.0, 0.0], 90.0, 40.0, 0.0),
        ],
        vec![object(9, CROSSING, [0.0, 50.0, 0.0], 0.0)],
    );
    let (_, ids) = drive_north(&n, 0.0, 20.0, 70.0, 1.0);
    assert_eq!(ids, [2, 2]);
}
