use glam::{DVec2, DVec3, Vec2, Vec3};
use hashbrown::HashMap;
use omsi_sim::traffic::{LaneKind, Network};
use omsi_ui::Painter;
use super::angle_diff;
use super::draw::{ARROW, LEVEL, level};

/// Lanes with a point within `radius` of `c` (by the network's 50 m grid).
pub(super) fn lanes_near(net: &Network, c: DVec2, radius: f64) -> Vec<usize> {
    let mut seen = hashbrown::HashSet::new();
    let (cx, cy) = Network::grid_cell(c.extend(0.0));
    let r = (radius / 50.0).ceil() as i32;
    if net.grid.is_empty() {
        return (0..net.lanes.len()).filter(|&i| net.lanes[i].points.iter().any(|q| (q.truncate() - c).length() < radius)).collect();
    }
    for gx in cx - r..=cx + r {
        for gy in cy - r..=cy + r {
            if let Some(v) = net.grid.get(&(gx, gy)) {
                seen.extend(v.iter().copied());
            }
        }
    }
    let mut v: Vec<usize> = seen.into_iter().collect();
    v.sort_unstable();
    v
}

/// `OMSI_NAV_PROBE=x,y[,r]`: the lanes of the map's network that start or end within r
/// metres (25) of a point - how they link, to see why a route cannot reach a place.
pub(super) fn probe_lanes(net: &Network) {
    let Some(v) = omsi_cfg::flags::NAV_PROBE.text() else { return };
    let f: Vec<f64> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
    if f.len() < 2 {
        return;
    }
    let c = DVec2::new(f[0], f[1]);
    let r = f.get(2).copied().unwrap_or(25.0);
    for (i, l) in net.lanes.iter().enumerate() {
        let (s, e) = (l.start(), l.end());
        if (s.truncate() - c).length() > r && (e.truncate() - c).length() > r && l.nearest_point(DVec3::new(c.x, c.y, s.z)).map(|p| p.1 > r).unwrap_or(true) {
            continue;
        }
        let prev = net.prev.get(i).cloned().unwrap_or_default();
        log::info!(
            "nav probe: lane {i} {:?} {:?} key {:?} rev {} start ({:.1}, {:.1}, {:.1}) h {:.0} end ({:.1}, {:.1}, {:.1}) h {:.0} len {:.1} next {:?} prev {:?}",
            l.kind, l.name, l.key, l.reversed, s.x, s.y, s.z, l.start_heading(), e.x, e.y, e.z, l.end_heading(), l.length(), l.next, prev
        );
    }
}

/// How a route is drawn: its width (lane width plus `extra_m`, at least `min_px`), arrows
/// every so many metres for so far, how much of it, and only near a point.
pub(super) struct RouteStyle {
    pub(super) extra_m: f32,
    pub(super) min_px: f32,
    pub(super) arrows: Option<(f32, f32)>,
    pub(super) max_len: f32,
    pub(super) near: Option<(DVec2, f64)>,
}

/// Which lane of `lane`'s road (the lanes beside it going the same way) the bus at `bus`
/// is in, counted from the kerb (0 = the kerb lane: the rightmost, the leftmost on a
/// left-hand-traffic map).
pub(super) fn lane_from_right(net: &Network, lane: usize, bus: DVec3) -> usize {
    let Some(mut cur) = net.lanes.get(lane).map(|_| lane) else { return 0 };
    let kerb = |l: &omsi_sim::traffic::Lane| if net.left_hand { l.left } else { l.right };
    let away = |l: &omsi_sim::traffic::Lane| if net.left_hand { l.right } else { l.left };
    for _ in 0..6 {
        match kerb(&net.lanes[cur]) {
            Some(n) if n < net.lanes.len() && net.lanes[n].kind == LaneKind::Street => cur = n,
            _ => break,
        }
    }
    let (mut best, mut best_d, mut k) = (0usize, f64::MAX, 0usize);
    loop {
        if let Some((_, d)) = net.lanes[cur].nearest_point(bus) {
            if d < best_d {
                best_d = d;
                best = k;
            }
        }
        match away(&net.lanes[cur]) {
            Some(n) if n < net.lanes.len() && net.lanes[n].kind == LaneKind::Street && k < 6 => {
                cur = n;
                k += 1;
            }
            _ => break,
        }
    }
    best
}

