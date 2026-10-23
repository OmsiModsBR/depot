//! Obstacles from a mesh: the triangles cut into boxes and the mesh grid.

use super::polygon::clip_z;
use super::{Box3, Obb};
use glam::{DVec2, DVec3};
use hashbrown::HashMap;

pub(super) const CELL: f64 = 50.0;

/// Cell of a mesh shape's own grid (m).
pub(crate) const PART_CELL: f64 = 8.0;

/// A part no thinner than this (m): a wall's triangle has no depth of its own.
const PART_HALF_MIN: f64 = 0.05;

/// A sloping triangle whose box would hold more than this much air (m³ of footprint times
/// height) is cut into bands: a roof pitched over a 10 m house would otherwise be a solid
/// block from its eaves up to its ridge.
const PART_SLACK: f64 = 0.25;

/// How high a band of a sloping triangle is (m): a bus's floor clears a ramp it drives on
/// by a good deal more.
const PART_BAND: f64 = 0.1;

/// A `[collision_mesh]` in its object's own frame (x right, y forward, z up), as what OMSI
/// hands to ODE: a triangle mesh, not its extents. Each triangle becomes a thin box (a
/// wall's is a slab along it, a floor's a flat plate), so a vehicle meets the faces it
/// really runs into - one box around the whole mesh stood as a wall over every road
/// between the buildings of a housing estate, and around the whole Heerstraße bridge.
#[derive(Debug, Default)]
pub struct MeshShape {
    /// Boxes in the object's frame: `center` and `heading` relative to its origin and
    /// heading, `z0..z1` over its origin.
    pub parts: Vec<Obb>,
    /// The triangle each part stands for (the part's band of it, `z0..z1`, is the face).
    part_tri: Vec<u32>,
    tris: Vec<[glam::Vec3; 3]>,
    pub(crate) grid: HashMap<(i32, i32), Vec<u32>>,
    /// Extents in the object's frame.
    pub lo: DVec3,
    pub hi: DVec3,
}

impl MeshShape {
    /// From triangles in the object's frame. Triangles whose top stays below `min_top` over
    /// the origin (a kerb, a floor at ground level) are no obstacle.
    pub fn from_triangles(tris: impl Iterator<Item = [DVec3; 3]>, min_top: f64) -> MeshShape {
        let mut shape = MeshShape {
            lo: DVec3::splat(f64::MAX),
            hi: DVec3::splat(f64::MIN),
            ..Default::default()
        };
        let mut seen: hashbrown::HashSet<[i64; 6]> = hashbrown::HashSet::new();
        for t in tris {
            let mut out = Vec::new();
            triangle_parts(t, &mut out);
            let ti = shape.tris.len() as u32;
            let mut used = false;
            for part in out {
                if part.z1 <= min_top {
                    continue;
                }
                // the two halves of a flat wall make the same slab
                let q = |v: f64| (v * 100.0).round() as i64;
                if !seen.insert([
                    q(part.center.x),
                    q(part.center.y),
                    q(part.half.x),
                    q(part.half.y),
                    q(part.z0),
                    q(part.z1),
                ]) {
                    continue;
                }
                shape.add(part);
                shape.part_tri.push(ti);
                used = true;
            }
            if used {
                shape.tris.push(t.map(|p| p.as_vec3()));
            }
        }
        shape
    }

    /// Part `i`'s piece of its triangle, in the object's frame.
    fn face(&self, i: usize) -> Vec<DVec3> {
        let p = &self.parts[i];
        let t = self.tris[self.part_tri[i] as usize].map(|v| v.as_dvec3());
        // a hair beyond the band, or the two bands either side of a vertex both lose it
        clip_z(&clip_z(&t, p.z0 - 1e-4, true), p.z1 + 1e-4, false)
    }

