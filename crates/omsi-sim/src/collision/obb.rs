//! Oriented boxes: the box test, its contact query and the impact height.

use super::polygon::{centroid, clip};
use glam::{DVec2, DVec3};

#[derive(Debug, Clone, Copy)]
pub struct Obb {
    pub center: DVec2,
    /// Half extents: x across, y along the heading.
    pub half: DVec2,
    /// Heading in radians (clockwise from north, like vehicles).
    pub heading: f64,
    pub z0: f64,
    pub z1: f64,
    /// Ground velocity (m/s) of a moving obstacle (an AI vehicle); zero for scenery.
    pub velocity: DVec2,
    /// Mass (kg) of a moving obstacle; zero for scenery, which does not give way.
    pub mass: f32,
    /// `[crashmode_pole]` (mass in t, break load in kN): a sign or a lamp post that
    /// breaks off at its foot when a vehicle runs into it, instead of standing like a wall.
    pub pole: Option<(f32, f32)>,
    /// Map object id of a placed obstacle; a moving one has `-2 - key` of its own key, and
    /// -1 is anything else.
    pub id: i64,
}

/// How far over the bottom of the height range two bodies share their bumpers meet (m).
const BUMPER: f64 = 0.5;

/// The height (world) at which a hit between two bodies sharing the heights `z0..z1` is
/// reported to the scripts as `coll_pos_z`: near the bottom of the range, where a bumper
/// or a frame meets the other body, not in its middle - the stock collision scripts of the
/// SD200, SD202 and NL202 only damage the engine and the drivetrain below 1.10 m.
pub fn impact_height(z0: f64, z1: f64) -> f64 {
    z0 + (0.5 * (z1 - z0)).clamp(0.0, BUMPER)
}

/// Where two boxes touch: push the tested box by `normal * depth` to separate them.
#[derive(Debug, Clone, Copy)]
pub struct Contact {
    /// Unit vector in the ground plane from the obstacle towards the tested box.
    pub normal: DVec2,
    pub depth: f64,
    /// Middle of the overlap in the ground plane.
    pub point: DVec2,
    /// Height range the two share.
    pub z0: f64,
    pub z1: f64,
}

impl Obb {
    /// From an OMSI `[boundingbox] w l h cx cy cz` in a frame at `pos` turned by `heading_deg`.
    pub fn from_box(bb: [f32; 6], pos: DVec3, heading_deg: f64) -> Obb {
        let h = heading_deg.to_radians();
        let (sh, ch) = (h.sin(), h.cos());
        let (cx, cy) = (bb[3] as f64, bb[4] as f64);
        // local (x right, y forward) → world for a clockwise heading
        let world = DVec2::new(pos.x + cx * ch + cy * sh, pos.y - cx * sh + cy * ch);
        Obb {
            center: world,
            half: DVec2::new(bb[0] as f64 * 0.5, bb[1] as f64 * 0.5),
            heading: h,
            z0: pos.z + bb[5] as f64 - bb[2] as f64 * 0.5,
            z1: pos.z + bb[5] as f64 + bb[2] as f64 * 0.5,
            velocity: DVec2::ZERO,
            mass: 0.0,
            pole: None,
            id: -1,
        }
    }

    /// A small upright box (the camera's probe).
    pub fn point(p: DVec3, half: f64) -> Obb {
        Obb {
            center: p.truncate(),
            half: DVec2::splat(half),
            heading: 0.0,
            z0: p.z - half,
            z1: p.z + half,
            velocity: DVec2::ZERO,
            mass: 0.0,
            pole: None,
            id: -1,
        }
    }

    /// A moving obstacle: its ground velocity (m/s), its mass (kg) and a key that stays the
    /// same from frame to frame (the AI car's id).
    pub fn moving(mut self, v: DVec2, mass: f32, key: u64) -> Obb {
        self.velocity = v;
        self.mass = mass;
        self.id = -2 - key as i64;
        self
    }

    pub fn axes(&self) -> [DVec2; 2] {
        let (s, c) = (self.heading.sin(), self.heading.cos());
        // "right" and "forward" of the box
        [DVec2::new(c, -s), DVec2::new(s, c)]
    }

    pub fn corners(&self) -> [DVec2; 4] {
        let [r, f] = self.axes();
        let (hx, hy) = (self.half.x, self.half.y);
        [
            self.center + r * hx + f * hy,
            self.center - r * hx + f * hy,
            self.center - r * hx - f * hy,
            self.center + r * hx - f * hy,
        ]
    }

