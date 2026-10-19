//! How the lanes join up: linking lane ends by proximity, growing the network as tiles
//! stream in, and how far a car can drive on from each lane (its reach).

use glam::DVec3;
use hashbrown::HashMap;

use super::{Network, GRID_CELL};
use crate::traffic::geometry::wrap_deg;
use crate::traffic::lane::{Lane, LaneKind};

/// `Network::reach` is counted up to this far (m).
pub const REACH_MAX: f32 = 600.0;

/// A way on that ends within this distance is a dead end to a driver who has a choice (m).
pub const DEAD_END: f32 = 500.0;
impl Network {
    /// Connect lane ends to lane starts that lie within `tol` metres with a compatible heading.
    pub fn link(&mut self, tol: f64) {
        self.by_key.clear();
        for (i, l) in self.lanes.iter().enumerate() {
            if let Some(k) = l.key {
                self.by_key.entry(k).or_default().push(i);
            }
        }
        self.build_grid();
        // neighbouring lanes: paths of one spline running the same way, next to each other
        let mut by_spline: HashMap<((i32, i32), i64, bool), Vec<usize>> = HashMap::new();
        for (i, l) in self.lanes.iter().enumerate() {
            if let Some(k) = l.key {
                if l.source == 1 && matches!(l.kind, LaneKind::Street) {
                    by_spline
                        .entry((k.tile, k.id as i64, l.reversed))
                        .or_default()
                        .push(i);
                }
            }
        }
        let mut neighbours = 0usize;
        for (_, mut list) in by_spline {
            list.sort_by(|a, b| self.lanes[*a].offset.total_cmp(&self.lanes[*b].offset));
            for w in list.windows(2) {
                let (a, b) = (w[0], w[1]);
                let gap = self.lanes[b].offset - self.lanes[a].offset;
                if gap > 1.5 && gap < 6.0 {
                    // b lies at the larger offset: to the right when driving along the spline
                    if neighbours == 0 {
                        let p = self.lanes[a].start();
                        log::info!(
                            "first neighbouring lane pair at ({:.1}, {:.1}), heading {:.0}",
                            p.x,
                            p.y,
                            self.lanes[a].start_heading()
                        );
                    }
                    neighbours += 1;
                    let reversed = self.lanes[a].reversed;
                    if reversed {
                        self.lanes[a].left = Some(b);
                        self.lanes[b].right = Some(a);
                    } else {
                        self.lanes[a].right = Some(b);
                        self.lanes[b].left = Some(a);
                    }
                }
            }
        }
        log::info!(
            "lanes: {} neighbouring lane pairs (lane changes)",
            neighbours
        );
        let cell = 4.0;
        let mut grid: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for (i, l) in self.lanes.iter().enumerate() {
            let s = l.start();
            grid.entry(((s.x / cell).floor() as i64, (s.y / cell).floor() as i64))
                .or_default()
                .push(i);
        }
        let mut links = 0;
        for i in 0..self.lanes.len() {
            let e = self.lanes[i].end();
            let eh = self.lanes[i].end_heading();
            let kind = self.lanes[i].kind;
            let cx = (e.x / cell).floor() as i64;
            let cy = (e.y / cell).floor() as i64;
            let mut found = Vec::new();
            for dx in -1..=1 {
                for dy in -1..=1 {
                    if let Some(list) = grid.get(&(cx + dx, cy + dy)) {
                        for &j in list {
                            if j == i {
                                continue;
                            }
                            let l = &self.lanes[j];
                            if l.kind != kind {
                                continue;
                            }
                            let d = (l.start() - e).truncate().length();
                            let dh = wrap_deg(l.start_heading() - eh).abs();
                            if d <= tol && dh < 40.0 && (l.start().z - e.z).abs() < 3.0 {
                                found.push(j);
                            }
                        }
                    }
                }
            }
            if found.is_empty() && omsi_cfg::flags::DEBUG_UNLINKED.is_set() {
                // a lane end with a start of its kind near it that was not taken
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        for &j in grid
                            .get(&(cx + dx, cy + dy))
                            .map(|v| v.as_slice())
                            .unwrap_or(&[])
                        {
                            let l = &self.lanes[j];
                            if j != i && l.kind == kind {
                                let d = (l.start() - e).truncate().length();
                                let dh = wrap_deg(l.start_heading() - eh).abs();
                                if d < 4.0 && dh < 40.0 {
                                    log::info!("unlinked: lane {i} {:?} end ({:.2}, {:.2}, {:.2}) -> lane {j} {:?} start {:.2} m away, dz {:.2}, dh {:.1}", self.lanes[i].key, e.x, e.y, e.z, l.key, d, l.start().z - e.z, dh);
                                }
                            }
                        }
                    }
                }
            }
            links += found.len();
            self.lanes[i].next = found;
        }
        log::info!("path network: {} lanes, {} links", self.lanes.len(), links);
        self.prev = vec![Vec::new(); self.lanes.len()];
        for i in 0..self.lanes.len() {
            for &n in &self.lanes[i].next {
                if n < self.lanes.len() {
                    self.prev[n].push(i);
                }
            }
        }
        self.compute_conflicts();
        self.compute_reach();
    }

    /// `reach` of every lane: its length plus the best reach of the lanes after it. The lanes
    /// are settled from the dead ends backwards (a lane once all the lanes after it are); a
    /// lane that never settles has a loop ahead of it and can be driven on for ever.
    pub fn compute_reach(&mut self) {
        let dead = self.update_reach();
        log::info!("path network: {dead} street lanes lead into a dead end within {DEAD_END} m");
    }

    /// `compute_reach` without the log line; returns the street lanes that lead into a dead
    /// end.
    fn update_reach(&mut self) -> usize {
        let n = self.lanes.len();
        // a lane closed to cars is no way on for one that is open
        let counts = |i: usize, j: usize| !self.lanes[j].no_cars || self.lanes[i].no_cars;
        let mut waiting: Vec<usize> = (0..n)
            .map(|i| self.lanes[i].next.iter().filter(|&&j| counts(i, j)).count())
            .collect();
        let mut reach = vec![REACH_MAX; n];
        let mut settled = vec![false; n];
        let mut ready: Vec<usize> = (0..n).filter(|&i| waiting[i] == 0).collect();
        while let Some(j) = ready.pop() {
            let l = &self.lanes[j];
            let best = l
                .next
                .iter()
                .filter(|&&k| counts(j, k))
                .map(|&k| reach[k])
                .fold(0.0f32, f32::max);
            reach[j] = (l.length() + best).min(REACH_MAX);
            settled[j] = true;
            for &p in self.prev.get(j).map(|v| v.as_slice()).unwrap_or(&[]) {
                if counts(p, j) && !settled[p] {
                    waiting[p] = waiting[p].saturating_sub(1);
                    if waiting[p] == 0 {
                        ready.push(p);
                    }
                }
            }
        }
        let dead = self
            .lanes
            .iter()
            .zip(&reach)
            .filter(|(l, r)| l.kind == LaneKind::Street && **r < DEAD_END)
            .count();
        self.reach = reach;
        dead
    }
}

