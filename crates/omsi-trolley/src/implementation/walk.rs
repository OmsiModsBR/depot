//! Moving along a wire: the point of a wire at a given distance from a pole's foot.

use glam::DVec3;

use crate::domain::network::{SegmentRef, WireNetwork, WireSide};
use crate::domain::pole::WireCursor;

/// How many pieces of wire one step may walk over.
const MAX_PIECES: usize = 64;

/// Where a walk ended.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Reached {
    pub cursor: WireCursor,
    pub point: DVec3,
    /// The distance from the foot to `point` minus the pole's length: 0 when the pole
    /// reaches the wire there, positive when the wire stays out of reach.
    pub error: f64,
    /// Joints passed: (spline left, spline entered, gap).
    pub joints: Vec<(i64, i64, f64)>,
    /// The wire ended before the pole reached it.
    pub wire_ended: bool,
}

pub(crate) fn point(net: &WireNetwork, c: &WireCursor) -> Option<DVec3> {
    let pts = &net.span(c.span)?.wires[c.side.index()];
    let (a, b) = (*pts.get(c.seg)?, *pts.get(c.seg + 1)?);
    Some(a.lerp(b, c.t))
}

/// The direction of the wire at `c` (towards its next point).
pub(crate) fn tangent(net: &WireNetwork, c: &WireCursor) -> Option<DVec3> {
    let pts = &net.span(c.span)?.wires[c.side.index()];
    Some((*pts.get(c.seg + 1)? - *pts.get(c.seg)?).normalize_or_zero())
}

/// The parameters (0..1 along `a`-`b`) where the piece crosses the sphere of radius `len`
/// about `foot`.
pub(crate) fn sphere_crossings(a: DVec3, b: DVec3, foot: DVec3, len: f64) -> Vec<f64> {
    let d = b - a;
    let f = a - foot;
    let qa = d.length_squared();
    if qa <= 0.0 {
        return Vec::new();
    }
    let qb = 2.0 * f.dot(d);
    let qc = f.length_squared() - len * len;
    let disc = qb * qb - 4.0 * qa * qc;
    if disc < 0.0 {
        return Vec::new();
    }
    let r = disc.sqrt();
    [(-qb - r) / (2.0 * qa), (-qb + r) / (2.0 * qa)]
        .into_iter()
        .filter(|u| (0.0..=1.0).contains(u))
        .collect()
}

