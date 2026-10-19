//! Finding lanes by place: the 50 m grid, the nearest lane to a point, a lane along a
//! heading, and projecting a point onto a route.

use glam::{DVec2, DVec3};

use super::Network;
use crate::traffic::lane::{Lane, LaneKind};

/// Grid cell size of `Network::grid` (m).
pub const GRID_CELL: f64 = 50.0;
impl Network {
    /// Point on a lane sequence closest to `p`: (index into `route`, distance along that lane).
    pub fn project_on_route(&self, route: &[usize], p: DVec3) -> Option<(usize, f32)> {
        self.project_on_route_lateral(route, p)
            .map(|(ri, s, _)| (ri, s))
    }

    /// Like `project_on_route`, plus the lateral offset of `p` from the lane (m, right of
    /// the driving direction positive).
    pub fn project_on_route_lateral(&self, route: &[usize], p: DVec3) -> Option<(usize, f32, f32)> {
        let mut best: Option<(usize, f32, f64, f32)> = None;
        for (ri, &li) in route.iter().enumerate() {
            let l = &self.lanes[li];
            for k in 0..l.points.len().saturating_sub(1) {
                let a = l.points[k];
                let b = l.points[k + 1];
                let ab = b - a;
                let t = ((p - a).dot(ab) / ab.length_squared().max(1e-9)).clamp(0.0, 1.0);
                let q = a + ab * t;
                let d = (q - p).truncate().length();
                if best.map(|b| d < b.2).unwrap_or(true) {
                    let dir = ab.truncate().normalize_or_zero();
                    let rel = (p - q).truncate();
                    // right vector of (dir.x, dir.y) is (dir.y, -dir.x)
                    let lateral = (rel.x * dir.y - rel.y * dir.x) as f32;
                    best = Some((
                        ri,
                        l.dist[k] + (l.dist[k + 1] - l.dist[k]) * t as f32,
                        d,
                        lateral,
                    ));
                }
            }
        }
        best.map(|(ri, s, _, lat)| (ri, s, lat))
    }

    /// Where a bus stop at `p` lies on `route`: like [`Network::project_on_route_lateral`],
    /// but among the points within `reach` (when given) and not before route index
    /// `from`, a lane with the stop on its kerb side (right, or left with left-hand traffic)
    /// goes before a nearer one with the stop across it - a route back along the same
    /// street passes each stop twice, once from the other side, and the bus stopped at the
    /// stop across the road on its way out. Falls back to the nearest point.
    pub fn project_stop_on_route(
        &self,
        route: &[usize],
        p: DVec3,
        reach: Option<f64>,
        from: usize,
    ) -> Option<(usize, f32, f32)> {
        self.project_stop_on_route_side(route, p, reach, from, 0)
    }

    /// The same for a platform opposite the usual kerb (1), or either side (2).
    /// The side is the stop object's OMSI value, relative to the map's traffic hand.
    pub fn project_stop_on_route_side(
        &self,
        route: &[usize],
        p: DVec3,
        reach: Option<f64>,
        from: usize,
        side: u8,
    ) -> Option<(usize, f32, f32)> {
        // (index, s, distance, lateral) of the best on the platform side, and of any
        let mut kerb: Option<(usize, f32, f64, f32)> = None;
        let mut any: Option<(usize, f32, f64, f32)> = None;
        for (ri, &li) in route.iter().enumerate().skip(from.min(route.len())) {
            let l = &self.lanes[li];
            for k in 0..l.points.len().saturating_sub(1) {
                let a = l.points[k];
                let b = l.points[k + 1];
                let ab = b - a;
                let t = ((p - a).dot(ab) / ab.length_squared().max(1e-9)).clamp(0.0, 1.0);
                let q = a + ab * t;
                let d = (q - p).truncate().length();
                if reach.is_some_and(|r| d > r) {
                    continue;
                }
                let dir = ab.truncate().normalize_or_zero();
                let rel = (p - q).truncate();
                let lateral = (rel.x * dir.y - rel.y * dir.x) as f32;
                let cand = (
                    ri,
                    l.dist[k] + (l.dist[k + 1] - l.dist[k]) * t as f32,
                    d,
                    lateral,
                );
                let left = self.left_hand != (side == 1);
                let kerb_side = side == 2 || if left { lateral < -0.3 } else { lateral > 0.3 };
                if kerb_side && kerb.map(|b| d < b.2).unwrap_or(true) {
                    kerb = Some(cand);
                }
                if any.map(|b| d < b.2).unwrap_or(true) {
                    any = Some(cand);
                }
            }
        }
        match kerb.or(any) {
            Some((ri, s, _, lat)) => Some((ri, s, lat)),
            None if from > 0 => self.project_stop_on_route_side(route, p, reach, 0, side),
            None => None,
        }
    }

