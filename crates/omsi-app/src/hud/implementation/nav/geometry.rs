use super::{MapRoad, RoadIndex, RoadPlan, RoadShape, ROAD, ROAD_CASING, ROAD_MAIN};
use glam::{DVec2, DVec3, Vec3};
use hashbrown::HashMap;
use omsi_sim::traffic::{LaneKey, LaneKind, Network};
use omsi_ui::{Color, Painter};

/// Congestion of the traffic's lanes carried over to the same paths of `net`.
pub(super) fn congestion_on(
    net: &Network,
    traffic: &Network,
    c: &HashMap<usize, f32>,
) -> HashMap<usize, f32> {
    let mut out = HashMap::new();
    for (&l, &v) in c {
        let Some(lane) = traffic.lanes.get(l) else {
            continue;
        };
        let Some(key) = lane.key else { continue };
        if let Some(cands) = net.by_key.get(&key) {
            for &g in cands {
                if net.lanes[g].reversed == lane.reversed {
                    out.insert(g, v);
                }
            }
        }
    }
    out
}

/// Street lanes that stand for drawn roads, including editor-only paths corroborated by
/// road surfaces. Uncorroborated helpers must not make a phantom road on the GPS.
pub fn visible_road_lanes(net: &Network) -> Vec<(usize, &omsi_sim::traffic::Lane)> {
    let mut seen = hashbrown::HashSet::<(LaneKey, u32)>::new();
    net.lanes
        .iter()
        .enumerate()
        .filter(|(_, l)| l.kind == LaneKind::Street && !l.invisible && l.points.len() >= 2)
        .filter(|(_, l)| {
            if let Some(key) = l.key {
                // A two-way [path] has the same geometry in both directions. Keep one
                // casing and surface; retain separate [path]s, which may be real lanes.
                return seen.insert((key, l.source));
            }
            true
        })
        .collect()
}

/// The roads a city map draws, from a whole map's lanes as they were read off the tiles:
/// every path linked to its neighbours, the editor-only ones confirmed by the map's own
/// asphalt, then grouped into carriageways with their real widths. The navigator's city map
/// and the launcher's map picture draw the same roads this way, so the two cannot drift
/// apart; the second value is the linked network the caller may walk (a trip's own lanes).
pub(crate) fn city_roads(
    lanes: Vec<omsi_sim::traffic::Lane>,
    surfaces: &[(Vec<DVec3>, f32)],
) -> (Vec<MapRoad>, Network) {
    let mut net = Network {
        lanes,
        ..Default::default()
    };
    net.link(1.5);
    confirm_road_surfaces(&mut net, surfaces);
    let roads = road_geometry(&net);
    (roads, net)
}

