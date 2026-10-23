//! Convex polygon clipping in the ground plane and at a height.

use glam::{DVec2, DVec3};

/// Area centroid of a polygon (the vertex average when it has no area).
pub(super) fn centroid(poly: &[DVec2]) -> Option<DVec2> {
    if poly.is_empty() {
        return None;
    }
    let (mut area, mut c) = (0.0, DVec2::ZERO);
    for i in 0..poly.len() {
        let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
        let a = p.perp_dot(q);
        area += a;
        c += (p + q) * a;
    }
    if area.abs() < 1e-6 {
        return Some(poly.iter().copied().sum::<DVec2>() / poly.len() as f64);
    }
    Some(c / (3.0 * area))
}

/// The part of the convex polygon `subject` inside the convex polygon `clipper`
/// (Sutherland-Hodgman); both wound the same way as [`Obb::corners`].
pub(super) fn clip(subject: &[DVec2], clipper: &[DVec2]) -> Vec<DVec2> {
    let mut out: Vec<DVec2> = subject.to_vec();
    // the corners run clockwise seen from above: "inside" is to the right of each edge
    let n = clipper.len();
    let centre = clipper.iter().copied().sum::<DVec2>() / n as f64;
    for i in 0..n {
        let (a, b) = (clipper[i], clipper[(i + 1) % n]);
        let edge = b - a;
        let side = |p: DVec2| edge.perp_dot(p - a);
        let inside_sign = side(centre).signum();
        let input = std::mem::take(&mut out);
        if input.is_empty() {
            break;
        }
        for j in 0..input.len() {
            let (p, q) = (input[j], input[(j + 1) % input.len()]);
            let (sp, sq) = (side(p) * inside_sign, side(q) * inside_sign);
            if sp >= 0.0 {
                out.push(p);
            }
            if (sp >= 0.0) != (sq >= 0.0) {
                let t = sp / (sp - sq);
                out.push(p + (q - p) * t);
            }
        }
    }
    out
}

/// The part of a convex polygon above (`keep_above`) or below the height `z`.
pub(super) fn clip_z(poly: &[DVec3], z: f64, keep_above: bool) -> Vec<DVec3> {
    let inside = |p: &DVec3| if keep_above { p.z >= z } else { p.z <= z };
    let mut out = Vec::new();
    for i in 0..poly.len() {
        let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
        if inside(&p) {
            out.push(p);
        }
        if inside(&p) != inside(&q) {
            let t = (z - p.z) / (q.z - p.z);
            out.push(p + (q - p) * t);
        }
    }
    out
}
