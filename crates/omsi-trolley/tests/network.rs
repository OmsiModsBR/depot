//! Building the wire of a tile and joining the spans of the loaded tiles.

use glam::{DVec2, DVec3};
use omsi_trolley::testing::{mirrored, object, params, spline, tile};
use omsi_trolley::{build_tile_wires, WireNetwork, WireObjectKind, WireSide};

const WIRE: &str = "Splines\\Net\\wire_x.sli";
const ROAD: &str = "Splines\\Roads\\road.sli";

fn close(a: DVec3, b: DVec3) -> bool {
    a.distance(b) < 1e-6
}

#[test]
fn only_wire_splines_become_spans() {
    let t = tile(
        vec![
            spline(1, WIRE, [10.0, 10.0, 0.0], 0.0, 50.0, 0.0),
            spline(2, ROAD, [10.0, 10.0, 0.0], 0.0, 50.0, 0.0),
        ],
        vec![],
    );
    let w = build_tile_wires((0, 0), DVec2::ZERO, &t, &params("wire_x"));
    assert_eq!(w.spans.len(), 1);
    assert_eq!(w.spans[0].spline_id, 1);
}

#[test]
fn a_map_without_wire_builds_nothing() {
    let t = tile(vec![spline(2, ROAD, [0.0; 3], 0.0, 50.0, 0.0)], vec![]);
    let w = build_tile_wires((0, 0), DVec2::ZERO, &t, &params("wire_x"));
    assert!(w.is_empty());
    let mut net = WireNetwork::default();
    net.insert_tile(w);
    assert!(net.is_empty());
}

#[test]
fn the_two_wires_lie_either_side_at_wire_height_in_the_global_frame() {
    // heading 0 = north: left is west (-x)
    let t = tile(
        vec![spline(1, WIRE, [10.0, 20.0, 3.0], 0.0, 40.0, 0.0)],
        vec![],
    );
    let w = build_tile_wires((1, 2), DVec2::new(300.0, 600.0), &t, &params("wire_x"));
    let s = &w.spans[0];
    let left = &s.wires[WireSide::Left.index()];
    let right = &s.wires[WireSide::Right.index()];
    assert!(close(left[0], DVec3::new(309.7, 620.0, 8.8)));
    assert!(close(right[0], DVec3::new(310.3, 620.0, 8.8)));
    assert!(close(*left.last().unwrap(), DVec3::new(309.7, 660.0, 8.8)));
    assert!(close(s.ends[0].point, DVec3::new(310.0, 620.0, 8.8)));
    assert!(close(s.ends[0].outward, DVec3::new(0.0, -1.0, 0.0)));
    assert!(close(s.ends[1].outward, DVec3::new(0.0, 1.0, 0.0)));
}

#[test]
fn a_mirrored_spline_swaps_the_sides() {
    let t = tile(
        vec![mirrored(spline(1, WIRE, [0.0; 3], 0.0, 10.0, 0.0))],
        vec![],
    );
    let w = build_tile_wires((0, 0), DVec2::ZERO, &t, &params("wire_x"));
    assert!(w.spans[0].wires[WireSide::Left.index()][0].x > 0.0);
}

#[test]
fn a_curved_wire_stays_within_a_centimetre_of_the_arc() {
    // a right-hand quarter circle of 20 m radius from the origin heading north: its centre
    // is 20 m east, and the right wire runs on a 19.7 m circle about it
    let length = std::f64::consts::FRAC_PI_2 * 20.0;
    let t = tile(vec![spline(1, WIRE, [0.0; 3], 0.0, length, 20.0)], vec![]);
    let w = build_tile_wires((0, 0), DVec2::ZERO, &t, &params("wire_x"));
    let right = &w.spans[0].wires[WireSide::Right.index()];
    assert!(right.len() > 10, "a curve is drawn with many points");
    let centre = DVec3::new(20.0, 0.0, 5.8);
    for pair in right.windows(2) {
        let mid = (pair[0] + pair[1]) / 2.0;
        assert!((mid.distance(centre) - 19.7).abs() < 0.01);
    }
}

#[test]
fn spans_that_meet_end_to_start_join() {
    let t = tile(
        vec![
            spline(1, WIRE, [0.0; 3], 0.0, 30.0, 0.0),
            spline(2, WIRE, [0.0, 30.0, 0.0], 0.0, 30.0, 0.0),
        ],
        vec![],
    );
    let mut net = WireNetwork::default();
    net.insert_tile(build_tile_wires((0, 0), DVec2::ZERO, &t, &params("wire_x")));
    let j = net.joints();
    assert_eq!(j.len(), 1);
    assert!(!j[0].reversed);
    assert_eq!((j[0].a.1, j[0].b.1), (1, 0));
}

#[test]
fn a_reversed_splice_joins() {
    // the second span runs back south and ends where the first ends
    let t = tile(
        vec![
            spline(1, WIRE, [0.0; 3], 0.0, 30.0, 0.0),
            spline(2, WIRE, [0.0, 60.0, 0.0], 180.0, 30.0, 0.0),
        ],
        vec![],
    );
    let mut net = WireNetwork::default();
    net.insert_tile(build_tile_wires((0, 0), DVec2::ZERO, &t, &params("wire_x")));
    let j = net.joints();
    assert_eq!(j.len(), 1);
    assert!(j[0].reversed);
}