/// Some maps separate the asphalt mesh from their editor-only traffic splines. Use the
/// actual road footprint to distinguish those paths from invisible scenery helpers. This
/// changes only the navigator's copy of the network; driving still uses the original data.
pub fn confirm_road_surfaces(net: &mut Network, surfaces: &[(Vec<DVec3>, f32)]) {
    let mut segments = Vec::new();
    let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    for (pts, width) in surfaces {
        for ab in pts.windows(2) {
            let (a, b) = (ab[0], ab[1]);
            let margin = *width as f64 * 0.5 + 0.5;
            let lo = a.truncate().min(b.truncate()) - DVec2::splat(margin);
            let hi = a.truncate().max(b.truncate()) + DVec2::splat(margin);
            let (x0, y0) = Network::grid_cell(lo.extend(0.0));
            let (x1, y1) = Network::grid_cell(hi.extend(0.0));
            let i = segments.len();
            segments.push((a, b, margin));
            for x in x0..=x1 {
                for y in y0..=y1 {
                    grid.entry((x, y)).or_default().push(i);
                }
            }
        }
    }
    for lane in net
        .lanes
        .iter_mut()
        .filter(|l| l.invisible && l.kind == LaneKind::Street)
    {
        let n = (lane.length() / 8.0).ceil().max(1.0) as usize;
        let mut covered = 0;
        for k in 0..n {
            let (p, _) = lane.at(lane.length() * (k as f32 + 0.5) / n as f32);
            let on_surface = grid
                .get(&Network::grid_cell(p))
                .map(|ids| {
                    ids.iter().any(|&i| {
                        let (a, b, margin) = segments[i];
                        let ab = (b - a).truncate();
                        let t = ((p - a).truncate().dot(ab) / ab.length_squared().max(1e-6))
                            .clamp(0.0, 1.0);
                        let q = a.lerp(b, t);
                        (p - q).truncate().length() <= margin && (p.z - q.z).abs() <= 2.0
                    })
                })
                .unwrap_or(false);
            if on_surface {
                covered += 1;
            }
        }
        if covered * 4 >= n * 3 {
            lane.invisible = false;
        }
    }
    // Crossings made of objects can have no asphalt spline. Keep helpers joining two
    // corroborated roads; an isolated invisible helper still does not become a street.
    let mut prev = vec![Vec::new(); net.lanes.len()];
    for (i, lane) in net
        .lanes
        .iter()
        .enumerate()
        .filter(|(_, l)| l.kind == LaneKind::Street)
    {
        for &j in &lane.next {
            if net
                .lanes
                .get(j)
                .map(|l| l.kind == LaneKind::Street)
                .unwrap_or(false)
            {
                prev[j].push(i);
            }
        }
    }
    let mut seen = vec![false; net.lanes.len()];
    let mut keep = Vec::new();
    for seed in 0..net.lanes.len() {
        if seen[seed] || !net.lanes[seed].invisible || net.lanes[seed].kind != LaneKind::Street {
            continue;
        }
        let mut queue = vec![seed];
        let mut component = Vec::new();
        let mut anchors = hashbrown::HashSet::new();
        while let Some(i) = queue.pop() {
            if seen[i] {
                continue;
            }
            seen[i] = true;
            component.push(i);
            for &j in net.lanes[i].next.iter().chain(prev[i].iter()) {
                let Some(l) = net.lanes.get(j).filter(|l| l.kind == LaneKind::Street) else {
                    continue;
                };
                if l.invisible {
                    if !seen[j] {
                        queue.push(j);
                    }
                } else {
                    anchors.insert((l.key, l.source, if l.key.is_none() { j } else { 0 }));
                }
            }
        }
        if anchors.len() >= 2 {
            keep.extend(component);
        }
    }
    for i in keep {
        net.lanes[i].invisible = false;
    }
}

/// Traffic paths are lane centres, not separate streets. Adjacent paths on one spline
/// describe one carriageway; combine their footprints without filling a median. Align
/// opposing directions before averaging so bends, mirrors and ramps retain their shape.
pub fn road_geometry(net: &Network) -> Vec<MapRoad> {
    road_plans(net).iter().map(|r| r.road(net)).collect()
}