    fn interval(&self, axis: DVec2) -> (f64, f64) {
        let [r, f] = self.axes();
        let c = self.center.dot(axis);
        let e = self.half.x * r.dot(axis).abs() + self.half.y * f.dot(axis).abs();
        (c - e, c + e)
    }

    /// Separating axis test in 2D and a height overlap.
    pub fn overlaps(&self, o: &Obb) -> bool {
        if self.z1 < o.z0 || o.z1 < self.z0 {
            return false;
        }
        for axis in self.axes().into_iter().chain(o.axes()) {
            let (min_a, max_a) = self.interval(axis);
            let (min_b, max_b) = o.interval(axis);
            if max_a < min_b || max_b < min_a {
                return false;
            }
        }
        true
    }

    /// How `self` touches the obstacle `o`, if it does. The push-out direction is the axis
    /// that needs the shortest way *out* - measured to whichever side is nearer, not by the
    /// width of the overlap, or a thin post in front of the bumper would be "separated"
    /// sideways by its own width and the bus would slip past it.
    pub fn contact(&self, o: &Obb) -> Option<Contact> {
        if self.z1 <= o.z0 || o.z1 <= self.z0 {
            return None;
        }
        let mut best: Option<(f64, DVec2)> = None;
        for axis in self.axes().into_iter().chain(o.axes()) {
            let (min_a, max_a) = self.interval(axis);
            let (min_b, max_b) = o.interval(axis);
            if max_a <= min_b || max_b <= min_a {
                return None;
            }
            // move self along +axis by (max_b - min_a), or along -axis by (max_a - min_b)
            let (d, n) = if max_b - min_a < max_a - min_b {
                (max_b - min_a, axis)
            } else {
                (max_a - min_b, -axis)
            };
            if best.map(|b| d < b.0).unwrap_or(true) {
                best = Some((d, n));
            }
        }
        let (depth, normal) = best?;
        // clipped around our own centre: products of map coordinates (four million metres
        // north on Spandau) drown a small overlap's area in rounding noise, and the contact
        // came out kilometres away
        let local = |c: [DVec2; 4]| c.map(|p| p - self.center);
        let point = centroid(&clip(&local(self.corners()), &local(o.corners())))
            .map(|p| p + self.center)
            .unwrap_or((self.center + o.center) * 0.5);
        Some(Contact {
            normal,
            depth,
            point,
            z0: self.z0.max(o.z0),
            z1: self.z1.min(o.z1),
        })
    }

    /// Separating axis test in plan view only.
    pub fn overlaps_plan(&self, o: &Obb) -> bool {
        self.separation(o) < 0.0
    }

    pub fn radius(&self) -> f64 {
        self.half.length()
    }

    /// A box on the ground from a vehicle's origin, heading (deg), the distances from the
    /// origin to its front and rear bumpers and its half width.
    pub fn vehicle(origin: DVec2, heading_deg: f64, front: f64, rear: f64, half_width: f64) -> Obb {
        let h = heading_deg.to_radians();
        let fwd = DVec2::new(h.sin(), h.cos());
        Obb {
            center: origin + fwd * ((front - rear) * 0.5),
            half: DVec2::new(half_width, (front + rear) * 0.5),
            heading: h,
            z0: f64::MIN,
            z1: f64::MAX,
            velocity: DVec2::ZERO,
            mass: 0.0,
            pole: None,
            id: -1,
        }
    }

    /// How far apart two boxes are in plan view (heights ignored): the widest gap along
    /// one of the four separating axes (m), or, when they overlap, minus the shallowest
    /// depth. A positive value is a lower bound of the true distance (exact where a face
    /// is nearest), which is what a clearance check wants.
    pub fn separation(&self, o: &Obb) -> f64 {
        let d = o.center - self.center;
        let (ra, fa) = (self.axes()[0], self.axes()[1]);
        let (rb, fb) = (o.axes()[0], o.axes()[1]);
        let extent = |r: DVec2, f: DVec2, half: DVec2, axis: DVec2| {
            r.dot(axis).abs() * half.x + f.dot(axis).abs() * half.y
        };
        let mut best = f64::MIN;
        for axis in [ra, fa, rb, fb] {
            let gap =
                d.dot(axis).abs() - extent(ra, fa, self.half, axis) - extent(rb, fb, o.half, axis);
            best = best.max(gap);
        }
        best
    }
}