/// Growing the network while the map streams in: the lanes of newly loaded tiles are
/// appended (existing indices stay valid for the cars and routes that hold them) and linked
/// to what is there, with the same rules as [`Network::link`].
impl Network {
    /// Append `new` lanes and link them. Returns the range of their indices.
    pub fn extend(&mut self, new: Vec<Lane>, tol: f64) -> std::ops::Range<usize> {
        let first = self.lanes.len();
        if new.is_empty() {
            return first..first;
        }
        if first == 0 {
            self.lanes = new;
            self.link(tol);
            return 0..self.lanes.len();
        }
        self.lanes.extend(new);
        let end = self.lanes.len();
        for i in first..end {
            if let Some(k) = self.lanes[i].key {
                self.by_key.entry(k).or_default().push(i);
            }
            self.start_grid
                .entry(Self::grid_cell(self.lanes[i].start()))
                .or_default()
                .push(i);
            for c in Self::lane_cells(&self.lanes[i]) {
                self.grid.entry(c).or_default().push(i);
            }
        }
        // neighbouring lanes of the new splines
        let mut by_spline: HashMap<((i32, i32), i64, bool), Vec<usize>> = HashMap::new();
        for i in first..end {
            let l = &self.lanes[i];
            if let (Some(k), 1, LaneKind::Street) = (l.key, l.source, l.kind) {
                by_spline
                    .entry((k.tile, k.id, l.reversed))
                    .or_default()
                    .push(i);
            }
        }
        for (_, mut list) in by_spline {
            list.sort_by(|a, b| self.lanes[*a].offset.total_cmp(&self.lanes[*b].offset));
            for w in list.windows(2) {
                let (a, b) = (w[0], w[1]);
                let gap = self.lanes[b].offset - self.lanes[a].offset;
                if gap > 1.5 && gap < 6.0 {
                    if self.lanes[a].reversed {
                        self.lanes[a].left = Some(b);
                        self.lanes[b].right = Some(a);
                    } else {
                        self.lanes[a].right = Some(b);
                        self.lanes[b].left = Some(a);
                    }
                }
            }
        }
        // links: ends of new lanes to any start, ends of old lanes to new starts
        let joins = |net: &Network, from: usize, to: usize| -> bool {
            let (a, b) = (&net.lanes[from], &net.lanes[to]);
            if from == to || a.kind != b.kind {
                return false;
            }
            let (e, s) = (a.end(), b.start());
            let dh = ((b.start_heading() - a.end_heading() + 540.0) % 360.0 - 180.0).abs();
            (s - e).truncate().length() <= tol && dh < 40.0 && (s.z - e.z).abs() < 3.0
        };
        let near = |net: &Network, p: DVec3| -> Vec<usize> {
            let c = (
                (p.x / GRID_CELL).floor() as i32,
                (p.y / GRID_CELL).floor() as i32,
            );
            let mut out: Vec<usize> = Vec::new();
            for dx in -1..=1 {
                for dy in -1..=1 {
                    if let Some(list) = net.grid.get(&(c.0 + dx, c.1 + dy)) {
                        out.extend(list.iter().copied());
                    }
                }
            }
            out.sort_unstable();
            out.dedup();
            out
        };
        let mut added: Vec<(usize, usize)> = Vec::new();
        for i in first..end {
            let e = self.lanes[i].end();
            let found: Vec<usize> = near(self, e)
                .into_iter()
                .filter(|&j| joins(self, i, j))
                .collect();
            for &j in &found {
                added.push((i, j));
            }
            self.lanes[i].next = found;
            let s = self.lanes[i].start();
            for j in near(self, s) {
                if j < first && joins(self, j, i) && !self.lanes[j].next.contains(&i) {
                    self.lanes[j].next.push(i);
                    added.push((j, i));
                }
            }
        }
        self.prev.resize(end, Vec::new());
        for (from, to) in added {
            if !self.prev[to].contains(&from) {
                self.prev[to].push(from);
            }
        }
        // meeting places and footpath crossings inside the new junction objects, and how far
        // every lane now leads (a dead end may go on into the new tiles)
        self.conflicts_from(first);
        self.update_reach_from(first);
        first..end
    }

