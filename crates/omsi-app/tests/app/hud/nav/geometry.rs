use openomsi_game::testing::hud::nav::geometry::turn_towards;
use std::f64::consts::PI;

#[test]
fn the_view_turns_the_short_way_and_lands_on_the_target() {
    // from 350 degrees to 10: forwards through north, not back through 340 degrees
    let (a, b) = (350f64.to_radians(), 10f64.to_radians());
    let step = turn_towards(a, b, 1.0 / 60.0);
    assert!(step > a, "{step}");
    // it eases in a few frames and then sits exactly on the target
    let mut p = 0.0;
    for _ in 0..600 {
        p = turn_towards(p, PI * 0.75, 1.0 / 60.0);
    }
    assert_eq!(p, PI * 0.75);
    // no time passing, no turning; a target already reached stays
    assert_eq!(turn_towards(1.0, 2.0, 0.0), 1.0);
    assert_eq!(turn_towards(0.5, 0.5, 0.016), 0.5);
    // a step never overshoots
    let s = turn_towards(0.0, 1.0, 10.0);
    assert!(s <= 1.0 && s > 0.99);
}

mod road_index {
    use glam::{DVec2, DVec3};
    use omsi_sim::traffic::{Lane, LaneBuilder, LaneKey, LaneKind, Network};
    use omsi_ui::Painter;
    use openomsi_game::testing::hud::nav::geometry::{build_roads, road_geometry, roads_within};
    use openomsi_game::testing::hud::nav::{MapRoad, RoadIndex};

    /// The minimap's radius of roads around the bus (`ROAD_RADIUS`).
    const ROAD_RADIUS: f64 = 1300.0;

    fn lane(points: Vec<DVec3>, speed: f32) -> Lane {
        let mut l = LaneBuilder::polyline(points, LaneKind::Street, 3.0);
        l.speed_limit_kmh = speed;
        l
    }

    /// Streets every 150 m over 9 km around the origin (points every 2 m, some fast), long
    /// diagonals across many cells, and the two lanes of a few two-lane splines.
    fn city() -> Network {
        let mut lanes = Vec::new();
        for k in -30..=30 {
            let c = k as f64 * 150.0 + 7.0;
            let along = |t: f64| t * 2.0 - 4500.0;
            let speed = if k % 4 == 0 { 60.0 } else { 30.0 };
            lanes.push(lane(
                (0..=4500)
                    .map(|t| DVec3::new(c, along(t as f64), 0.0))
                    .collect(),
                speed,
            ));
            lanes.push(lane(
                (0..=4500)
                    .map(|t| DVec3::new(along(t as f64), c, 0.0))
                    .collect(),
                speed,
            ));
        }
        for d in [-3000.0, 0.0, 2500.0] {
            lanes.push(lane(
                (0..=600)
                    .map(|t| {
                        DVec3::new(t as f64 * 10.0 - 3000.0, t as f64 * 10.0 - 3000.0 + d, 0.0)
                    })
                    .collect(),
                50.0,
            ));
        }
        for (id, (x, y)) in [(-2000.0, 1300.0), (400.0, -900.0), (1700.0, 2100.0)]
            .into_iter()
            .enumerate()
        {
            for (path, offset) in [-1.75f32, 1.75].into_iter().enumerate() {
                let mut l = lane(
                    (0..=200)
                        .map(|t| DVec3::new(x + offset as f64, y + t as f64 * 2.0, 0.0))
                        .collect(),
                    40.0,
                );
                l.source = 1;
                l.offset = offset;
                l.key = Some(LaneKey {
                    tile: (0, 0),
                    id: id as i64,
                    path: path as u16,
                });
                lanes.push(l);
            }
        }
        Network {
            lanes,
            ..Default::default()
        }
    }

    fn same(a: &[MapRoad], b: &[MapRoad]) -> bool {
        a.len() == b.len()
            && a.iter()
                .zip(b)
                .all(|(a, b)| a.points == b.points && a.width == b.width && a.main == b.main)
    }

    /// Places around the map, on cell borders, at its edge and outside it.
    const CENTRES: [(f64, f64); 7] = [
        (0.0, 0.0),
        (250.0, -250.0),
        (1234.5, 987.6),
        (-3999.0, 4100.0),
        (4400.0, -4400.0),
        (-1300.0, 0.0),
        (9000.0, 9000.0),
    ];

    #[test]
    fn the_index_finds_the_same_roads_in_the_same_order_as_looking_at_all() {
        let net = city();
        let index = RoadIndex::new(&net);
        for (x, y) in CENTRES {
            let c = DVec2::new(x, y);
            for radius in [ROAD_RADIUS, 100.0, 1.0] {
                let indexed: Vec<MapRoad> = index.near(&net, c, radius).collect();
                let looked: Vec<MapRoad> =
                    roads_within(road_geometry(&net).into_iter(), c, radius).collect();
                assert!(same(&indexed, &looked), "around {c} within {radius} m");
            }
        }
        // (the places on the map do find roads, the one outside it none)
        assert!(index.near(&net, DVec2::ZERO, ROAD_RADIUS).count() > 10);
        let outside = DVec2::new(9000.0, 9000.0);
        assert_eq!(index.near(&net, outside, ROAD_RADIUS).count(), 0);
    }

    #[test]
    fn the_minimap_draws_the_same_roads_from_the_index() {
        let net = city();
        let index = RoadIndex::new(&net);
        for (x, y) in CENTRES {
            let c = DVec2::new(x, y);
            for light in [false, true] {
                let mut indexed = Painter::new();
                build_roads(&mut indexed, index.near(&net, c, ROAD_RADIUS), c, light);
                let mut looked = Painter::new();
                let all = road_geometry(&net).into_iter();
                build_roads(&mut looked, roads_within(all, c, ROAD_RADIUS), c, light);
                assert!(indexed.verts == looked.verts, "around {c}, light {light}");
            }
        }
    }
}