/// The route from the bus on: one band, coloured stretch by stretch by how busy the road
/// is (`jam`: congestion per lane), with arrows along it in a colour that stands out.
#[allow(clippy::too_many_arguments)]
pub(super) fn build_route(p: &mut Painter, net: &Network, lanes: &[usize], anchor: DVec2, s0: f32, jam: &HashMap<usize, f32>, style: &RouteStyle, bus_lane: usize) {
    let rel = |q: DVec3| Vec3::new((q.x - anchor.x) as f32, (q.y - anchor.y) as f32, 0.0);
    // runs of one colour: the lanes' points joined up, the first lane cut where the bus is
    let mut runs: Vec<(Vec<Vec3>, f32, usize)> = Vec::new();
    let mut arrows: Vec<(DVec3, f32, usize, f32)> = Vec::new();
    let mut total = 0.0f32;
    // the lane to show: of a road with several lanes each way, the one a driver takes - the
    // leftmost before a left turn, the rightmost before a right turn, and otherwise the one
    // the bus is driving in (`bus_lane`, counted from the right). Always the rightmost, the
    // route ran along the kerb on Berlin's three-lane roads, where the cars are parked; the
    // timetable's own track often runs in the middle lane.
    let turn_after = |k: usize| -> i32 {
        let mut acc = 0.0f32;
        for &j in lanes.iter().skip(k + 1).take(40) {
            let Some(l) = net.lanes.get(j) else { break };
            let d = omsi_sim::traffic::wrap_deg(l.end_heading() - l.start_heading());
            if d.abs() > 35.0 && l.length() < 60.0 {
                return if d > 0.0 { 1 } else { -1 };
            }
            // a lane that has no neighbours ends the stretch of several lanes
            if l.left.is_none() && l.right.is_none() {
                break;
            }
            acc += l.length();
            if acc > 250.0 {
                break;
            }
        }
        0
    };
    let shown = |k: usize, l: usize| -> usize {
        let side = turn_after(k);
        let step = |cur: usize, left: bool| -> Option<usize> {
            let n = if left { net.lanes[cur].left } else { net.lanes[cur].right }?;
            (n < net.lanes.len() && net.lanes[n].kind == LaneKind::Street).then_some(n)
        };
        let mut cur = l;
        // before a turn the lane on its side; straight on the bus's own, counted from the kerb
        // (the right, or the left on a left-hand-traffic map)
        let to_left = if side == 0 { net.left_hand } else { side < 0 };
        for _ in 0..6 {
            match step(cur, to_left) {
                Some(n) => cur = n,
                None => break,
            }
        }
        if side == 0 {
            for _ in 0..bus_lane {
                match step(cur, !net.left_hand) {
                    Some(n) => cur = n,
                    None => break,
                }
            }
        }
        cur
    };
    for (k, &l) in lanes.iter().enumerate() {
        let Some(lane) = net.lanes.get(l) else { break };
        let route_l = l;
        let l = shown(k, l);
        let lane = net.lanes.get(l).unwrap_or(lane);
        if let Some((c, r)) = style.near {
            if (c - lane.start().truncate()).length() > r {
                break;
            }
        }
        let from = if k == 0 { s0 } else { 0.0 };
        let mut pts: Vec<Vec3> = Vec::new();
        if k == 0 {
            pts.push(rel(lane.at(s0).0));
        }
        for (q, d) in lane.points.iter().zip(&lane.dist) {
            if *d > from + 0.05 || k > 0 {
                pts.push(rel(*q));
            }
        }
        let lv = level(jam.get(&route_l).copied().unwrap_or(0.0));
        let w = lane.width.max(2.6);
        match runs.last_mut() {
            Some(run) if run.2 == lv => {
                run.0.extend(pts);
                run.1 = run.1.max(w);
            }
            _ => {
                // (a new colour starts where the last one ended)
                let mut start = runs.last().and_then(|r| r.0.last().copied()).map(|q| vec![q]).unwrap_or_default();
                start.extend(pts);
                runs.push((start, w, lv));
            }
        }
        // arrows at even steps along each lane (they stay put while the bus drives)
        if let Some((every, reach)) = style.arrows {
            let len = lane.length();
            // (never more than a few dozen a lane: zoomed far in on the city map the step
            // came out a few millimetres and the loop ran for minutes - the game froze)
            if total < reach && len > every * 0.4 && every > 0.5 {
                let n = (len / every).round().clamp(1.0, 64.0);
                let step = len / n;
                for i in 0..n as usize {
                    let at = step * (i as f32 + 0.5);
                    if at > from + 4.0 && total + at - from < reach {
                        let (q, h) = lane.at(at);
                        arrows.push((q, h, lv, w));
                    }
                }
            }
        }
        total += lane.length() - from;
        if total > style.max_len {
            break;
        }
    }
    for (pts, w, lv) in &runs {
        p.ribbon(pts, w + style.extra_m, style.min_px, LEVEL[*lv], true);
    }
    for (q, h, lv, w) in arrows {
        let hr = h.to_radians();
        let d = Vec2::new(hr.sin(), hr.cos());
        let n = Vec2::new(-d.y, d.x);
        let (tip, bl, br) = (d * 0.55, -d * 0.45 + n * 0.75, -d * 0.45 - n * 0.75);
        let t = -d * 0.42;
        let (sm, spx) = ((w + style.extra_m) * 0.5 * 0.8, style.min_px * 0.5 * 1.05);
        let c = ARROW[lv];
        p.world_shape(rel(q), &[tip, bl, bl + t, tip + t], sm, spx, c);
        p.world_shape(rel(q), &[tip, br, br + t, tip + t], sm, spx, c);
    }
}

