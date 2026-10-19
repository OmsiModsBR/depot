//! Where lanes cross or merge, and who gives way to whom there.

use glam::DVec3;
use hashbrown::HashMap;

use super::{Network, GRID_CELL};
use crate::traffic::geometry::wrap_deg;
use crate::traffic::lane::{Lane, LaneKind};

/// Where two lanes of a junction meet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Crossing {
    pub other: usize,
    /// Distance along this lane and along the other one to the meeting point.
    pub at: f32,
    pub other_at: f32,
    /// Both lanes end in the same place (one road runs into the other) rather than crossing.
    pub merge: bool,
    /// The meeting place: how far before and after `at` this lane's centre line stays
    /// closer than `MEET_DIST` to the other's (where two vehicles on them would touch),
    /// and the same along the other lane. Two paths that cross at a shallow angle, or two
    /// turns that bend towards each other, meet over metres, not at a point.
    pub before: f32,
    pub after: f32,
    pub other_before: f32,
    pub other_after: f32,
}

/// Two vehicles whose centre lines come closer than this (m) touch: the half widths of two
/// buses.
pub const MEET_DIST: f64 = 2.6;

/// A meeting place reaches at most this far either side of the crossing point (m).
const MEET_MAX: f32 = 14.0;

/// How far before and after distance `at` of lane `a` its centre line stays within
/// `MEET_DIST` of lane `b`'s (in half-metre steps, at least the half metre around the point).
fn meeting_extent(a: &Lane, at: f32, b: &Lane) -> (f32, f32) {
    let near = |s: f32| {
        b.nearest_point(a.at(s).0)
            .map(|(_, d)| d < MEET_DIST)
            .unwrap_or(false)
    };
    let mut before = 0.5f32;
    while before < MEET_MAX && at - before >= 0.0 && near(at - before) {
        before += 0.5;
    }
    let mut after = 0.5f32;
    while after < MEET_MAX && at + after <= a.length() && near(at + after) {
        after += 0.5;
    }
    (before, after)
}

/// Where two polylines cross: the distance along each (ends excluded).
fn polyline_crossing(a: &Lane, b: &Lane) -> Option<(f32, f32)> {
    for (i, pa) in a.points.windows(2).enumerate() {
        for (j, pb) in b.points.windows(2).enumerate() {
            let (p, r) = (pa[0].truncate(), (pa[1] - pa[0]).truncate());
            let (q, s) = (pb[0].truncate(), (pb[1] - pb[0]).truncate());
            let den = r.perp_dot(s);
            if den.abs() < 1e-9 {
                continue;
            }
            let t = (q - p).perp_dot(s) / den;
            let u = (q - p).perp_dot(r) / den;
            if !(0.0..=1.0).contains(&t) || !(0.0..=1.0).contains(&u) {
                continue;
            }
            let sa = a.dist[i] + (a.dist[i + 1] - a.dist[i]) * t as f32;
            let sb = b.dist[j] + (b.dist[j + 1] - b.dist[j]) * u as f32;
            // meeting at an end is a joint (or a fork), not a crossing
            let inner = |s: f32, l: &Lane| s > 0.3 && s < l.length() - 0.3;
            if inner(sa, a) && inner(sb, b) {
                return Some((sa, sb));
            }
        }
    }
    None
}

impl Network {
    /// Geometric conflicts between the lanes of each crossing object (paths that intersect
    /// or end at the same point, with where they meet), and the footpaths crossing its
    /// street lanes. AI cars sort out who goes first from these (`Network::must_yield`).
    pub fn compute_conflicts(&mut self) {
        self.conflicts = vec![Vec::new(); self.lanes.len()];
        self.crossings = vec![Vec::new(); self.lanes.len()];
        self.walks = vec![Vec::new(); self.lanes.len()];
        let (n, walks) = self.conflicts_from(0);
        log::info!(
            "path network: {n} conflicting lane pairs at crossings, {walks} footpath crossings"
        );
    }

