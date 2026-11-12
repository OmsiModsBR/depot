//! Moving along a wire: the point of a wire at a given distance from a pole's foot.

use glam::{DVec2, DVec3};

use crate::domain::network::{SegmentRef, SpanId, WireNetwork, WireObjectKind, WireSide};
use crate::domain::pole::WireCursor;

/// The radius (metres) the wire a switch draws across its gap turns with, from the way it
/// comes in to the way it goes on (the turn a 20 degree switch's branch makes; a shorter
/// turn where the gap leaves no room for it).
const GAP_TURN_RADIUS: f64 = 5.0;

/// How many pieces the turn across a gap is drawn in.
const GAP_TURN_PIECES: usize = 8;

/// The wire across a gap from `a` (coming in along `ta`) to `b` (going on along `tb`): on
/// along `ta`, a turn of [`GAP_TURN_RADIUS`] that meets both ways, and on along `tb` - as a
/// switch's branch leaves the line; straight across where the two ways are in line (or do
/// not meet between the ends).
pub(crate) fn gap_path(a: DVec3, ta: DVec3, b: DVec3, tb: DVec3) -> Vec<DVec3> {
    let (a2, b2) = (a.truncate(), b.truncate());
    let (u, v) = (
        ta.truncate().normalize_or_zero(),
        tb.truncate().normalize_or_zero(),
    );
    let turn = u.perp_dot(v).atan2(u.dot(v));
    let cross = u.perp_dot(v);
    if turn.abs() < 1f64.to_radians() || cross.abs() < 1e-9 {
        return vec![a, b];
    }
    // where the two ways meet: a + s u = b - r v
    let ab = b2 - a2;
    let s = ab.perp_dot(v) / cross;
    let r = -ab.perp_dot(u) / cross;
    if s <= 0.0 || r <= 0.0 {
        return vec![a, b];
    }
    let half = (turn.abs() / 2.0).tan();
    let d = (GAP_TURN_RADIUS * half).min(s).min(r);
    let radius = d / half;
    let meet = a2 + u * s;
    let t1 = meet - u * d;
    let normal = if turn > 0.0 { u.perp() } else { -u.perp() };
    let centre = t1 + normal * radius;
    let mut flat = vec![a2];
    if t1.distance(a2) > 1e-6 {
        flat.push(t1);
    }
    for k in 1..=GAP_TURN_PIECES {
        let ang = turn * k as f64 / GAP_TURN_PIECES as f64;
        flat.push(centre + DVec2::from_angle(ang).rotate(t1 - centre));
    }
    if flat.last().is_some_and(|l| l.distance(b2) > 1e-6) {
        flat.push(b2);
    }
    // (height along the way, from one end's to the other's)
    let total: f64 = flat
        .windows(2)
        .map(|w| w[0].distance(w[1]))
        .sum::<f64>()
        .max(1e-9);
    let mut run = 0.0;
    let mut out = Vec::with_capacity(flat.len());
    for (k, p) in flat.iter().enumerate() {
        if k > 0 {
            run += flat[k - 1].distance(*p);
        }
        out.push(p.extend(a.z + (b.z - a.z) * run / total));
    }
    out
}

/// The first point of `path` (from its start) at `len` from `foot`.
pub(crate) fn first_crossing(path: &[DVec3], foot: DVec3, len: f64) -> Option<DVec3> {
    path.windows(2).find_map(|w| {
        sphere_crossings(w[0], w[1], foot, len)
            .into_iter()
            .reduce(f64::min)
            .map(|u| w[0].lerp(w[1], u))
    })
}

/// How many pieces of wire one step may walk over.
const MAX_PIECES: usize = 64;

/// How far (metres) beside a span's end a plain joint may lead and still be a way on where
/// the end has another: farther, it is the start of a neighbouring wire that happens to face
/// the same way (a switch's branch beside a line's end), and taking it puts a shoe on the
/// wrong wire. An end with no other way on keeps it.
pub(crate) const SPLICE_MAX_BESIDE: f64 = 0.3;

