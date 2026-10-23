//! The collision world: the boxes of the scene and the queries against them.

use super::mesh::{MeshObstacle, CELL, PART_CELL};
use super::{Box3, Obb};
use glam::{DVec2, DVec3};
use hashbrown::HashMap;

#[derive(Default)]
pub struct CollisionWorld {
    pub boxes: Vec<Obb>,
    grid: HashMap<(i32, i32), Vec<usize>>,
    /// Objects that collide with their `[collision_mesh]`.
    pub meshes: Vec<MeshObstacle>,
    mesh_grid: HashMap<(i32, i32), Vec<usize>>,
}

impl CollisionWorld {
    pub fn add(&mut self, b: Obb) {
        let i = self.boxes.len();
        self.boxes.push(b);
        let r = b.radius();
        let (x0, x1) = (
            ((b.center.x - r) / CELL).floor() as i32,
            ((b.center.x + r) / CELL).floor() as i32,
        );
        let (y0, y1) = (
            ((b.center.y - r) / CELL).floor() as i32,
            ((b.center.y + r) / CELL).floor() as i32,
        );
        for y in y0..=y1 {
            for x in x0..=x1 {
                self.grid.entry((x, y)).or_default().push(i);
            }
        }
    }

    pub fn add_mesh(&mut self, m: MeshObstacle) {
        let i = self.meshes.len();
        let b = m.bounds;
        self.meshes.push(m);
        let r = b.radius();
        for y in ((b.center.y - r) / CELL).floor() as i32..=((b.center.y + r) / CELL).floor() as i32
        {
            for x in
                ((b.center.x - r) / CELL).floor() as i32..=((b.center.x + r) / CELL).floor() as i32
            {
                self.mesh_grid.entry((x, y)).or_default().push(i);
            }
        }
    }

    /// The meshes whose bounds may touch `b`.
    fn meshes_near(&self, b: &Obb) -> Vec<usize> {
        let r = b.radius();
        let mut out = Vec::new();
        for y in ((b.center.y - r) / CELL).floor() as i32..=((b.center.y + r) / CELL).floor() as i32
        {
            for x in
                ((b.center.x - r) / CELL).floor() as i32..=((b.center.x + r) / CELL).floor() as i32
            {
                for &i in self
                    .mesh_grid
                    .get(&(x, y))
                    .map(|v| v.as_slice())
                    .unwrap_or(&[])
                {
                    let o = &self.meshes[i].bounds;
                    if (o.center - b.center).length() <= r + o.radius() && !out.contains(&i) {
                        out.push(i);
                    }
                }
            }
        }
        out
    }

    /// Everything that may touch `b` as plain boxes in the world: the placed boxes and the
    /// parts of the collision meshes near it.
    pub fn obstacles_near(&self, b: &Obb) -> Vec<Obb> {
        self.obstacles_near_solid(b, Box3::from_obb(b).as_ref())
    }

    /// As [`Self::obstacles_near`], the collision meshes' faces only where they pass
    /// through `solid` (the vehicle's body as it is turned, pitched and banked).
    pub fn obstacles_near_solid(&self, b: &Obb, solid: Option<&Box3>) -> Vec<Obb> {
        let mut out: Vec<Obb> = self.near(b).into_iter().map(|i| self.boxes[i]).collect();
        for i in self.meshes_near(b) {
            out.extend(self.meshes[i].parts_near(b, solid));
        }
        out
    }

    /// How many boxes the collision meshes are made of.
    pub fn mesh_parts(&self) -> usize {
        self.meshes.iter().map(|m| m.shape.parts.len()).sum()
    }

    /// Does something building-sized (at least `min_half` m across on both axes and
    /// `min_height` m tall) stand between `a` and `b`? A box that holds `a` itself does not
    /// count (a camera in a depot hall or under a bridge).
    pub fn ray_blocked(&self, a: DVec3, b: DVec3, min_half: f64, min_height: f64) -> bool {
        self.ray_blocker(a, b, min_half, min_height).is_some()
    }