    /// `conflicts`, `crossings` and `walks` of the junction objects whose lanes start at
    /// index `first` (all lanes of an object come with its tile, so a grown network only
    /// needs its new objects looked at). Returns the pairs and footpath crossings found.
    pub(super) fn conflicts_from(&mut self, first: usize) -> (usize, usize) {
        let len = self.lanes.len();
        self.conflicts.resize(len, Vec::new());
        self.crossings.resize(len, Vec::new());
        self.walks.resize(len, Vec::new());
        let mut by_object: HashMap<((i32, i32), i64), Vec<usize>> = HashMap::new();
        let mut walks_by_object: HashMap<((i32, i32), i64), Vec<usize>> = HashMap::new();
        for (i, l) in self.lanes.iter().enumerate().skip(first) {
            if l.source != 2 {
                continue;
            }
            let Some(k) = l.key else { continue };
            match l.kind {
                // a level crossing is a junction of a road and a railway
                LaneKind::Street | LaneKind::Rail => {
                    by_object.entry((k.tile, k.id)).or_default().push(i)
                }
                LaneKind::Sidewalk => walks_by_object.entry((k.tile, k.id)).or_default().push(i),
                LaneKind::Air => {}
            }
        }
        let mut n = 0;
        for (key, lanes) in &by_object {
            for (x, &i) in lanes.iter().enumerate() {
                for &j in &lanes[x + 1..] {
                    let (a, b) = (&self.lanes[i], &self.lanes[j]);
                    let same_path = a.key.map(|k| k.path) == b.key.map(|k| k.path);
                    if same_path || (a.kind == LaneKind::Rail && b.kind == LaneKind::Rail) {
                        continue;
                    }
                    // one runs into the other: the same end, or ends a car's width apart that
                    // lead into the same lane (two turns converging on one exit)
                    let ends = (a.end() - b.end()).truncate().length();
                    let joint = ends < 1.5
                        || (ends < MEET_DIST && a.next.iter().any(|n| b.next.contains(n)));
                    let merge = joint && (a.start() - b.start()).truncate().length() > 3.0;
                    let place = if merge {
                        Some((a.length(), b.length()))
                    } else {
                        polyline_crossing(a, b)
                    };
                    // `[blockpath]`: the object says the two are in each other's way even
                    // where their lines do not cross - the whole of both is the meeting place
                    let (pa, pb) = (
                        a.key.map(|k| k.path).unwrap_or(u16::MAX),
                        b.key.map(|k| k.path).unwrap_or(u16::MAX),
                    );
                    if place.is_none() && (a.blocks.contains(&pb) || b.blocks.contains(&pa)) {
                        let (la, lb) = (a.length(), b.length());
                        self.conflicts[i].push(j);
                        self.conflicts[j].push(i);
                        self.crossings[i].push(Crossing {
                            other: j,
                            at: la * 0.5,
                            other_at: lb * 0.5,
                            merge: false,
                            before: la * 0.5,
                            after: la * 0.5,
                            other_before: lb * 0.5,
                            other_after: lb * 0.5,
                        });
                        self.crossings[j].push(Crossing {
                            other: i,
                            at: lb * 0.5,
                            other_at: la * 0.5,
                            merge: false,
                            before: lb * 0.5,
                            after: lb * 0.5,
                            other_before: la * 0.5,
                            other_after: la * 0.5,
                        });
                        n += 1;
                        continue;
                    }
                    if let Some((sa, sb)) = place {
                        // (after a joint the two are in one lane, and follow each other)
                        let (ab, aa) = if merge {
                            (meeting_extent(a, sa, b).0, 0.5)
                        } else {
                            meeting_extent(a, sa, b)
                        };
                        let (bb, ba) = if merge {
                            (meeting_extent(b, sb, a).0, 0.5)
                        } else {
                            meeting_extent(b, sb, a)
                        };
                        self.conflicts[i].push(j);
                        self.conflicts[j].push(i);
                        self.crossings[i].push(Crossing {
                            other: j,
                            at: sa,
                            other_at: sb,
                            merge,
                            before: ab,
                            after: aa,
                            other_before: bb,
                            other_after: ba,
                        });
                        self.crossings[j].push(Crossing {
                            other: i,
                            at: sb,
                            other_at: sa,
                            merge,
                            before: bb,
                            after: ba,
                            other_before: ab,
                            other_after: aa,
                        });
                        n += 1;
                    }
                }
            }
            if let Some(walks) = walks_by_object.get(key) {
                for &i in lanes
                    .iter()
                    .filter(|&&i| self.lanes[i].kind == LaneKind::Street)
                {
                    for &w in walks {
                        if let Some((sa, sw)) = polyline_crossing(&self.lanes[i], &self.lanes[w]) {
                            self.walks[i].push((w, sa, sw));
                        }
                    }
                }
            }
        }
        let walks: usize = self.walks[first..].iter().map(|w| w.len()).sum();
        (n, walks)
    }

