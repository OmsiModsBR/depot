//! Where on a shoe the wire runs, from the shoe's drawn mesh (the rule: `domain::shoe`).

use glam::{DVec2, DVec3};

use crate::domain::shoe::{
    ShoeFit, SHOE_AXIS, SHOE_CONTACT_MAX, SHOE_REACH, SHOE_SEAT, WIRE_RADIUS,
};

/// Where the wire runs on the shoe whose pivot (the shoe's turn, `origin_trans`) is `pivot`,
/// given the points of the meshes the shoe's turn moves, both in the model's frame (x right,
/// y forward, z up: `omsi_geometry::mesh_from_o3d` gives a mesh file's points in it). None:
/// no place within [`SHOE_CONTACT_MAX`] of the pivot (no points near it, or none there).
pub fn shoe_contact(pivot: DVec3, points: impl IntoIterator<Item = DVec3>) -> Option<ShoeFit> {
    let along = |p: DVec3| DVec2::new(p.x - pivot.x, p.y - pivot.y).length();
    let mut top = f64::NEG_INFINITY;
    let mut groove: Option<f64> = None;
    let mut count = 0;
    for p in points.into_iter().filter(|p| along(*p) <= SHOE_REACH) {
        count += 1;
        top = top.max(p.z);
        if along(p) <= SHOE_AXIS {
            groove = Some(groove.map_or(p.z, |g| g.max(p.z)));
        }
    }
    if count == 0 {
        return None;
    }
    let seated = groove.map_or(f64::NEG_INFINITY, |g| g + SHOE_SEAT);
    let wire = (top - WIRE_RADIUS).max(seated);
    ((wire - pivot.z).abs() <= SHOE_CONTACT_MAX).then_some(ShoeFit {
        contact: DVec3::new(pivot.x, pivot.y, wire),
        points: count,
        top,
        groove,
    })
}