    /// Appending lanes can only change the reach of those lanes and their predecessors.
    /// An outgoing edge into the rest of the network uses its already settled reach. This
    /// avoids rewalking every loaded tile whenever the streamer adds a small batch.
    fn update_reach_from(&mut self, first: usize) {
        if self.reach.len() != first {
            self.update_reach();
            return;
        }
        let mut affected: hashbrown::HashSet<usize> = (first..self.lanes.len()).collect();
        let mut pending: Vec<usize> = affected.iter().copied().collect();
        while let Some(i) = pending.pop() {
            // The vector-based full pass is cheaper once much of the network leads
            // into this tile (for example a strongly connected city grid).
            if affected.len() > self.lanes.len() / 4 {
                self.update_reach();
                return;
            }
            for &p in &self.prev[i] {
                if affected.insert(p) {
                    pending.push(p);
                }
            }
        }
        self.reach.resize(self.lanes.len(), REACH_MAX);
        let counts = |i: usize, j: usize| !self.lanes[j].no_cars || self.lanes[i].no_cars;
        let mut waiting: HashMap<usize, usize> = affected
            .iter()
            .map(|&i| {
                (
                    i,
                    self.lanes[i]
                        .next
                        .iter()
                        .filter(|&&j| affected.contains(&j) && counts(i, j))
                        .count(),
                )
            })
            .collect();
        let mut ready: Vec<usize> = waiting
            .iter()
            .filter_map(|(&i, &n)| (n == 0).then_some(i))
            .collect();
        for &i in &affected {
            self.reach[i] = REACH_MAX;
        }
        while let Some(i) = ready.pop() {
            let l = &self.lanes[i];
            let best = l
                .next
                .iter()
                .filter(|&&j| counts(i, j))
                .map(|&j| self.reach[j])
                .fold(0.0f32, f32::max);
            self.reach[i] = (l.length() + best).min(REACH_MAX);
            for &p in &self.prev[i] {
                if counts(p, i) {
                    if let Some(n) = waiting.get_mut(&p) {
                        *n = n.saturating_sub(1);
                        if *n == 0 {
                            ready.push(p);
                        }
                    }
                }
            }
        }
    }
}
