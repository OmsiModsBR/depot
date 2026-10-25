//! Lanes from analytic descriptions (arcs, curves, polylines) and the heading arithmetic
//! the network shares.

use glam::{DVec2, DVec3};

use super::lane::{Lane, LaneKind, AIR_NO_LIMIT_KMH, DEFAULT_PRIORITY};

/// Builds lanes from analytic descriptions.
pub struct LaneBuilder;
impl LaneBuilder {
    /// Sample an arc/straight lane: start position, heading (deg), length, radius (0 = straight,
    /// > 0 right turn), height change over the length.
    pub fn arc(
        start: DVec3,
        heading_deg: f64,
        length: f64,
        radius: f64,
        dz: f64,
        kind: LaneKind,
        width: f32,
    ) -> Lane {
        Self::sample_arc(start, heading_deg, length, radius, kind, width, |s| {
            dz * s / length.max(1e-6)
        })
    }

    /// Object-path gradients are rise per metre, not a total height change. Integrate
    /// the gradient between the ends so the lane meets the next path on a slope.
    pub fn arc_with_gradients(
        start: DVec3,
        heading_deg: f64,
        length: f64,
        radius: f64,
        grad_start: f64,
        grad_end: f64,
        kind: LaneKind,
        width: f32,
    ) -> Lane {
        Self::sample_arc(start, heading_deg, length, radius, kind, width, |s| {
            grad_start * s + (grad_end - grad_start) * s * s / (2.0 * length.max(1e-6))
        })
    }

    fn sample_arc(
        start: DVec3,
        heading_deg: f64,
        length: f64,
        radius: f64,
        kind: LaneKind,
        width: f32,
        height: impl Fn(f64) -> f64,
    ) -> Lane {
        let n = ((length / 2.0).ceil() as usize).clamp(1, 400);
        let mut points = Vec::with_capacity(n + 1);
        let mut headings = Vec::with_capacity(n + 1);
        for i in 0..=n {
            let s = length * i as f64 / n as f64;
            let (p, h) = arc_point(start, heading_deg, s, radius);
            points.push(DVec3::new(p.x, p.y, start.z + height(s)));
            headings.push(h as f32);
        }
        let k = if radius.abs() < 1e-6 {
            0.0
        } else {
            (1.0 / radius) as f32
        };
        Self::curve(points, headings, vec![k; n + 1], kind, width)
    }

    /// Lane from sampled points with their tangent headings and curvatures.
    /// A flight path has no speed limit of its own unless the map gives it one with a
    /// `[rule] speedlimit`; the street default of 50 km/h had the Tegel approach flown at
    /// walking pace for an airliner.
    pub fn curve(
        points: Vec<DVec3>,
        headings: Vec<f32>,
        curvature: Vec<f32>,
        kind: LaneKind,
        width: f32,
    ) -> Lane {
        let dist = cumulative(&points);
        let speed_limit_kmh = if kind == LaneKind::Air {
            AIR_NO_LIMIT_KMH
        } else {
            50.0
        };
        Lane {
            key: None,
            reversed: false,
            kind,
            width,
            points,
            headings,
            curvature,
            dist,
            speed_limit_kmh,
            limit_from_map: false,
            next: Vec::new(),
            traffic_light: None,
            turn: 0,
            source: 0,
            offset: 0.0,
            name: String::new(),
            invisible: false,
            density: 1.0,
            group_density: Vec::new(),
            no_cars: false,
            rule_bus: false,
            rule_trucks: false,
            left: None,
            right: None,
            priority: DEFAULT_PRIORITY,
            blocks: Vec::new(),
        }
    }

    /// Lane from bare points: headings from the neighbouring points on both sides (the
    /// tangent there, not the chord ahead), curvature from how the heading changes.
    pub fn polyline(points: Vec<DVec3>, kind: LaneKind, width: f32) -> Lane {
        let n = points.len();
        let mut headings = Vec::with_capacity(n);
        for i in 0..n {
            let (a, b) = match (i.checked_sub(1), (i + 1 < n).then_some(i + 1)) {
                (Some(p), Some(q)) => (points[p], points[q]),
                (None, Some(q)) => (points[i], points[q]),
                (Some(p), None) => (points[p], points[i]),
                (None, None) => (points[i], points[i] + DVec3::Y),
            };
            let d = b - a;
            headings.push((d.x.atan2(d.y)).to_degrees() as f32);
        }
        let dist = cumulative(&points);
        let curvature = (0..n)
            .map(|i| {
                let (p, q) = (i.saturating_sub(1), (i + 1).min(n.saturating_sub(1)));
                let ds = dist.get(q).copied().unwrap_or(0.0) - dist.get(p).copied().unwrap_or(0.0);
                if ds > 1e-3 {
                    wrap_deg(headings[q] - headings[p]).to_radians() / ds
                } else {
                    0.0
                }
            })
            .collect();
        Self::curve(points, headings, curvature, kind, width)
    }
}

/// Cumulative distance along a sequence of points.
pub(super) fn cumulative(points: &[DVec3]) -> Vec<f32> {
    let mut acc = 0.0f32;
    points
        .iter()
        .enumerate()
        .map(|(i, p)| {
            if i > 0 {
                acc += (*p - points[i - 1]).length() as f32;
            }
            acc
        })
        .collect()
}

/// An angle difference in degrees brought into -180..180.
pub fn wrap_deg(d: f32) -> f32 {
    (d + 180.0).rem_euclid(360.0) - 180.0
}

/// Unit vector (east, north) of a heading in degrees.
pub(super) fn heading_dir(h: f64) -> DVec2 {
    let r = h.to_radians();
    DVec2::new(r.sin(), r.cos())
}

/// Point and heading after travelling `s` metres on an arc starting at `start`.
pub fn arc_point(start: DVec3, heading_deg: f64, s: f64, radius: f64) -> (DVec2, f64) {
    let h = heading_deg.to_radians();
    let d = DVec2::new(h.sin(), h.cos());
    if radius.abs() < 1e-6 {
        return (start.truncate() + d * s, heading_deg);
    }
    let r = radius.abs();
    let turn = radius.signum();
    let right = DVec2::new(d.y, -d.x);
    let centre = start.truncate() + right * r * turn;
    let ang = -turn * s / r;
    let p = start.truncate() - centre;
    let (sa, ca) = ang.sin_cos();
    (
        centre + DVec2::new(p.x * ca - p.y * sa, p.x * sa + p.y * ca),
        heading_deg + turn * (s / r).to_degrees(),
    )
}