    /// Nearest street lane and distance along it to a world point.
    /// Sort the lanes into the grid (called by `link`; call again after adding lanes).
    pub fn build_grid(&mut self) {
        self.grid.clear();
        self.start_grid.clear();
        for (i, l) in self.lanes.iter().enumerate() {
            self.start_grid
                .entry(Self::grid_cell(l.start()))
                .or_default()
                .push(i);
            let mut cells: Vec<(i32, i32)> = Vec::new();
            for w in l.points.windows(2) {
                // every cell along the segment, sampled finer than a cell
                let n = ((w[1] - w[0]).truncate().length() / (GRID_CELL * 0.5))
                    .ceil()
                    .max(1.0) as usize;
                for k in 0..=n {
                    let q = w[0].lerp(w[1], k as f64 / n as f64);
                    let c = (
                        (q.x / GRID_CELL).floor() as i32,
                        (q.y / GRID_CELL).floor() as i32,
                    );
                    if !cells.contains(&c) {
                        cells.push(c);
                    }
                }
            }
            if l.points.len() == 1 {
                cells.push((
                    (l.points[0].x / GRID_CELL).floor() as i32,
                    (l.points[0].y / GRID_CELL).floor() as i32,
                ));
            }
            for c in cells {
                self.grid.entry(c).or_default().push(i);
            }
        }
    }

    /// Lane indices whose starts lie in cells intersecting a circle around `p`.
    /// Callers still apply their own exact distance and lane-kind tests. Sorting keeps
    /// their traversal (and seeded traffic selection) in map order.
    pub fn lanes_starting_near(&self, p: DVec3, radius: f64) -> Vec<usize> {
        let min_x = ((p.x - radius) / GRID_CELL).floor() as i32;
        let max_x = ((p.x + radius) / GRID_CELL).floor() as i32;
        let min_y = ((p.y - radius) / GRID_CELL).floor() as i32;
        let max_y = ((p.y + radius) / GRID_CELL).floor() as i32;
        let cells = (max_x as i64 - min_x as i64 + 1) * (max_y as i64 - min_y as i64 + 1);
        if self.start_grid.is_empty() || cells > self.start_grid.len() as i64 * 2 {
            return (0..self.lanes.len()).collect();
        }
        let mut out = Vec::new();
        for x in min_x..=max_x {
            for y in min_y..=max_y {
                if let Some(lanes) = self.start_grid.get(&(x, y)) {
                    out.extend_from_slice(lanes);
                }
            }
        }
        out.sort_unstable();
        out
    }

    /// The nearest lane of `kind` to `p`: (lane, distance along it, distance to it). With
    /// the grid built, only lanes within about a cell of `p` are considered - enough for
    /// everything that asks where a vehicle or a person stands.
    pub fn nearest_lane(&self, p: DVec3, kind: LaneKind) -> Option<(usize, f32, f64)> {
        if !self.grid.is_empty() {
            if let Some(best) = self.nearest_lane_near(p, kind) {
                return Some(best);
            }
        }
        let mut best: Option<(usize, f32, f64)> = None;
        for (i, l) in self.lanes.iter().enumerate() {
            if l.kind != kind {
                continue;
            }
            for k in 0..l.points.len().saturating_sub(1) {
                let a = l.points[k];
                let b = l.points[k + 1];
                let ab = b - a;
                let t = ((p - a).dot(ab) / ab.length_squared().max(1e-9)).clamp(0.0, 1.0);
                let q = a + ab * t;
                let d = (q - p).length();
                if best.map(|b| d < b.2).unwrap_or(true) {
                    let s = l.dist[k] + (l.dist[k + 1] - l.dist[k]) * t as f32;
                    best = Some((i, s, d));
                }
            }
        }
        best
    }