    /// Must a vehicle on lane `a` let one on lane `b` go first where they meet? The path with
    /// the higher `[rule] priority` goes first; between equals, a left turn waits for the
    /// oncoming traffic, and otherwise the one coming from the right has the right of way
    /// (the German "rechts vor links" that the stock maps rely on wherever they set no
    /// priorities) - mirrored on a left-hand-traffic map. A train always goes first.
    pub fn must_yield(&self, a: usize, b: usize) -> bool {
        let (la, lb) = (&self.lanes[a], &self.lanes[b]);
        if la.kind == LaneKind::Rail {
            return false;
        }
        if lb.kind == LaneKind::Rail {
            return true;
        }
        if (la.priority - lb.priority).abs() > 0.5 {
            return la.priority < lb.priority;
        }
        let rel = wrap_deg(lb.start_heading() - la.start_heading());
        // (the turn across the oncoming traffic: left, or right when driving on the left)
        let across = if self.left_hand { 2 } else { 1 };
        if rel.abs() > 135.0 {
            // oncoming
            return la.turn == across && lb.turn != across;
        }
        // `b` comes from the right when it travels to the left of `a`'s way (from the left,
        // travelling to its right, on a left-hand-traffic map)
        let rel = if self.left_hand { -rel } else { rel };
        if rel < -30.0 {
            return true;
        }
        if rel > 30.0 {
            return false;
        }
        // side by side from the same direction: the one turning across the other waits
        la.turn != 0 && lb.turn == 0
    }

    /// The lane beside `lane` at `s` that carries the traffic the other way (the other half
    /// of a two-way street): (lane, distance along it at the same place, how far its middle
    /// lies over to the oncoming side - the left, or the right on a left-hand-traffic map;
    /// `oncoming_sign` turns it into a lateral offset).
    pub fn opposite(&self, lane: usize, s: f32) -> Option<(usize, f32, f32)> {
        let l = self.lanes.get(lane)?;
        let (p, h) = l.at(s);
        let hr = (h as f64).to_radians();
        let left = DVec3::new(-hr.cos(), hr.sin(), 0.0);
        let left = if self.left_hand { -left } else { left };
        let cx = (p.x / GRID_CELL).floor() as i32;
        let cy = (p.y / GRID_CELL).floor() as i32;
        let mut best: Option<(usize, f32, f32)> = None;
        let mut seen: Vec<usize> = Vec::new();
        for dx in -1..=1 {
            for dy in -1..=1 {
                for &i in self
                    .grid
                    .get(&(cx + dx, cy + dy))
                    .map(|v| v.as_slice())
                    .unwrap_or(&[])
                {
                    if i == lane || seen.contains(&i) {
                        continue;
                    }
                    seen.push(i);
                    let o = &self.lanes[i];
                    if o.kind != LaneKind::Street || o.no_cars {
                        continue;
                    }
                    let Some((os, d)) = o.nearest_point(p) else {
                        continue;
                    };
                    if d > 7.0 || d < 1.5 {
                        continue;
                    }
                    let (q, oh) = o.at(os);
                    let side = (q - p).dot(left) as f32;
                    if side < 1.5 || wrap_deg(oh - h).abs() < 150.0 {
                        continue;
                    }
                    if best.map(|b| side < b.2).unwrap_or(true) {
                        best = Some((i, os, side));
                    }
                }
            }
        }
        best
    }

    /// The lanes traffic comes from towards distance `s` of `lane`, walked backwards over
    /// joints and through junctions for up to `within` metres: (lane, offset, the lane it
    /// leads into on the way to `lane`). A vehicle at distance `x` along such a lane is at
    /// `x + offset` in `lane`'s own distances (negative before its start); the first entry
    /// is `lane` itself with offset 0. Where several ways lead to one lane, the shortest
    /// counts. At most `max` lanes.
    pub fn upstream(
        &self,
        lane: usize,
        s: f32,
        within: f32,
        max: usize,
    ) -> Vec<(usize, f32, Option<usize>)> {
        let mut out: Vec<(usize, f32, Option<usize>)> = vec![(lane, 0.0, None)];
        if lane >= self.lanes.len() {
            return out;
        }
        // nearest first (a junction's lanes can be reached two ways)
        let mut done = vec![false];
        while let Some(k) = (0..out.len())
            .filter(|&k| !done[k])
            .max_by(|&a, &b| out[a].1.total_cmp(&out[b].1))
        {
            done[k] = true;
            let (l, off, _) = out[k];
            // this lane starts `s - off` metres before the place: nothing before it is in reach
            if s - off > within {
                continue;
            }
            for &p in self.prev.get(l).map(|v| v.as_slice()).unwrap_or(&[]) {
                if p == lane || self.lanes[p].kind != self.lanes[lane].kind {
                    continue;
                }
                let o = off - self.lanes[p].length();
                match out.iter().position(|e| e.0 == p) {
                    Some(e) => {
                        if o > out[e].1 && !done[e] {
                            out[e] = (p, o, Some(l));
                        }
                    }
                    None => {
                        if out.len() < max {
                            out.push((p, o, Some(l)));
                            done.push(false);
                        }
                    }
                }
            }
        }
        out
    }
}