#[test]
fn a_loose_joint_of_a_metre_joins_and_a_larger_gap_does_not() {
    let net_with_gap = |gap: f64| {
        let t = tile(
            vec![
                spline(1, WIRE, [0.0; 3], 0.0, 30.0, 0.0),
                spline(2, WIRE, [0.0, 30.0 + gap, 0.0], 0.0, 30.0, 0.0),
            ],
            vec![],
        );
        let mut net = WireNetwork::default();
        net.insert_tile(build_tile_wires((0, 0), DVec2::ZERO, &t, &params("wire_x")));
        net.joints().len()
    };
    assert_eq!(net_with_gap(1.0), 1);
    assert_eq!(net_with_gap(3.0), 0);
}

#[test]
fn spans_side_by_side_do_not_join() {
    // two parallel wires 1.9 m apart, ends level: their ways out point the same way
    let t = tile(
        vec![
            spline(1, WIRE, [0.0; 3], 0.0, 30.0, 0.0),
            spline(2, WIRE, [1.9, 0.0, 0.0], 0.0, 30.0, 0.0),
        ],
        vec![],
    );
    let mut net = WireNetwork::default();
    net.insert_tile(build_tile_wires((0, 0), DVec2::ZERO, &t, &params("wire_x")));
    assert!(net.joints().is_empty());
}

#[test]
fn joints_across_tiles_come_and_go_with_the_tiles() {
    let p = params("wire_x");
    let a = tile(
        vec![spline(1, WIRE, [0.0, 280.0, 0.0], 0.0, 20.0, 0.0)],
        vec![],
    );
    let b = tile(
        vec![spline(2, WIRE, [0.0, 0.0, 0.0], 0.0, 20.0, 0.0)],
        vec![],
    );
    let mut net = WireNetwork::default();
    net.insert_tile(build_tile_wires((0, 0), DVec2::ZERO, &a, &p));
    net.insert_tile(build_tile_wires((0, 1), DVec2::new(0.0, 300.0), &b, &p));
    assert_eq!(net.joints().len(), 1);
    assert_eq!(net.span_count(), 2);
    net.remove_tile((0, 1));
    assert!(net.joints().is_empty());
    assert_eq!(net.span_count(), 1);
    assert!(net
        .segments_near(DVec3::new(0.0, 310.0, 5.8), 2.0)
        .is_empty());
}

#[test]
fn the_nearest_wire_comes_first() {
    let t = tile(vec![spline(1, WIRE, [0.0; 3], 0.0, 100.0, 0.0)], vec![]);
    let mut net = WireNetwork::default();
    net.insert_tile(build_tile_wires((0, 0), DVec2::ZERO, &t, &params("wire_x")));
    // (45 m: inside one piece of each wire, not on the point between two)
    let near = net.segments_near(DVec3::new(0.25, 45.0, 5.8), 1.0);
    assert_eq!(near.len(), 2);
    assert_eq!(near[0].0.side, WireSide::Right);
    assert!((near[0].1 - 0.05).abs() < 1e-9);
    assert!(net
        .segments_near(DVec3::new(5.0, 45.0, 5.8), 1.0)
        .is_empty());
}

#[test]
fn wire_objects_are_found_by_file_name() {
    let t = tile(
        vec![],
        vec![
            object(
                7,
                "Sceneryobjects\\Net\\Troll_Strelka_out_02.sco",
                [5.0, 6.0, 7.0],
                90.0,
            ),
            object(
                8,
                "Sceneryobjects\\Net\\chave_segmento_direita_depot.sco",
                [0.0; 3],
                0.0,
            ),
            object(9, "Sceneryobjects\\Net\\TwoKrest.sco", [0.0; 3], 0.0),
            object(10, "Sceneryobjects\\Houses\\house.sco", [0.0; 3], 0.0),
        ],
    );
    let w = build_tile_wires((0, 0), DVec2::new(300.0, 0.0), &t, &params("wire_x"));
    let kinds: Vec<_> = w.objects.iter().map(|o| o.kind).collect();
    assert_eq!(
        kinds,
        [
            WireObjectKind::SwitchY,
            WireObjectKind::SwitchBranchRight,
            WireObjectKind::Crossing
        ]
    );
    assert!(close(w.objects[0].position, DVec3::new(305.0, 6.0, 7.0)));
    assert_eq!(w.objects[0].heading_deg, 90.0);
}

#[test]
fn the_joints_do_not_depend_on_the_order_tiles_load_in() {
    let p = params("wire_x");
    let tiles = [
        tile(
            vec![spline(1, WIRE, [0.0, 280.0, 0.0], 0.0, 20.0, 0.0)],
            vec![],
        ),
        tile(
            vec![
                spline(2, WIRE, [0.0, 0.0, 0.0], 0.0, 20.0, 0.0),
                spline(3, WIRE, [0.0, 40.0, 0.0], 180.0, 20.0, 0.0),
            ],
            vec![],
        ),
    ];
    let build = |order: [usize; 2]| {
        let mut net = WireNetwork::default();
        for i in order {
            let origin = DVec2::new(0.0, 300.0 * i as f64);
            net.insert_tile(build_tile_wires((0, i as i32), origin, &tiles[i], &p));
        }
        net.joints().to_vec()
    };
    let forward = build([0, 1]);
    assert_eq!(forward.len(), 2, "tile border and the reversed splice");
    assert_eq!(forward, build([1, 0]));
    // a tile loaded again replaces its old wire
    let mut net = WireNetwork::default();
    for _ in 0..2 {
        for (i, t) in tiles.iter().enumerate() {
            let origin = DVec2::new(0.0, 300.0 * i as f64);
            net.insert_tile(build_tile_wires((0, i as i32), origin, t, &p));
        }
    }
    assert_eq!(net.joints(), forward.as_slice());
    assert_eq!(net.span_count(), 3);
}