/// Walk from `from` along its wire (across joints) to the nearest point at `len` from
/// `foot`: towards the foot when the wire there is too far; when too near, out the way
/// `back` points (a trailing pole's shoe stays behind its foot).
pub(crate) fn reach(
    net: &WireNetwork,
    from: WireCursor,
    foot: DVec3,
    len: f64,
    back: DVec3,
) -> Option<Reached> {
    let mut c = from;
    let mut p = point(net, &c)?;
    let start_error = p.distance(foot) - len;
    let mut joints = Vec::new();
    if start_error.abs() < 1e-9 {
        return Some(Reached {
            cursor: c,
            point: p,
            error: 0.0,
            joints,
            wire_ended: false,
        });
    }
    let t = tangent(net, &c)?;
    // +1 along the wire's points, -1 against them
    let mut dir: i32 = if start_error < 0.0 {
        if t.dot(back) >= 0.0 {
            1
        } else {
            -1
        }
    } else if (p - foot).dot(t) >= 0.0 {
        -1
    } else {
        1
    };
    let mut closest = (c, p, start_error);
    for _ in 0..MAX_PIECES {
        let pts = &net.span(c.span)?.wires[c.side.index()];
        let (a, b) = (pts[c.seg], pts[c.seg + 1]);
        let q = if dir > 0 { b } else { a };
        // the crossing nearest to p ahead of it
        let ahead: Vec<f64> = sphere_crossings(a, b, foot, len)
            .into_iter()
            .filter(|u| {
                if dir > 0 {
                    *u >= c.t - 1e-12
                } else {
                    *u <= c.t + 1e-12
                }
            })
            .collect();
        let hit = if dir > 0 {
            ahead.into_iter().reduce(f64::min)
        } else {
            ahead.into_iter().reduce(f64::max)
        };
        if let Some(u) = hit {
            c.t = u;
            let at = a.lerp(b, u);
            return Some(Reached {
                cursor: c,
                point: at,
                error: 0.0,
                joints,
                wire_ended: false,
            });
        }
        if start_error > 0.0 {
            // going in towards the foot without reaching it: the nearest the wire comes
            let ab = b - a;
            let u = if ab.length_squared() > 0.0 {
                ((foot - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let u = if dir > 0 { u.max(c.t) } else { u.min(c.t) };
            let at = a.lerp(b, u);
            let e = at.distance(foot) - len;
            if e < closest.2 {
                closest = (WireCursor { t: u, ..c }, at, e);
            }
            if q.distance(foot) > p.distance(foot) {
                break;
            }
        }
        p = q;
        // the next piece: in this span, or across a joint
        let next = if dir > 0 {
            c.seg + 1 < pts.len() - 1
        } else {
            c.seg > 0
        };
        if next {
            if dir > 0 {
                c.seg += 1;
                c.t = 0.0;
            } else {
                c.seg -= 1;
                c.t = 1.0;
            }
            continue;
        }
        let end = usize::from(dir > 0);
        let from_id = net.span(c.span)?.spline_id;
        let Some((nc, ndir, gap)) = across_joint(net, &c, end, q) else {
            let e = q.distance(foot) - len;
            let (cur, pt, err) = if start_error > 0.0 {
                closest
            } else {
                (
                    WireCursor {
                        t: f64::from(end as u8),
                        ..c
                    },
                    q,
                    e,
                )
            };
            return Some(Reached {
                cursor: cur,
                point: pt,
                error: err.max(0.0),
                joints,
                wire_ended: true,
            });
        };
        joints.push((from_id, net.span(nc.span)?.spline_id, gap));
        c = nc;
        dir = ndir;
        p = point(net, &c)?;
        // the pole's length fell in the joint's gap: the shoe passes the loose joint onto
        // the next wire's end
        if (start_error < 0.0) == (p.distance(foot) > len) {
            return Some(Reached {
                cursor: c,
                point: p,
                error: 0.0,
                joints,
                wire_ended: false,
            });
        }
    }
    let (cur, pt, err) = closest;
    Some(Reached {
        cursor: cur,
        point: pt,
        error: err.max(0.0),
        joints,
        wire_ended: false,
    })
}

/// The wire that goes on from the end `end` of `c`'s span (whose wire ends at `at`): the
/// joined span's wire nearest to `at`, the cursor at its joined end and the way into it.
fn across_joint(
    net: &WireNetwork,
    c: &WireCursor,
    end: usize,
    at: DVec3,
) -> Option<(WireCursor, i32, f64)> {
    let mut best: Option<(WireCursor, i32, f64)> = None;
    for j in net.joints_at(c.span, end) {
        let (other, other_end) = if j.a == (c.span, end) { j.b } else { j.a };
        let span = net.span(other)?;
        for side in WireSide::BOTH {
            let pts = &span.wires[side.index()];
            if pts.len() < 2 {
                continue;
            }
            let (seg, t, dir, p) = if other_end == 0 {
                (0, 0.0, 1, pts[0])
            } else {
                (pts.len() - 2, 1.0, -1, pts[pts.len() - 1])
            };
            let gap = p.distance(at);
            if best.as_ref().is_none_or(|b| gap < b.2) {
                best = Some((
                    WireCursor {
                        span: other,
                        side,
                        seg,
                        t,
                    },
                    dir,
                    gap,
                ));
            }
        }
    }
    best
}

/// Points of the wire pieces near `p` that lie at `len` from `foot`, with their cursor.
pub(crate) fn crossings_near(
    net: &WireNetwork,
    p: DVec3,
    radius: f64,
    foot: DVec3,
    len: f64,
) -> Vec<(WireCursor, DVec3)> {
    net.segments_near(p, radius)
        .into_iter()
        .filter_map(|(r, _): (SegmentRef, f64)| {
            let (a, b) = net.segment(r)?;
            Some(
                sphere_crossings(a, b, foot, len)
                    .into_iter()
                    .map(move |u| {
                        (
                            WireCursor {
                                span: r.span,
                                side: r.side,
                                seg: r.index,
                                t: u,
                            },
                            a.lerp(b, u),
                        )
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .flatten()
        .collect()
}