    fn add(&mut self, b: Obb) {
        let i = self.parts.len() as u32;
        self.parts.push(b);
        let r = b.radius();
        self.lo = self
            .lo
            .min(DVec3::new(b.center.x - r, b.center.y - r, b.z0));
        self.hi = self
            .hi
            .max(DVec3::new(b.center.x + r, b.center.y + r, b.z1));
        for y in ((b.center.y - r) / PART_CELL).floor() as i32
            ..=((b.center.y + r) / PART_CELL).floor() as i32
        {
            for x in ((b.center.x - r) / PART_CELL).floor() as i32
                ..=((b.center.x + r) / PART_CELL).floor() as i32
            {
                self.grid.entry((x, y)).or_default().push(i);
            }
        }
    }

    /// Indices of the parts within `r` of the local point `c` (each once).
    pub(crate) fn near(&self, c: DVec2, r: f64) -> Vec<u32> {
        let mut out: Vec<u32> = Vec::new();
        let (x0, y0) = (
            ((c.x - r) / PART_CELL).floor() as i32,
            ((c.y - r) / PART_CELL).floor() as i32,
        );
        for y in y0..=((c.y + r) / PART_CELL).floor() as i32 {
            for x in x0..=((c.x + r) / PART_CELL).floor() as i32 {
                for &i in self.grid.get(&(x, y)).map(|v| v.as_slice()).unwrap_or(&[]) {
                    let p = &self.parts[i as usize];
                    let pr = p.radius();
                    // listed in every cell it covers: taken in the first of them met here
                    let first = (((p.center.x - pr) / PART_CELL).floor() as i32).max(x0) == x
                        && (((p.center.y - pr) / PART_CELL).floor() as i32).max(y0) == y;
                    if first && (p.center - c).length() <= r + pr {
                        out.push(i);
                    }
                }
            }
        }
        out
    }
}