/// A way from where the bus is back onto `ahead` (the route from the lane it was last
/// on): the lanes to drive up to the route, and the index in `ahead` of the route lane it
/// reaches (the route goes on from there). Dijkstra over
/// the street lanes, from the lane under the bus that runs its way, to the first route
/// lane reached (the search stops at 6 km).
pub(crate) fn way_back(net: &Network, bus: DVec3, heading: f64, ahead: &[usize], max_cost: f32) -> Option<(Vec<usize>, usize)> {
    use std::cmp::Ordering;
    use std::collections::BinaryHeap;
    // the lane under the bus going its way; else (a depot, a car park, the grass) the
    // nearest street lane that does not point back at the bus, else any near
    let cands: Vec<(usize, f64, f64)> = lanes_near(net, bus.truncate(), 90.0)
        .into_iter()
        .filter_map(|i| {
            let l = net.lanes.get(i)?;
            if l.kind != LaneKind::Street {
                return None;
            }
            let (s, d) = l.nearest_point(bus)?;
            let (_, h) = l.at(s);
            Some((i, d, angle_diff(heading, h as f64).abs()))
        })
        .collect();
    let pick = |max_d: f64, max_a: f64| cands.iter().filter(|c| c.1 < max_d && c.2 < max_a).min_by(|a, b| (a.1 + a.2 * 0.1).total_cmp(&(b.1 + b.2 * 0.1))).map(|c| c.0);
    let start = pick(14.0, 70.0).or_else(|| pick(80.0, 100.0)).or_else(|| pick(80.0, 181.0));
    if omsi_cfg::flags::DEBUG_NAV.is_set() {
        log::info!("navigator: way search from {:?} ({} street lanes within 90 m) to {} route lanes", start, cands.len(), ahead.len());
    }
    let start = start?;
    let targets: HashMap<usize, usize> = ahead.iter().take(120).enumerate().map(|(k, &l)| (l, k)).collect();
    #[derive(PartialEq)]
    struct Node(f32, usize);
    impl Eq for Node {}
    impl Ord for Node {
        fn cmp(&self, o: &Self) -> Ordering {
            o.0.total_cmp(&self.0)
        }
    }
    impl PartialOrd for Node {
        fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
            Some(self.cmp(o))
        }
    }
    let mut dist: HashMap<usize, f32> = HashMap::new();
    let mut parent: HashMap<usize, usize> = HashMap::new();
    let mut heap = BinaryHeap::new();
    dist.insert(start, 0.0);
    heap.push(Node(0.0, start));
    while let Some(Node(cost, lane)) = heap.pop() {
        if cost > dist.get(&lane).copied().unwrap_or(f32::INFINITY) {
            continue;
        }
        if lane != start {
            if let Some(&k) = targets.get(&lane) {
                let mut path = vec![lane];
                let mut c = lane;
                while let Some(&p) = parent.get(&c) {
                    path.push(p);
                    c = p;
                }
                path.reverse();
                // (the route lane reached is where the route goes on)
                path.pop();
                return Some((path, k));
            }
        }
        if cost > max_cost {
            break;
        }
        let Some(l) = net.lanes.get(lane) else { continue };
        for &n in &l.next {
            let Some(nl) = net.lanes.get(n) else { continue };
            if nl.kind != LaneKind::Street {
                continue;
            }
            // turning back is the last resort
            let u_turn = angle_diff(l.end_heading() as f64, nl.start_heading() as f64).abs() > 150.0;
            let c = cost + nl.length() + if u_turn { 400.0 } else { 0.0 };
            if c < dist.get(&n).copied().unwrap_or(f32::INFINITY) {
                dist.insert(n, c);
                parent.insert(n, lane);
                heap.push(Node(c, n));
            }
        }
    }
    None
}