/// One way on from the end of a span: the joined span's wire on the same side, where it
/// starts and which way it runs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Way {
    pub cursor: WireCursor,
    pub dir: i32,
    /// The joined wire's end point.
    pub start: DVec3,
    /// Its direction leaving that end (unit).
    pub heading: DVec3,
    pub gap: f64,
    /// The switch or crossing whose gap the way crosses.
    pub via: Option<WireObjectKind>,
}

/// Picks the way a shoe takes where a span's end has more than one: given the span end
/// (span, 0 start or 1 end), the ways and the direction the shoe travels in, the index of
/// the way.
pub(crate) type Chooser<'a> = dyn FnMut((SpanId, usize), &[Way], DVec3) -> usize + 'a;

/// A joint a walk passed.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Passed {
    /// The spline left and the one entered.
    pub from: i64,
    pub to: i64,
    pub gap: f64,
    /// The switch or crossing whose gap the joint bridges.
    pub via: Option<WireObjectKind>,
    /// Where the joined wire starts.
    pub at: DVec3,
    /// At a switch the shoe came in to from one of its two ways: whether that way is the
    /// left one (None: no switch, or the shoe left the switch's entry).
    pub merge_left: Option<bool>,
}

/// Where a walk ended.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Reached {
    pub cursor: WireCursor,
    pub point: DVec3,
    /// The distance from the foot to `point` minus the pole's length: 0 when the pole
    /// reaches the wire there, positive when the wire stays out of reach.
    pub error: f64,
    /// Joints passed.
    pub joints: Vec<Passed>,
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
    choose: &mut Chooser<'_>,
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
        let travel = if dir > 0 { b - a } else { a - b }.normalize_or_zero();
        let ways = ways_on(net, &c, end)?;
        if ways.is_empty() {
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
        }
        let pick = if ways.len() == 1 {
            0
        } else {
            choose((c.span, end), &ways, travel).min(ways.len() - 1)
        };
        let w = ways[pick];
        // the gap itself (a joint's looseness, or the wire a switch or crossing draws): the
        // pole's length may fall in it. The shoe is still on the wire it came from until it
        // is past the gap: put on the joined wire, the next step would walk back across the
        // gap from there (and, at a switch's entry, choose a way again).
        let in_gap = first_crossing(&gap_path(q, travel, w.start, w.heading), foot, len);
        if let Some(at) = in_gap {
            return Some(Reached {
                cursor: WireCursor {
                    t: f64::from(end as u8),
                    ..c
                },
                point: at,
                error: 0.0,
                joints,
                wire_ended: false,
            });
        }
        let merge_left = if ways.len() == 1 && w.via.is_some_and(WireObjectKind::is_switch) {
            merge_side(net, (c.span, end), &w)
        } else {
            None
        };
        joints.push(Passed {
            from: from_id,
            to: net.span(w.cursor.span)?.spline_id,
            gap: w.gap,
            via: w.via,
            at: w.start,
            merge_left,
        });
        c = w.cursor;
        dir = w.dir;
        p = w.start;
        // past the gap already: the shoe goes on from the joined wire's end
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

