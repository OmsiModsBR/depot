//! The shortest way over the street lanes from a vehicle back onto a list of lanes (the
//! navigator's way back to the route, a duty's way to its first stop), and the lanes near
//! a point it starts from.

use glam::{DVec2, DVec3};
use hashbrown::HashMap;

use super::{LaneKind, Network};
use crate::crowd::angle_diff;

impl Network {
    /// Lanes with a point within `radius` of `c` (by the network's 50 m grid).
    pub fn lanes_near(&self, c: DVec2, radius: f64) -> Vec<usize> {
        let mut seen = hashbrown::HashSet::new();
        let (cx, cy) = Network::grid_cell(c.extend(0.0));
        let r = (radius / 50.0).ceil() as i32;
        if self.grid.is_empty() {
            return (0..self.lanes.len())
                .filter(|&i| {
                    self.lanes[i]
                        .points
                        .iter()
                        .any(|q| (q.truncate() - c).length() < radius)
                })
                .collect();
        }
        for gx in cx - r..=cx + r {
            for gy in cy - r..=cy + r {
                if let Some(v) = self.grid.get(&(gx, gy)) {
                    seen.extend(v.iter().copied());
                }
            }
        }
        let mut v: Vec<usize> = seen.into_iter().collect();
        v.sort_unstable();
        v
    }

    /// A way from where the bus is back onto `ahead` (the route from the lane it was last
    /// on): the lanes to drive up to the route, and the index in `ahead` of the route lane it
    /// reaches (the route goes on from there). Dijkstra over
    /// the street lanes, from the lane under the bus that runs its way, to the first route
    /// lane reached (the search stops at 6 km).
    pub fn shortest_path_to_any(
        &self,
        bus: DVec3,
        heading: f64,
        ahead: &[usize],
        max_cost: f32,
    ) -> Option<(Vec<usize>, usize)> {
        use std::cmp::Ordering;
        use std::collections::BinaryHeap;
        // the lane under the bus going its way; else (a depot, a car park, the grass) the
        // nearest street lane that does not point back at the bus, else any near
        let cands: Vec<(usize, f64, f64)> = self
            .lanes_near(bus.truncate(), 90.0)
            .into_iter()
            .filter_map(|i| {
                let l = self.lanes.get(i)?;
                if l.kind != LaneKind::Street {
                    return None;
                }
                let (s, d) = l.nearest_point(bus)?;
                let (_, h) = l.at(s);
                Some((i, d, angle_diff(heading, h as f64).abs()))
            })
            .collect();
        let pick = |max_d: f64, max_a: f64| {
            cands
                .iter()
                .filter(|c| c.1 < max_d && c.2 < max_a)
                .min_by(|a, b| (a.1 + a.2 * 0.1).total_cmp(&(b.1 + b.2 * 0.1)))
                .map(|c| c.0)
        };
        let start = pick(14.0, 70.0)
            .or_else(|| pick(80.0, 100.0))
            .or_else(|| pick(80.0, 181.0));
        if omsi_cfg::flags::DEBUG_NAV.is_set() {
            log::info!(
                "navigator: way search from {:?} ({} street lanes within 90 m) to {} route lanes",
                start,
                cands.len(),
                ahead.len()
            );
        }
        let start = start?;
        let targets: HashMap<usize, usize> = ahead
            .iter()
            .take(120)
            .enumerate()
            .map(|(k, &l)| (l, k))
            .collect();
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
            let Some(l) = self.lanes.get(lane) else {
                continue;
            };
            for &n in &l.next {
                let Some(nl) = self.lanes.get(n) else {
                    continue;
                };
                if nl.kind != LaneKind::Street {
                    continue;
                }
                // turning back is the last resort
                let u_turn =
                    angle_diff(l.end_heading() as f64, nl.start_heading() as f64).abs() > 150.0;
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
}

#[cfg(test)]
mod tests {
    use super::super::{Lane, LaneBuilder};
    use super::*;

    fn straight(a: (f64, f64), b: (f64, f64)) -> Lane {
        LaneBuilder::polyline(
            vec![DVec3::new(a.0, a.1, 0.0), DVec3::new(b.0, b.1, 0.0)],
            LaneKind::Street,
            3.0,
        )
    }

    /// A square block: the route goes north then east; the bus turned west by mistake at
    /// the first corner and must be led round the block back onto the route.
    #[test]
    fn a_way_back_joins_the_route_ahead() {
        let lanes = vec![
            straight((0.0, 0.0), (0.0, 100.0)),         // 0 north (route)
            straight((0.0, 100.0), (100.0, 100.0)),     // 1 east (route)
            straight((100.0, 100.0), (200.0, 100.0)),   // 2 east (route)
            straight((0.0, 100.0), (-100.0, 100.0)),    // 3 west (the mistake)
            straight((-100.0, 100.0), (-100.0, 200.0)), // 4 north
            straight((-100.0, 200.0), (100.0, 200.0)),  // 5 east
            straight((100.0, 200.0), (100.0, 100.0)),   // 6 south, back to the route's corner
        ];
        let mut net = Network {
            lanes,
            ..Default::default()
        };
        net.link(1.5);
        // (square corners: joined by hand, `link` wants a junction's curves)
        for (a, n) in [
            (0, vec![1, 3]),
            (1, vec![2]),
            (3, vec![4]),
            (4, vec![5]),
            (5, vec![6]),
            (6, vec![2]),
        ] {
            net.lanes[a].next = n;
        }
        let route = [0usize, 1, 2];
        let (path, join) = net
            .shortest_path_to_any(DVec3::new(-40.0, 100.0, 0.0), 270.0, &route[1..], 6000.0)
            .expect("a way");
        assert_eq!(path.first(), Some(&3));
        assert!(path.contains(&6), "{path:?}");
        // round the block back to the route's second corner: it goes on east on lane 2
        assert_eq!(route[1..][join], 2, "{path:?} join {join}");
    }
}
