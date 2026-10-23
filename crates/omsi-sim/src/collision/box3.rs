//! The axis-aligned box of a vehicle or an obstacle in the ground plane.

use super::Obb;
use glam::DVec3;

/// A box in space: a vehicle's body as it is turned, pitched and banked.
#[derive(Debug, Clone, Copy)]
pub struct Box3 {
    pub center: DVec3,
    /// Unit axes (right, forward, up of the body).
    pub axes: [DVec3; 3],
    pub half: DVec3,
}

impl Box3 {
    /// The upright box of a plan-view box and its height range (None when the range is
    /// open, as a vehicle's outline for the traffic).
    pub fn from_obb(b: &Obb) -> Option<Box3> {
        if !(b.z1 - b.z0).is_finite() || b.z1 - b.z0 > 1e5 {
            return None;
        }
        let [r, f] = b.axes();
        Some(Box3 {
            center: DVec3::new(b.center.x, b.center.y, (b.z0 + b.z1) * 0.5),
            axes: [r.extend(0.0), f.extend(0.0), DVec3::Z],
            half: DVec3::new(b.half.x, b.half.y, (b.z1 - b.z0) * 0.5),
        })
    }

    /// Does the convex polygon (a face) pass through the box? Separating axes: the box's
    /// three, the face's normal and the cross products of their edges.
    pub fn meets(&self, poly: &[DVec3]) -> bool {
        // round the box's centre: products of map coordinates lose the centimetres
        let pts: Vec<DVec3> = poly.iter().map(|p| *p - self.center).collect();
        let n = (pts[1] - pts[0]).cross(pts[2] - pts[0]);
        let separated = |axis: DVec3| {
            if axis.length_squared() < 1e-12 {
                return false;
            }
            let r = self.half.x * self.axes[0].dot(axis).abs()
                + self.half.y * self.axes[1].dot(axis).abs()
                + self.half.z * self.axes[2].dot(axis).abs();
            let (mut lo, mut hi) = (f64::MAX, f64::MIN);
            for p in &pts {
                let d = p.dot(axis);
                lo = lo.min(d);
                hi = hi.max(d);
            }
            lo > r || hi < -r
        };
        if self.axes.iter().any(|a| separated(*a)) || separated(n) {
            return false;
        }
        for i in 0..pts.len() {
            let e = pts[(i + 1) % pts.len()] - pts[i];
            if self.axes.iter().any(|a| separated(a.cross(e))) {
                return false;
            }
        }
        true
    }
}