/// The roads of `road_geometry` as the lanes they are made of, without their points.
fn road_plans(net: &Network) -> Vec<RoadPlan> {
    let mut roads = Vec::new();
    let mut splines = std::collections::BTreeMap::<
        ((i32, i32), i64),
        Vec<(usize, &omsi_sim::traffic::Lane)>,
    >::new();
    for (i, lane) in visible_road_lanes(net) {
        if let Some(key) = lane.key.filter(|_| lane.source == 1) {
            splines
                .entry((key.tile, key.id))
                .or_default()
                .push((i, lane));
        } else {
            roads.push(RoadPlan {
                shape: RoadShape::Lane(i),
                width: lane.width.max(2.6),
                main: lane.speed_limit_kmh >= 55.0,
            });
        }
    }
    for mut lanes in splines.into_values() {
        lanes.sort_by(|a, b| a.1.offset.total_cmp(&b.1.offset));
        let mut start = 0;
        while start < lanes.len() {
            let a = lanes[start].1;
            let lo = a.offset - a.width.max(2.6) * 0.5;
            let mut hi = a.offset + a.width.max(2.6) * 0.5;
            let mut end = start + 1;
            while end < lanes.len() {
                let b = lanes[end].1;
                if b.points.len() != a.points.len() || b.offset - b.width.max(2.6) * 0.5 > hi + 0.65
                {
                    break;
                }
                let mid = a.points.len() / 2;
                let za = a.points[if a.reversed {
                    a.points.len() - 1 - mid
                } else {
                    mid
                }]
                .z;
                let zb = b.points[if b.reversed {
                    b.points.len() - 1 - mid
                } else {
                    mid
                }]
                .z;
                if (za - zb).abs() > 1.5 {
                    break;
                }
                hi = hi.max(b.offset + b.width.max(2.6) * 0.5);
                end += 1;
            }
            let b = lanes[end - 1].1;
            let span = b.offset - a.offset;
            let t = if span.abs() > 0.01 {
                (((lo + hi) * 0.5 - a.offset) / span) as f64
            } else {
                0.0
            };
            roads.push(RoadPlan {
                shape: RoadShape::Between {
                    a: lanes[start].0,
                    b: lanes[end - 1].0,
                    t,
                },
                width: hi - lo,
                main: lanes[start..end]
                    .iter()
                    .any(|l| l.1.speed_limit_kmh >= 55.0),
            });
            start = end;
        }
    }
    // Bridge only explicit graph connections within the map's placement tolerance.
    let street = |l: &&omsi_sim::traffic::Lane| {
        l.kind == LaneKind::Street && !l.invisible && l.points.len() >= 2
    };
    for (i, lane) in net.lanes.iter().enumerate().filter(|(_, l)| street(l)) {
        for &j in &lane.next {
            let Some(next) = net.lanes.get(j).filter(street) else {
                continue;
            };
            let distance = lane.end().distance(next.start());
            if distance > 0.05 && distance <= 2.0 {
                roads.push(RoadPlan {
                    shape: RoadShape::Gap { from: i, to: j },
                    width: lane.width.min(next.width).max(2.6),
                    main: lane.speed_limit_kmh >= 55.0 && next.speed_limit_kmh >= 55.0,
                });
            }
        }
    }
    roads
}

impl RoadPlan {
    /// The road with its points, made from `net`'s lanes (the network it was planned on).
    fn road(&self, net: &Network) -> MapRoad {
        MapRoad {
            points: self.points(net),
            width: self.width,
            main: self.main,
        }
    }

    fn points(&self, net: &Network) -> Vec<DVec3> {
        let lanes = &net.lanes;
        match self.shape {
            RoadShape::Lane(i) => lanes[i].points.clone(),
            RoadShape::Between { a, b, t } => {
                let (a, b) = (&lanes[a], &lanes[b]);
                let point = |l: &omsi_sim::traffic::Lane, i: usize| {
                    l.points[if l.reversed {
                        l.points.len() - 1 - i
                    } else {
                        i
                    }]
                };
                (0..a.points.len())
                    .map(|i| point(a, i).lerp(point(b, i), t))
                    .collect()
            }
            RoadShape::Gap { from, to } => vec![lanes[from].end(), lanes[to].start()],
        }
    }
}

/// The colours roads are drawn in: edge, side street, main road (the light look has pale roads with a grey edge on the light ground).
pub fn road_colors(light: bool) -> (Color, Color, Color) {
    if light {
        (
            Color::hex(0xa8a397),
            Color::hex(0xffffff),
            Color::hex(0xf3d98b),
        )
    } else {
        (ROAD_CASING, ROAD, ROAD_MAIN)
    }
}

/// `roads` (those around `anchor`, see `roads_within`) as the minimap draws them: casings
/// first, then surfaces (so that a junction's surfaces cover each other's casings).
pub fn build_roads(
    p: &mut Painter,
    roads: impl Iterator<Item = MapRoad>,
    anchor: DVec2,
    light: bool,
) {
    let (casing, road, main) = road_colors(light);
    let rel = |q: DVec3| Vec3::new((q.x - anchor.x) as f32, (q.y - anchor.y) as f32, 0.0);
    let lanes: Vec<(MapRoad, Vec<Vec3>)> = roads
        .map(|l| {
            let pts = simplify(&l.points.iter().map(|q| rel(*q)).collect::<Vec<_>>(), 0.12);
            (l, pts)
        })
        .collect();
    for (l, pts) in &lanes {
        p.ribbon(pts, l.width + 1.6, 3.6, casing, true);
    }
    for (l, pts) in &lanes {
        p.ribbon(
            pts,
            l.width + 0.2,
            2.4,
            if l.main { main } else { road },
            true,
        );
    }
}