    /// Like [`nearest_lane`](Self::nearest_lane), but only among the lanes in the grid cells
    /// around `p` (a cell is [`GRID_CELL`]): None when there are none there, never a search
    /// of the whole network.
    pub fn nearest_lane_near(&self, p: DVec3, kind: LaneKind) -> Option<(usize, f32, f64)> {
        let (cx, cy) = Self::grid_cell(p);
        let mut best: Option<(usize, f32, f64)> = None;
        let mut seen: Vec<usize> = Vec::new();
        for dx in -1..=1 {
            for dy in -1..=1 {
                let Some(list) = self.grid.get(&(cx + dx, cy + dy)) else {
                    continue;
                };
                for &i in list {
                    if seen.contains(&i) {
                        continue;
                    }
                    seen.push(i);
                    let l = &self.lanes[i];
                    if l.kind != kind {
                        continue;
                    }
                    if let Some((s, d)) = l.nearest_point(p) {
                        if best.map(|b| d < b.2).unwrap_or(true) {
                            best = Some((i, s, d));
                        }
                    }
                }
            }
        }
        best
    }

    /// The nearest lane of `kind` within `max_dist` of `p` that runs within `max_turn`
    /// degrees of `heading` there (a vehicle's own lane, not the one beside it going the
    /// other way): (lane, distance along it, distance to it).
    pub fn lane_along(
        &self,
        p: DVec3,
        heading: f64,
        kind: LaneKind,
        max_dist: f64,
        max_turn: f64,
    ) -> Option<(usize, f32, f64)> {
        let (cx, cy) = Self::grid_cell(p);
        let mut best: Option<(usize, f32, f64)> = None;
        let mut seen: Vec<usize> = Vec::new();
        for dx in -1..=1 {
            for dy in -1..=1 {
                let Some(list) = self.grid.get(&(cx + dx, cy + dy)) else {
                    continue;
                };
                for &i in list {
                    if seen.contains(&i) {
                        continue;
                    }
                    seen.push(i);
                    let l = &self.lanes[i];
                    if l.kind != kind {
                        continue;
                    }
                    let Some((s, d)) = l.nearest_point(p) else {
                        continue;
                    };
                    if d > max_dist || best.map(|b| d >= b.2).unwrap_or(false) {
                        continue;
                    }
                    let turn = omsi_geometry::util::wrap_deg(l.at(s).1 as f64 - heading);
                    if turn.abs() <= max_turn {
                        best = Some((i, s, d));
                    }
                }
            }
        }
        best
    }

    /// The grid cell `p` lies in.
    pub fn grid_cell(p: DVec3) -> (i32, i32) {
        (
            (p.x / GRID_CELL).floor() as i32,
            (p.y / GRID_CELL).floor() as i32,
        )
    }

    /// The grid cells a lane's points touch (see [`Network::build_grid`]).
    pub fn lane_cells(l: &Lane) -> Vec<(i32, i32)> {
        let mut cells: Vec<(i32, i32)> = Vec::new();
        for w in l.points.windows(2) {
            let n = ((w[1] - w[0]).truncate().length() / (GRID_CELL * 0.5))
                .ceil()
                .max(1.0) as usize;
            for k in 0..=n {
                let q = w[0].lerp(w[1], k as f64 / n as f64);
                let c = (
                    (q.x / GRID_CELL).floor() as i32,
                    (q.y / GRID_CELL).floor() as i32,
                );
                if !cells.contains(&c) {
                    cells.push(c);
                }
            }
        }
        if l.points.len() == 1 {
            cells.push((
                (l.points[0].x / GRID_CELL).floor() as i32,
                (l.points[0].y / GRID_CELL).floor() as i32,
            ));
        }
        cells
    }

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
}
