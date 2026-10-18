//! Shortest ways over the lanes: from one lane to another, and from a vehicle back onto a
//! list of lanes (the navigator's way back to the route, a duty's way to its first stop).

use glam::DVec3;
use hashbrown::HashMap;

use super::Network;
use crate::traffic::lane::LaneKind;
use omsi_geometry::util::angle_diff;

impl Network {
    /// Find shortest lane path from `start` to `target` using Dijkstra's algorithm.
    pub fn shortest_path(&self, start: usize, target: usize) -> Option<Vec<usize>> {
        if start >= self.lanes.len() || target >= self.lanes.len() {
            return None;
        }
        if start == target {
            return Some(vec![start]);
        }
        use std::cmp::Ordering;
        use std::collections::BinaryHeap;

        #[derive(Copy, Clone, PartialEq)]
        struct State {
            cost: f32,
            position: usize,
        }
        impl Eq for State {}
        impl Ord for State {
            fn cmp(&self, other: &Self) -> Ordering {
                other.cost.total_cmp(&self.cost)
            }
        }
        impl PartialOrd for State {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }

        let mut dist = vec![f32::INFINITY; self.lanes.len()];
        let mut parent = vec![None; self.lanes.len()];
        let mut heap = BinaryHeap::new();

        dist[start] = 0.0;
        heap.push(State {
            cost: 0.0,
            position: start,
        });

        while let Some(State { cost, position }) = heap.pop() {
            if position == target {
                let mut path = Vec::new();
                let mut curr = Some(target);
                while let Some(node) = curr {
                    path.push(node);
                    curr = parent[node];
                }
                path.reverse();
                return Some(path);
            }

            if cost > dist[position] {
                continue;
            }

            for &next_idx in &self.lanes[position].next {
                if next_idx >= self.lanes.len() {
                    continue;
                }
                let next_len = self.lanes[next_idx].length();
                let next_cost = cost + next_len;
                if next_cost < dist[next_idx] {
                    dist[next_idx] = next_cost;
                    parent[next_idx] = Some(position);
                    heap.push(State {
                        cost: next_cost,
                        position: next_idx,
                    });
                }
            }
        }
        None
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