    /// The box `ray_blocked` finds in the way.
    pub fn ray_blocker(&self, a: DVec3, b: DVec3, min_half: f64, min_height: f64) -> Option<Obb> {
        let d = b - a;
        let len = d.truncate().length();
        let steps = (len / (CELL * 0.5)).ceil().max(1.0) as usize;
        let mut seen: Vec<usize> = Vec::new();
        for k in 0..=steps {
            let q = a + d * (k as f64 / steps as f64);
            let cell = ((q.x / CELL).floor() as i32, (q.y / CELL).floor() as i32);
            for &i in self.grid.get(&cell).map(|v| v.as_slice()).unwrap_or(&[]) {
                if seen.contains(&i) {
                    continue;
                }
                seen.push(i);
                let o = &self.boxes[i];
                if o.half.x < min_half || o.half.y < min_half || o.z1 - o.z0 < min_height {
                    continue;
                }
                let [r, f] = o.axes();
                let local = |p: DVec3| {
                    let rel = p.truncate() - o.center;
                    DVec2::new(rel.dot(r), rel.dot(f))
                };
                let (la, lb) = (local(a), local(b));
                if la.x.abs() <= o.half.x && la.y.abs() <= o.half.y {
                    continue;
                }
                // slab clip of the segment against the box in its own frame
                let (mut t0, mut t1) = (0.0f64, 1.0f64);
                let dl = lb - la;
                let mut hit = true;
                for (p, dd, h) in [(la.x, dl.x, o.half.x), (la.y, dl.y, o.half.y)] {
                    if dd.abs() < 1e-9 {
                        if p.abs() > h {
                            hit = false;
                            break;
                        }
                    } else {
                        let (u0, u1) = ((-h - p) / dd, (h - p) / dd);
                        t0 = t0.max(u0.min(u1));
                        t1 = t1.min(u0.max(u1));
                        if t0 > t1 {
                            hit = false;
                            break;
                        }
                    }
                }
                // and the line of sight passes through the box's height there (not over a
                // roof, not under a bridge deck)
                let (za, zb) = (a.z + d.z * t0, a.z + d.z * t1);
                if hit && za.min(zb) < o.z1 && za.max(zb) > o.z0 {
                    return Some(*o);
                }
            }
        }
        // a collision mesh counts by its size as a whole and blocks where the segment meets
        // one of its faces
        let span = Obb {
            center: (a.truncate() + b.truncate()) * 0.5,
            half: DVec2::new(0.1, len * 0.5 + 0.1),
            heading: d.x.atan2(d.y),
            z0: a.z.min(b.z),
            z1: a.z.max(b.z),
            velocity: DVec2::ZERO,
            mass: 0.0,
            pole: None,
            id: -1,
        };
        for i in self.meshes_near(&span) {
            let m = &self.meshes[i];
            let o = &m.bounds;
            if o.half.x < min_half || o.half.y < min_half || o.z1 - o.z0 < min_height {
                continue;
            }
            let steps = (len / PART_CELL).ceil().max(1.0) as usize;
            for k in 0..steps {
                let (p0, p1) = (
                    a + d * (k as f64 / steps as f64),
                    a + d * ((k + 1) as f64 / steps as f64),
                );
                let piece = Obb {
                    center: (p0.truncate() + p1.truncate()) * 0.5,
                    half: DVec2::new(0.05, (len / steps as f64) * 0.5 + 0.05),
                    heading: span.heading,
                    z0: p0.z.min(p1.z),
                    z1: p0.z.max(p1.z),
                    ..span
                };
                if let Some(p) = m
                    .parts_near(&piece, None)
                    .find(|p| segment_through(p0, p1, p))
                {
                    return Some(p);
                }
            }
        }
        None
    }

    /// Indices of the boxes that may touch `b` (a box listed in several cells once).
    pub fn near(&self, b: &Obb) -> Vec<usize> {
        let r = b.radius();
        let (x0, x1) = (
            ((b.center.x - r) / CELL).floor() as i32,
            ((b.center.x + r) / CELL).floor() as i32,
        );
        let (y0, y1) = (
            ((b.center.y - r) / CELL).floor() as i32,
            ((b.center.y + r) / CELL).floor() as i32,
        );
        let mut out = Vec::new();
        for y in y0..=y1 {
            for x in x0..=x1 {
                if let Some(list) = self.grid.get(&(x, y)) {
                    for &i in list {
                        let o = &self.boxes[i];
                        if (o.center - b.center).length() <= r + o.radius() && !out.contains(&i) {
                            out.push(i);
                        }
                    }
                }
            }
        }
        out
    }

    /// First box (or collision mesh face) overlapping `b`.
    pub fn hit(&self, b: &Obb) -> Option<Obb> {
        self.obstacles_near(b).into_iter().find(|o| o.overlaps(b))
    }
}

/// Does the segment `a`-`b` pass through the box (in plan view within its footprint, and
/// within its heights there)?
fn segment_through(a: DVec3, b: DVec3, o: &Obb) -> bool {
    let [r, f] = o.axes();
    let local = |p: DVec3| {
        let rel = p.truncate() - o.center;
        DVec2::new(rel.dot(r), rel.dot(f))
    };
    let (la, lb) = (local(a), local(b));
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    let dl = lb - la;
    for (p, dd, h) in [(la.x, dl.x, o.half.x), (la.y, dl.y, o.half.y)] {
        if dd.abs() < 1e-9 {
            if p.abs() > h {
                return false;
            }
        } else {
            let (u0, u1) = ((-h - p) / dd, (h - p) / dd);
            t0 = t0.max(u0.min(u1));
            t1 = t1.min(u0.max(u1));
            if t0 > t1 {
                return false;
            }
        }
    }
    let (za, zb) = (a.z + (b.z - a.z) * t0, a.z + (b.z - a.z) * t1);
    za.min(zb) < o.z1 && za.max(zb) > o.z0
}
