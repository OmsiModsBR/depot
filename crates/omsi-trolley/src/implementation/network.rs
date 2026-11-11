//! Building the wire of a tile and keeping the network of the loaded tiles.

use std::collections::{HashMap, HashSet};

use glam::{DVec2, DVec3};

use crate::domain::convention::ConventionParams;
use crate::domain::network::{
    SegmentRef, SpanEnd, SpanId, TileWires, WireJoint, WireNetwork, WireObject, WireSide, WireSpan,
    GRID_CELL, JOIN_MIN_ALIGNMENT, JOIN_TOLERANCE, OBJECT_MARKERS,
};
use crate::interface::output::{MapObject, MapSpline, SplineCurve, Tile};

/// Largest distance between a curved wire and the straight pieces drawn through its points.
const CHORD_SAGITTA: f64 = 0.005;
/// The polyline has at least one point every this many metres (the height changes along a
/// spline even when it runs straight).
const MAX_STEP: f64 = 10.0;

/// The wire of tile `tile`, whose origin in the global frame is `origin` (its tile index
/// times the map's tile size). `map` is the tile with its chrono changes applied and fitted
/// to the map's grid, as the game reads it.
pub fn build_tile_wires(
    tile: (i32, i32),
    origin: DVec2,
    map: &Tile,
    params: &ConventionParams,
) -> TileWires {
    let spans = map
        .splines
        .iter()
        .filter(|s| !s.deleted && params.is_wire_spline(&s.file))
        .map(|s| span_of(s, origin, params))
        .collect();
    let objects = map
        .objects
        .iter()
        .filter_map(|o| object_of(o, origin))
        .collect();
    TileWires {
        tile,
        spans,
        objects,
    }
}

fn span_of(s: &MapSpline, origin: DVec2, params: &ConventionParams) -> WireSpan {
    let curve = SplineCurve::from_map(s, origin);
    let length = curve.length.max(0.0);
    let steps = step_count(&curve);
    // (a mirrored spline lies the other way round: its left is the map's right)
    let side = if s.mirror { -1.0 } else { 1.0 };
    let xs = [params.wire_x_left * side, params.wire_x_right * side];
    let wires = xs.map(|x| {
        (0..=steps)
            .map(|i| {
                let at = length * i as f64 / steps as f64;
                curve.offset_point(at, x, params.wire_height)
            })
            .collect()
    });
    let tangent = |at: f64| {
        let d = SplineCurve::dir(curve.heading_at(at));
        DVec3::new(d.x, d.y, curve.slope_at(at)).normalize()
    };
    let ends = [
        SpanEnd {
            point: curve.offset_point(0.0, 0.0, params.wire_height),
            outward: -tangent(0.0),
        },
        SpanEnd {
            point: curve.offset_point(length, 0.0, params.wire_height),
            outward: tangent(length),
        },
    ];
    WireSpan {
        spline_id: s.id,
        file: s.file.clone(),
        length,
        wires,
        ends,
    }
}

fn step_count(curve: &SplineCurve) -> usize {
    let length = curve.length.max(0.0);
    let by_height = (length / MAX_STEP).ceil();
    let by_turn = if curve.radius == 0.0 {
        1.0
    } else {
        // (a chord of length c on radius r stands c²/8r off the arc)
        let step = (8.0 * curve.radius.abs() * CHORD_SAGITTA).sqrt().max(0.05);
        (length / step).ceil()
    };
    by_height.max(by_turn).max(1.0) as usize
}

fn object_of(o: &MapObject, origin: DVec2) -> Option<WireObject> {
    let path = o.file.to_ascii_lowercase();
    let kind = OBJECT_MARKERS
        .iter()
        .find(|(marker, _)| path.contains(marker))
        .map(|(_, kind)| *kind)?;
    Some(WireObject {
        kind,
        object_id: o.id,
        position: DVec3::new(origin.x + o.pos[0], origin.y + o.pos[1], o.pos[2]),
        heading_deg: o.rot[0],
    })
}

impl WireNetwork {
    /// Add (or replace) a tile's wire. An empty tile is not kept.
    pub fn insert_tile(&mut self, wires: TileWires) {
        let key = wires.tile;
        self.drop_tile(key);
        if wires.is_empty() {
            self.rebuild_joints();
            return;
        }
        for (i, span) in wires.spans.iter().enumerate() {
            for side in WireSide::BOTH {
                let pts = &span.wires[side.index()];
                for index in 0..pts.len().saturating_sub(1) {
                    let r = SegmentRef {
                        span: (key, i),
                        side,
                        index,
                    };
                    for c in cells(pts[index], pts[index + 1], 0.0) {
                        self.grid.entry(c).or_default().push(r);
                    }
                }
            }
        }
        self.tiles.insert(key, wires);
        self.rebuild_joints();
    }

    /// Take a tile's wire away (the tile was unloaded).
    pub fn remove_tile(&mut self, tile: (i32, i32)) {
        if self.drop_tile(tile) {
            self.rebuild_joints();
        }
    }