/// The ways on from the end `end` of `c`'s span, keeping the shoe's side: of each joined
/// span's two wires, the one that lies on the same side of its partner as `c`'s wire does
/// of its own (seen along the way the shoe travels), so a left shoe stays on the left wire
/// through reversed splices and into either branch of a switch.
pub(crate) fn ways_on(net: &WireNetwork, c: &WireCursor, end: usize) -> Option<Vec<Way>> {
    let here = net.span(c.span)?;
    let tip = |wire: &Vec<DVec3>, e: usize| {
        if e == 0 {
            wire[0]
        } else {
            wire[wire.len() - 1]
        }
    };
    let own = tip(&here.wires[c.side.index()], end);
    let partner = tip(&here.wires[other(c.side).index()], end);
    let out = here.ends[end].outward;
    // (which side of the partner the shoe's wire is, about the vertical, along the travel)
    let side_of = |w: DVec3, p: DVec3, along: DVec3| along.cross(w - p).z.signum();
    let ours = side_of(own, partner, out);
    let mut ways = Vec::new();
    let mut beside = Vec::new();
    for j in net.joints_at(c.span, end) {
        let (span_id, other_end) = if j.a == (c.span, end) { j.b } else { j.a };
        let span = net.span(span_id)?;
        if span.wires.iter().any(|w| w.len() < 2) {
            continue;
        }
        let heading = -span.ends[other_end].outward;
        let side = WireSide::BOTH
            .into_iter()
            .find(|s| {
                let w = tip(&span.wires[s.index()], other_end);
                let p = tip(&span.wires[other(*s).index()], other_end);
                side_of(w, p, heading) == ours
            })
            .unwrap_or(c.side);
        let pts = &span.wires[side.index()];
        let (seg, t, dir) = if other_end == 0 {
            (0, 0.0, 1)
        } else {
            (pts.len() - 2, 1.0, -1)
        };
        let start = tip(pts, other_end);
        ways.push(Way {
            cursor: WireCursor {
                span: span_id,
                side,
                seg,
                t,
            },
            dir,
            start,
            heading,
            gap: start.distance(own),
            via: j.via,
        });
        let d = span.ends[other_end].point - here.ends[end].point;
        beside.push((d - out * d.dot(out)).truncate().length());
    }
    let true_way = |k: usize| ways[k].via.is_some() || beside[k] <= SPLICE_MAX_BESIDE;
    if (0..ways.len()).any(true_way) {
        let keep: Vec<bool> = (0..ways.len()).map(true_way).collect();
        let mut k = 0;
        ways.retain(|_| {
            k += 1;
            keep[k - 1]
        });
    }
    Some(ways)
}

/// Coming from the span end `from` over the switch way `w` into the switch's entry: whether
/// `from` is the left of the two ways that meet there, seen from the entry (None: no other
/// way meets it).
fn merge_side(net: &WireNetwork, from: (SpanId, usize), w: &Way) -> Option<bool> {
    let into = (w.cursor.span, usize::from(w.dir < 0));
    let ours = net.span(from.0)?.ends[from.1].point;
    let theirs = net
        .joints_at(into.0, into.1)
        .map(|j| if j.a == into { j.b } else { j.a })
        .filter(|o| *o != from)
        .find_map(|(s, e)| net.span(s).map(|sp| sp.ends[e].point))?;
    // (left as seen from the entry, looking along its ways)
    Some(w.heading.cross(ours - theirs).z < 0.0)
}

fn other(s: WireSide) -> WireSide {
    match s {
        WireSide::Left => WireSide::Right,
        WireSide::Right => WireSide::Left,
    }
}

/// Points of the wire across a gap (the straight from a span's end to the way on: what a
/// switch or crossing draws, or a loose splice), near `p`, that lie at `len` from `foot`: the cursor stands at the end
/// the gap starts from, as a shoe in a gap does; with the span of the way the gap leads to.
pub(crate) fn gap_crossings_near(
    net: &WireNetwork,
    p: DVec3,
    radius: f64,
    foot: DVec3,
    len: f64,
) -> Vec<(WireCursor, DVec3, SpanId)> {
    let mut ends: Vec<(SpanId, usize)> = Vec::new();
    for (r, _) in net.segments_near(p, radius + 6.0) {
        for e in 0..2 {
            if !ends.contains(&(r.span, e)) {
                ends.push((r.span, e));
            }
        }
    }
    let mut out = Vec::new();
    for (span, e) in ends {
        let Some(s) = net.span(span) else {
            continue;
        };
        if (s.ends[e].point - p).truncate().length() > radius + 6.0 {
            continue;
        }
        for side in WireSide::BOTH {
            let pts = &s.wires[side.index()];
            if pts.len() < 2 {
                continue;
            }
            let c = WireCursor {
                span,
                side,
                seg: if e == 0 { 0 } else { pts.len() - 2 },
                t: f64::from(e as u8),
            };
            let (tip, inner) = if e == 0 {
                (pts[0], pts[1])
            } else {
                (pts[pts.len() - 1], pts[pts.len() - 2])
            };
            for w in ways_on(net, &c, e).unwrap_or_default() {
                let path = gap_path(tip, tip - inner, w.start, w.heading);
                for piece in path.windows(2) {
                    for u in sphere_crossings(piece[0], piece[1], foot, len) {
                        let at = piece[0].lerp(piece[1], u);
                        if (at - p).length() <= radius {
                            out.push((c, at, w.cursor.span));
                        }
                    }
                }
            }
        }
    }
    out
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
