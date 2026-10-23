//! Routes over a vehicle's cabin path network (`paths.cfg`).

use glam::Vec3;

use crate::people::crowd::corridor::project_on_segment;

/// The walking network of a vehicle's cabin: `[pathpnt]`s and `[pathlink]`s.
#[derive(Debug, Clone, Default)]
pub struct PathGraph {
    pub points: Vec<Vec3>,
    adj: Vec<Vec<(usize, f32)>>,
}

impl PathGraph {
    /// `links`: (a, b, one-way) as in `paths.cfg`.
    pub fn new(points: Vec<Vec3>, links: &[(i32, i32, bool)]) -> PathGraph {
        let n = points.len();
        let mut adj = vec![Vec::new(); n];
        for &(a, b, oneway) in links {
            if a < 0 || b < 0 {
                continue;
            }
            let (a, b) = (a as usize, b as usize);
            if a >= n || b >= n || a == b {
                continue;
            }
            let d = (points[a] - points[b]).length();
            adj[a].push((b, d));
            if !oneway {
                adj[b].push((a, d));
            }
        }
        PathGraph { points, adj }
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Path point nearest `p` (height counts three times: the upper deck is not "near").
    pub fn nearest(&self, p: Vec3) -> Option<usize> {
        let d = |q: Vec3| {
            let v = q - p;
            v.x * v.x + v.y * v.y + (v.z * 3.0) * (v.z * 3.0)
        };
        self.points
            .iter()
            .enumerate()
            .filter(|(i, _)| !self.adj[*i].is_empty())
            .min_by(|a, b| d(*a.1).total_cmp(&d(*b.1)))
            .map(|(i, _)| i)
    }

    /// Shortest walk from path point `a` to path point `b` (point indices, `a` first).
    pub fn route_points(&self, a: usize, b: usize) -> Option<Vec<usize>> {
        let n = self.points.len();
        if a >= n || b >= n {
            return None;
        }
        let mut dist = vec![f32::INFINITY; n];
        let mut prev = vec![usize::MAX; n];
        let mut done = vec![false; n];
        dist[a] = 0.0;
        loop {
            let mut u = usize::MAX;
            let mut best = f32::INFINITY;
            for i in 0..n {
                if !done[i] && dist[i] < best {
                    best = dist[i];
                    u = i;
                }
            }
            if u == usize::MAX || u == b {
                break;
            }
            done[u] = true;
            for &(v, w) in &self.adj[u] {
                if dist[u] + w < dist[v] {
                    dist[v] = dist[u] + w;
                    prev[v] = u;
                }
            }
        }
        if !dist[b].is_finite() {
            return None;
        }
        let mut out = vec![b];
        let mut x = b;
        while x != a {
            x = prev[x];
            out.push(x);
        }
        out.reverse();
        Some(out)
    }

    /// Path points linked to `i` (either way).
    pub fn neighbours(&self, i: usize) -> Vec<usize> {
        let mut out: Vec<usize> = self
            .adj
            .get(i)
            .map(|a| a.iter().map(|x| x.0).collect())
            .unwrap_or_default();
        for (j, a) in self.adj.iter().enumerate() {
            if a.iter().any(|x| x.0 == i) && !out.contains(&j) {
                out.push(j);
            }
        }
        out
    }

    /// Length of the shortest walk between two path points (infinite when unconnected).
    pub fn distance(&self, a: usize, b: usize) -> f32 {
        match self.route_points(a, b) {
            Some(pts) => pts
                .windows(2)
                .map(|w| (self.points[w[0]] - self.points[w[1]]).length())
                .sum(),
            None => f32::INFINITY,
        }
    }

    /// Walk from `from` to `to` (bus frame): onto the network at the point nearest `from`,
    /// along it to the point nearest `to`, then to `to`. Points the walker is already
    /// standing on or has half passed are left out. A cabin without a network is crossed
    /// in a straight line.
    pub fn route(&self, from: Vec3, to: Vec3) -> Vec<Vec3> {
        // Onto the network where it passes closest - on a link, not at its nearest point.
        // The nearest *point* of a seat was often the one of the row behind (or across a
        // partition), and the walk to it went diagonally through the seat backs and the
        // wall; the nearest link is the aisle beside the seat.
        if let (Some(l1), Some(l2)) = (self.nearest_link(from), self.nearest_link(to)) {
            if let Some(r) = self.route_links(from, l1, l2, to) {
                return r;
            }
        }
        let (Some(a), Some(b)) = (self.nearest(from), self.nearest(to)) else {
            return vec![to];
        };
        self.route_via(from, a, b, to)
    }

    /// The link passing closest to `p` (height counts three times, as in `nearest`): its
    /// two points and the closest point on it.
    fn nearest_link(&self, p: Vec3) -> Option<(usize, usize, Vec3)> {
        let mut best: Option<(f32, usize, usize, Vec3)> = None;
        for (a, adj) in self.adj.iter().enumerate() {
            for &(b, _) in adj {
                let (pa, pb) = (self.points[a], self.points[b]);
                let ab = pb - pa;
                let len2 = ab.length_squared();
                let t = if len2 > 1e-6 {
                    ((p - pa).dot(ab) / len2).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let q = pa + ab * t;
                let v = q - p;
                let d = v.x * v.x + v.y * v.y + (v.z * 3.0) * (v.z * 3.0);
                if best.map(|b| d < b.0).unwrap_or(true) {
                    best = Some((d, a, b, q));
                }
            }
        }
        best.map(|(_, a, b, q)| (a, b, q))
    }

    /// The walk from `from` onto link `l1`, along the network and off link `l2` to `to`
    /// (None when the two links are not connected).
    fn route_links(
        &self,
        from: Vec3,
        l1: (usize, usize, Vec3),
        l2: (usize, usize, Vec3),
        to: Vec3,
    ) -> Option<Vec<Vec3>> {
        let (a1, b1, q1) = l1;
        let (a2, b2, q2) = l2;
        let mut out: Vec<Vec3> = Vec::new();
        let push = |out: &mut Vec<Vec3>, p: Vec3| {
            let last = out.last().copied().unwrap_or(from);
            if (p - last).length() > 0.05 {
                out.push(p);
            }
        };
        // (stepping onto the network only from off it: a walker on the link goes on)
        if (q1 - from).truncate().length() > 0.25 {
            push(&mut out, q1);
        }
        let same = (a1 == a2 && b1 == b2) || (a1 == b2 && b1 == a2);
        if !same {
            let mut best: Option<(f32, Vec<usize>)> = None;
            for e1 in [a1, b1] {
                for e2 in [a2, b2] {
                    let Some(pts) = self.route_points(e1, e2) else {
                        continue;
                    };
                    let along: f32 = pts
                        .windows(2)
                        .map(|w| (self.points[w[0]] - self.points[w[1]]).length())
                        .sum();
                    let total =
                        (self.points[e1] - q1).length() + along + (self.points[e2] - q2).length();
                    if best.as_ref().map(|b| total < b.0).unwrap_or(true) {
                        best = Some((total, pts));
                    }
                }
            }
            let (_, pts) = best?;
            for i in pts {
                push(&mut out, self.points[i]);
            }
        }
        if (q2 - to).truncate().length() > 0.25 {
            push(&mut out, q2);
        }
        push(&mut out, to);
        if out.is_empty() {
            out.push(to);
        }
        Some(out)
    }

    /// Like `route`, between known path points.
    pub fn route_via(&self, from: Vec3, a: usize, b: usize, to: Vec3) -> Vec<Vec3> {
        let mut out: Vec<Vec3> = match self.route_points(a, b) {
            Some(pts) => pts.into_iter().map(|i| self.points[i]).collect(),
            None => Vec::new(),
        };
        while !out.is_empty() && (out[0] - from).truncate().length() < 0.05 {
            out.remove(0);
        }
        // the walker already stands on the first leg: go straight for the second point
        if out.len() >= 2 {
            let (p0, p1) = (out[0], out[1]);
            let (_, t) = project_on_segment(
                from.truncate().as_dvec2(),
                p0.truncate().as_dvec2(),
                p1.truncate().as_dvec2(),
            );
            let q = p0 + (p1 - p0) * t as f32;
            if t > 0.0 && t < 1.0 && (q - from).truncate().length() < 0.3 {
                out.remove(0);
            }
        }
        if out
            .last()
            .map(|l| (*l - to).length() > 0.05)
            .unwrap_or(true)
        {
            out.push(to);
        }
        out
    }

    /// Length of a route starting at `from`.
    pub fn length(from: Vec3, route: &[Vec3]) -> f32 {
        let mut last = from;
        let mut total = 0.0;
        for p in route {
            total += (*p - last).truncate().length();
            last = *p;
        }
        total
    }
}