/// Side of a cell of `RoadIndex`'s grid (m): the minimap's 1.3 km radius covers about a
/// hundred cells, a long road only a few.
const ROAD_CELL: f64 = 250.0;

impl RoadIndex {
    pub fn new(net: &Network) -> Self {
        let plans = road_plans(net);
        let mut cells: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for (i, plan) in plans.iter().enumerate() {
            let mut last = None;
            for q in &plan.points(net) {
                // (a lane has a point every metre or two: most follow one in the same cell)
                let cell = road_cell(q.truncate());
                if last == Some(cell) {
                    continue;
                }
                last = Some(cell);
                let ids = cells.entry(cell).or_default();
                // (roads go in in order, so a road already in this cell is the last one)
                if ids.last() != Some(&i) {
                    ids.push(i);
                }
            }
        }
        Self { plans, cells }
    }

    /// The roads of `net` (the network the index was made of) with a point closer than
    /// `radius` to `centre`, in `road_geometry`'s order: the order they are drawn in, so
    /// that the picture does not depend on the index.
    pub fn near<'a>(
        &'a self,
        net: &'a Network,
        centre: DVec2,
        radius: f64,
    ) -> impl Iterator<Item = MapRoad> + 'a {
        let (x0, y0) = road_cell(centre - DVec2::splat(radius));
        let (x1, y1) = road_cell(centre + DVec2::splat(radius));
        let mut ids: Vec<usize> = (x0..=x1)
            .flat_map(|x| (y0..=y1).map(move |y| (x, y)))
            .filter_map(|c| self.cells.get(&c))
            .flatten()
            .copied()
            .collect();
        ids.sort_unstable();
        ids.dedup();
        roads_within(
            ids.into_iter().map(move |i| self.plans[i].road(net)),
            centre,
            radius,
        )
    }
}

/// The `roads` with a point closer than `radius` to `centre`, in their order.
pub fn roads_within(
    roads: impl Iterator<Item = MapRoad>,
    centre: DVec2,
    radius: f64,
) -> impl Iterator<Item = MapRoad> {
    roads.filter(move |l| {
        l.points
            .iter()
            .any(|q| (q.truncate() - centre).length() < radius)
    })
}

fn road_cell(p: DVec2) -> (i64, i64) {
    (
        (p.x / ROAD_CELL).floor() as i64,
        (p.y / ROAD_CELL).floor() as i64,
    )
}

/// `pts` with the points dropped that lie within `tol` metres of the line through their
/// neighbours kept (Douglas-Peucker): a lane is sampled every metre or two, a ribbon needs
/// only its bends. The launcher's map picture draws its roads with this too.
pub(crate) fn simplify(pts: &[Vec3], tol: f32) -> Vec<Vec3> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let mut keep = vec![false; pts.len()];
    keep[0] = true;
    keep[pts.len() - 1] = true;
    let mut stack = vec![(0usize, pts.len() - 1)];
    while let Some((a, b)) = stack.pop() {
        let (pa, pb) = (pts[a].truncate(), pts[b].truncate());
        let ab = pb - pa;
        let len = ab.length().max(1e-6);
        let mut worst = (0.0f32, 0usize);
        for k in a + 1..b {
            let d = (pts[k].truncate() - pa).perp_dot(ab).abs() / len;
            if d > worst.0 {
                worst = (d, k);
            }
        }
        if worst.0 > tol {
            keep[worst.1] = true;
            stack.push((a, worst.1));
            stack.push((worst.1, b));
        }
    }
    pts.iter()
        .zip(keep)
        .filter(|(_, k)| *k)
        .map(|(p, _)| *p)
        .collect()
}

/// The map's turn (radians) one frame of `dt` seconds on its way to `target`: the short way round, easing out, and
/// exactly on the target once it is within a thousandth of a radian.
pub fn turn_towards(phi: f64, target: f64, dt: f64) -> f64 {
    use std::f64::consts::{PI, TAU};
    let diff = (target - phi + PI).rem_euclid(TAU) - PI;
    if diff.abs() < 1e-3 {
        target
    } else {
        phi + diff * (1.0 - (-dt.max(0.0) * 8.0).exp())
    }
}