/// The boxes standing in for one triangle: one when it is upright (a wall) or flat (a
/// floor, a roof), else a slab per band of its height, each laid along the band - a road
/// ramping up a bridge, cut into one box, stood 0.4 m over its own foot, where the bus
/// that drives up it runs into the box.
fn triangle_parts(t: [DVec3; 3], out: &mut Vec<Obb>) {
    let q = t.map(|p| p.truncate());
    let area = 0.5 * (q[1] - q[0]).perp_dot(q[2] - q[0]).abs();
    let (z0, z1) = (
        t[0].z.min(t[1].z).min(t[2].z),
        t[0].z.max(t[1].z).max(t[2].z),
    );
    let mut part = |poly: &[DVec3], along: DVec2| {
        let heading = if along.length() < 1e-6 {
            0.0
        } else {
            along.x.atan2(along.y)
        };
        let mut b = Obb {
            center: DVec2::ZERO,
            half: DVec2::ZERO,
            heading,
            z0: f64::MAX,
            z1: f64::MIN,
            velocity: DVec2::ZERO,
            mass: 0.0,
            pole: None,
            id: -1,
        };
        let [r, f] = b.axes();
        let (mut u0, mut u1, mut v0, mut v1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for p in poly {
            let p2 = p.truncate();
            u0 = u0.min(p2.dot(r));
            u1 = u1.max(p2.dot(r));
            v0 = v0.min(p2.dot(f));
            v1 = v1.max(p2.dot(f));
            b.z0 = b.z0.min(p.z);
            b.z1 = b.z1.max(p.z);
        }
        b.center = r * ((u0 + u1) * 0.5) + f * ((v0 + v1) * 0.5);
        b.half = DVec2::new(
            ((u1 - u0) * 0.5).max(PART_HALF_MIN),
            ((v1 - v0) * 0.5).max(PART_HALF_MIN),
        );
        out.push(b);
    };
    if area * (z1 - z0) <= PART_SLACK {
        // along the longest edge in plan view
        let edges = [q[1] - q[0], q[2] - q[1], q[0] - q[2]];
        let d = edges
            .into_iter()
            .max_by(|a, b| a.length_squared().total_cmp(&b.length_squared()))
            .unwrap();
        part(&t, d);
        return;
    }
    // bands between contour lines, laid along them
    let n = (t[1] - t[0]).cross(t[2] - t[0]);
    let contour = DVec2::new(-n.y, n.x);
    let bands = ((z1 - z0) / PART_BAND).ceil().clamp(1.0, 64.0) as usize;
    let dz = (z1 - z0) / bands as f64;
    for k in 0..bands {
        let (lo, hi) = (z0 + dz * k as f64, z0 + dz * (k + 1) as f64);
        let poly = clip_z(&clip_z(&t, lo, true), hi, false);
        if poly.len() >= 3 {
            part(&poly, contour);
        }
    }
}

/// A placed object's collision mesh.
#[derive(Debug, Clone)]
pub struct MeshObstacle {
    pub shape: std::sync::Arc<MeshShape>,
    pub pos: DVec3,
    /// Radians, clockwise from north.
    pub heading: f64,
    pub id: i64,
    /// Around the whole mesh, in the world.
    pub bounds: Obb,
}

impl MeshObstacle {
    pub fn new(
        shape: std::sync::Arc<MeshShape>,
        pos: DVec3,
        heading_deg: f64,
        id: i64,
    ) -> MeshObstacle {
        let (lo, hi) = (shape.lo, shape.hi);
        let size = hi - lo;
        let c = (lo + hi) * 0.5;
        let mut bounds = Obb::from_box(
            [
                size.x as f32,
                size.y as f32,
                size.z as f32,
                c.x as f32,
                c.y as f32,
                c.z as f32,
            ],
            pos,
            heading_deg,
        );
        // the f32 of a box's centre offset is fine, not so its height over a map at 4000 km
        bounds.z0 = pos.z + lo.z;
        bounds.z1 = pos.z + hi.z;
        bounds.id = id;
        MeshObstacle {
            shape,
            pos,
            heading: heading_deg.to_radians(),
            id,
            bounds,
        }
    }

    /// Object frame → world.
    fn to_world(&self, p: &Obb) -> Obb {
        let (s, c) = self.heading.sin_cos();
        let l = p.center;
        Obb {
            center: DVec2::new(
                self.pos.x + l.x * c + l.y * s,
                self.pos.y - l.x * s + l.y * c,
            ),
            heading: p.heading + self.heading,
            z0: p.z0 + self.pos.z,
            z1: p.z1 + self.pos.z,
            id: self.id,
            ..*p
        }
    }

    /// World point → object frame (plan view).
    fn to_local(&self, p: DVec2) -> DVec2 {
        let (s, c) = self.heading.sin_cos();
        let d = p - self.pos.truncate();
        DVec2::new(d.x * c - d.y * s, d.x * s + d.y * c)
    }

    /// The parts that may touch `b`, in the world; with `solid`, only those whose face
    /// really passes through that box (a pitched bus's own, which the plan-view box with the
    /// height range of all its corners overstates on a ramp).
    pub fn parts_near<'a>(
        &'a self,
        b: &Obb,
        solid: Option<&'a Box3>,
    ) -> impl Iterator<Item = Obb> + 'a {
        let (z0, z1) = (b.z0 - self.pos.z, b.z1 - self.pos.z);
        let list = if self.bounds.overlaps_plan(b) {
            self.shape.near(self.to_local(b.center), b.radius())
        } else {
            Vec::new()
        };
        list.into_iter()
            .filter(move |&i| {
                let p = &self.shape.parts[i as usize];
                p.z1 > z0 && p.z0 < z1
            })
            .filter(move |&i| {
                let Some(solid) = solid else { return true };
                let (s, c) = self.heading.sin_cos();
                let face: Vec<DVec3> = self
                    .shape
                    .face(i as usize)
                    .into_iter()
                    .map(|l| {
                        DVec3::new(
                            self.pos.x + l.x * c + l.y * s,
                            self.pos.y - l.x * s + l.y * c,
                            self.pos.z + l.z,
                        )
                    })
                    .collect();
                face.len() >= 3 && solid.meets(&face)
            })
            .map(|i| self.to_world(&self.shape.parts[i as usize]))
    }
}
