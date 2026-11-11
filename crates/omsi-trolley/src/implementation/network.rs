//! Building the wire of a tile and keeping the network of the loaded tiles.

use std::collections::HashSet;

use glam::{DVec2, DVec3};

use crate::domain::convention::ConventionParams;
use crate::domain::network::{
    SegmentRef, SpanEnd, SpanId, TileWires, WireJoint, WireNetwork, WireObject, WireObjectKind,
    WireSide, WireSpan, BRIDGE_MAX_GAP, BRIDGE_MIN_ALIGNMENT, BRIDGE_RADIUS, GRID_CELL,
    JOIN_MIN_ALIGNMENT, JOIN_TOLERANCE, OBJECT_MARKERS,
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
    /// Add (or replace) a tile's wire: its segments go into the grid and its span ends are
    /// joined to the ends already there. An empty tile is not kept.
    pub fn insert_tile(&mut self, wires: TileWires) {
        let key = wires.tile;
        self.drop_tile(key);
        if wires.is_empty() {
            return;
        }
        let mut used = HashSet::new();
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
                        used.insert(c);
                    }
                }
            }
        }
        self.tile_cells.insert(key, used.into_iter().collect());
        let ends: Vec<(SpanId, usize, SpanEnd)> = wires
            .spans
            .iter()
            .enumerate()
            .flat_map(|(i, span)| (0..2).map(move |e| ((key, i), e, span.ends[e])))
            .collect();
        self.tiles.insert(key, wires);
        for (span, end, at) in ends {
            let (cx, cy) = cell_of(at.point, JOIN_TOLERANCE);
            for dx in -1..=1 {
                for dy in -1..=1 {
                    let Some(list) = self.end_grid.get(&(cx + dx, cy + dy)) else {
                        continue;
                    };
                    for &(other, other_end) in list {
                        if other == span {
                            continue;
                        }
                        let Some(o) = self
                            .tiles
                            .get(&other.0)
                            .map(|t| t.spans[other.1].ends[other_end])
                        else {
                            continue;
                        };
                        let gap = at.point.distance(o.point);
                        // (one end's way out is the other's way in)
                        if gap <= JOIN_TOLERANCE && at.outward.dot(o.outward) <= -JOIN_MIN_ALIGNMENT
                        {
                            // (each joint names its lower end first, whatever came in first)
                            let (a, b) = if (span, end) < (other, other_end) {
                                ((span, end), (other, other_end))
                            } else {
                                ((other, other_end), (span, end))
                            };
                            self.joints.push(WireJoint {
                                a,
                                b,
                                gap,
                                reversed: end == other_end,
                                via: None,
                            });
                        }
                    }
                }
            }
            self.end_grid.entry((cx, cy)).or_default().push((span, end));
        }
        self.rebuild_bridges();
    }

    /// Take a tile's wire away (the tile was unloaded).
    pub fn remove_tile(&mut self, tile: (i32, i32)) {
        self.drop_tile(tile);
    }

    fn drop_tile(&mut self, tile: (i32, i32)) {
        let Some(old) = self.tiles.remove(&tile) else {
            return;
        };
        for c in self.tile_cells.remove(&tile).unwrap_or_default() {
            if let Some(refs) = self.grid.get_mut(&c) {
                refs.retain(|r| r.span.0 != tile);
                if refs.is_empty() {
                    self.grid.remove(&c);
                }
            }
        }
        for end in old.spans.iter().flat_map(|s| s.ends.iter()) {
            let c = cell_of(end.point, JOIN_TOLERANCE);
            if let Some(list) = self.end_grid.get_mut(&c) {
                list.retain(|(span, _)| span.0 != tile);
                if list.is_empty() {
                    self.end_grid.remove(&c);
                }
            }
        }
        self.joints.retain(|j| j.a.0 .0 != tile && j.b.0 .0 != tile);
        self.rebuild_bridges();
    }

    /// The joints across the gaps of the loaded switches and crossings (see
    /// [`BRIDGE_RADIUS`]), made again from scratch: a tile that comes or goes can add or take
    /// the ends on either side of a gap.
    fn rebuild_bridges(&mut self) {
        self.joints.retain(|j| j.via.is_none());
        let objects: Vec<WireObject> = self
            .tiles
            .values()
            .flat_map(|t| t.objects.iter())
            .filter(|o| o.kind != WireObjectKind::SoundMarker)
            .cloned()
            .collect();
        let reach = (BRIDGE_RADIUS / JOIN_TOLERANCE).ceil() as i64;
        let mut bridges: Vec<WireJoint> = Vec::new();
        for o in &objects {
            let (cx, cy) = cell_of(o.position, JOIN_TOLERANCE);
            let mut near: Vec<(SpanId, usize, SpanEnd)> = Vec::new();
            for dx in -reach..=reach {
                for dy in -reach..=reach {
                    for &(span, end) in self.end_grid.get(&(cx + dx, cy + dy)).into_iter().flatten()
                    {
                        let Some(e) = self.span(span).map(|s| s.ends[end]) else {
                            continue;
                        };
                        if (e.point - o.position).truncate().length() <= BRIDGE_RADIUS {
                            near.push((span, end, e));
                        }
                    }
                }
            }
            near.sort_by_key(|(span, end, _)| (*span, *end));
            for (n, (sa, ea, a)) in near.iter().enumerate() {
                for (sb, eb, b) in &near[n + 1..] {
                    if sa == sb {
                        continue;
                    }
                    let gap = a.point.distance(b.point);
                    if gap <= JOIN_TOLERANCE || gap > BRIDGE_MAX_GAP {
                        continue;
                    }
                    let d = (b.point - a.point) / gap;
                    let facing = a.outward.dot(b.outward) <= -BRIDGE_MIN_ALIGNMENT
                        && d.dot(a.outward) >= BRIDGE_MIN_ALIGNMENT
                        && (-d).dot(b.outward) >= BRIDGE_MIN_ALIGNMENT;
                    let pair = ((*sa, *ea), (*sb, *eb));
                    if facing && !bridges.iter().any(|j| (j.a, j.b) == pair) {
                        bridges.push(WireJoint {
                            a: pair.0,
                            b: pair.1,
                            gap,
                            reversed: ea == eb,
                            via: Some(o.kind),
                        });
                    }
                }
            }
        }
        self.joints.extend(bridges);
        self.joints.sort_by_key(|j| (j.a, j.b));
    }

    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    /// The loaded tiles' wire.
    pub fn tiles(&self) -> impl Iterator<Item = ((i32, i32), &TileWires)> {
        self.tiles.iter().map(|(k, t)| (*k, t))
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