    fn drop_tile(&mut self, tile: (i32, i32)) -> bool {
        if self.tiles.remove(&tile).is_none() {
            return false;
        }
        self.grid.retain(|_, refs| {
            refs.retain(|r| r.span.0 != tile);
            !refs.is_empty()
        });
        true
    }

    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    pub fn tile(&self, tile: (i32, i32)) -> Option<&TileWires> {
        self.tiles.get(&tile)
    }

    pub fn span(&self, id: SpanId) -> Option<&WireSpan> {
        self.tiles.get(&id.0).and_then(|t| t.spans.get(id.1))
    }

    pub fn span_count(&self) -> usize {
        self.tiles.values().map(|t| t.spans.len()).sum()
    }

    pub fn objects(&self) -> impl Iterator<Item = &WireObject> {
        self.tiles.values().flat_map(|t| t.objects.iter())
    }

    pub fn joints(&self) -> &[WireJoint] {
        &self.joints
    }

    /// The joints at one end of a span.
    pub fn joints_at(&self, span: SpanId, end: usize) -> impl Iterator<Item = &WireJoint> {
        self.joints
            .iter()
            .filter(move |j| j.a == (span, end) || j.b == (span, end))
    }

    /// The two points of a segment.
    pub fn segment(&self, r: SegmentRef) -> Option<(DVec3, DVec3)> {
        let pts = &self.span(r.span)?.wires[r.side.index()];
        Some((*pts.get(r.index)?, *pts.get(r.index + 1)?))
    }

    /// The wire segments that pass within `radius` of `p`, nearest first, with their
    /// distance.
    pub fn segments_near(&self, p: DVec3, radius: f64) -> Vec<(SegmentRef, f64)> {
        let mut seen = HashSet::new();
        let mut out: Vec<(SegmentRef, f64)> = cells(p, p, radius)
            .into_iter()
            .filter_map(|c| self.grid.get(&c))
            .flatten()
            .filter(|r| seen.insert(**r))
            .filter_map(|r| {
                let (a, b) = self.segment(*r)?;
                let d = distance_to_segment(p, a, b);
                (d <= radius).then_some((*r, d))
            })
            .collect();
        out.sort_by(|x, y| x.1.total_cmp(&y.1));
        out
    }

    fn rebuild_joints(&mut self) {
        let mut ends: Vec<(SpanId, usize, SpanEnd)> = Vec::new();
        for (tile, t) in &self.tiles {
            for (i, span) in t.spans.iter().enumerate() {
                for (e, end) in span.ends.iter().enumerate() {
                    ends.push(((*tile, i), e, *end));
                }
            }
        }
        // (the tiles come out of a hash map: order the ends so the same wire gives the same
        // joints, each with its lower end first)
        ends.sort_by_key(|(span, end, _)| (*span, *end));
        let mut buckets: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for (n, (_, _, end)) in ends.iter().enumerate() {
            buckets
                .entry(cell_of(end.point, JOIN_TOLERANCE))
                .or_default()
                .push(n);
        }
        let mut joints = Vec::new();
        for (n, (span_a, end_a, a)) in ends.iter().enumerate() {
            let (cx, cy) = cell_of(a.point, JOIN_TOLERANCE);
            for dx in -1..=1 {
                for dy in -1..=1 {
                    let Some(list) = buckets.get(&(cx + dx, cy + dy)) else {
                        continue;
                    };
                    for &m in list.iter().filter(|&&m| m > n) {
                        let (span_b, end_b, b) = &ends[m];
                        if span_a == span_b {
                            continue;
                        }
                        let gap = a.point.distance(b.point);
                        // (one end's way out is the other's way in)
                        if gap <= JOIN_TOLERANCE && a.outward.dot(b.outward) <= -JOIN_MIN_ALIGNMENT
                        {
                            joints.push(WireJoint {
                                a: (*span_a, *end_a),
                                b: (*span_b, *end_b),
                                gap,
                                reversed: end_a == end_b,
                            });
                        }
                    }
                }
            }
        }
        joints.sort_by_key(|j| (j.a.0, j.a.1, j.b.0, j.b.1));
        self.joints = joints;
    }
}

fn cell_of(p: DVec3, size: f64) -> (i64, i64) {
    ((p.x / size).floor() as i64, (p.y / size).floor() as i64)
}

/// The grid cells the box around `a`-`b`, widened by `margin`, touches.
fn cells(a: DVec3, b: DVec3, margin: f64) -> Vec<(i64, i64)> {
    let lo = cell_of(a.min(b) - DVec3::splat(margin), GRID_CELL);
    let hi = cell_of(a.max(b) + DVec3::splat(margin), GRID_CELL);
    let mut out = Vec::new();
    for x in lo.0..=hi.0 {
        for y in lo.1..=hi.1 {
            out.push((x, y));
        }
    }
    out
}

fn distance_to_segment(p: DVec3, a: DVec3, b: DVec3) -> f64 {
    let ab = b - a;
    let len2 = ab.length_squared();
    let t = if len2 > 0.0 {
        ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    p.distance(a + ab * t)
}
