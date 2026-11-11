//! The overhead wire network of the loaded tiles.
//!
//! Positions are in the game's global frame (metres; x east, y north, z up).

use std::collections::HashMap;

use glam::DVec3;

/// How far apart two span ends may lie and still be one joint. Depot's own choice: loose
/// joints of the content (up to about a metre) must join, a gap clearly larger than that
/// is a gap in the wire.
pub const JOIN_TOLERANCE: f64 = 1.5;

/// How well two span ends must line up to join: the cosine of the angle between one
/// end's way out and the other's way in.
pub(crate) const JOIN_MIN_ALIGNMENT: f64 = 0.9;

/// Side of the grid's square cells, metres.
pub(crate) const GRID_CELL: f64 = 20.0;

/// A switch or crossing object draws the wire across a gap: span ends within this many
/// metres of it that face each other across at most [`BRIDGE_MAX_GAP`] metres are joined
/// through it. Depot's own choice, from the gaps the content leaves (about 3 to 4 m).
pub(crate) const BRIDGE_RADIUS: f64 = 8.0;
pub(crate) const BRIDGE_MAX_GAP: f64 = 6.0;
/// How well two ends must face each other across the gap (cosine of 25 degrees): each
/// end's way out points at the other end, and the two ways out are opposite.
pub(crate) const BRIDGE_MIN_ALIGNMENT: f64 = 0.906;

/// Which of the two wires of a span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WireSide {
    /// The wire at the convention's `wire_x_left` (left of the spline's direction).
    Left,
    /// The wire at `wire_x_right`.
    Right,
}

impl WireSide {
    pub const BOTH: [WireSide; 2] = [WireSide::Left, WireSide::Right];

    pub fn index(self) -> usize {
        match self {
            WireSide::Left => 0,
            WireSide::Right => 1,
        }
    }
}

/// One end of a span: the point midway between its two wires and the direction leaving
/// the span there (unit).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpanEnd {
    pub point: DVec3,
    pub outward: DVec3,
}

/// One wire spline: its two wires as polylines along the spline.
#[derive(Debug, Clone, PartialEq)]
pub struct WireSpan {
    /// The spline's id in the map.
    pub spline_id: i64,
    /// The spline's `.sli` path as the map writes it.
    pub file: String,
    pub length: f64,
    /// Points of the left and right wire, start to end (index by [`WireSide::index`]).
    pub wires: [Vec<DVec3>; 2],
    /// Start (0) and end (1).
    pub ends: [SpanEnd; 2],
}

/// What a wire object of the map is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WireObjectKind {
    /// A switch whose two branches part symmetrically.
    SwitchY,
    /// A switch whose branch leaves to the left of the straight wire.
    SwitchBranchLeft,
    /// A switch whose branch leaves to the right of the straight wire.
    SwitchBranchRight,
    /// A crossing of two lines.
    Crossing,
    /// A marker that only carries a sound.
    SoundMarker,
}

/// The file-name markers that make a map object a wire object (matched case-insensitively
/// against the object's whole path, so a folder name counts too).
pub(crate) const OBJECT_MARKERS: &[(&str, WireObjectKind)] = &[
    // (20 degree branch switches made from the Y's object, named after their side)
    ("desvio20_dir", WireObjectKind::SwitchBranchRight),
    ("desvio20_esq", WireObjectKind::SwitchBranchLeft),
    ("troll_strelka_out_02", WireObjectKind::SwitchY),
    ("troll_strelka_out_03", WireObjectKind::SwitchY),
    (
        "chave_segmento_esquerda_depot",
        WireObjectKind::SwitchBranchLeft,
    ),
    (
        "chave_segmento_direita_depot",
        WireObjectKind::SwitchBranchRight,
    ),
    ("twokrest", WireObjectKind::Crossing),
    ("onkrest", WireObjectKind::Crossing),
    ("troll_podves_02", WireObjectKind::SoundMarker),
];

/// A wire object placed in the map.
#[derive(Debug, Clone, PartialEq)]
pub struct WireObject {
    pub kind: WireObjectKind,
    pub object_id: i64,
    pub position: DVec3,
    /// The object's heading as the map records it, degrees.
    pub heading_deg: f64,
}

/// The wire of one tile.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TileWires {
    pub tile: (i32, i32),
    pub spans: Vec<WireSpan>,
    pub objects: Vec<WireObject>,
}

impl TileWires {
    pub fn is_empty(&self) -> bool {
        self.spans.is_empty() && self.objects.is_empty()
    }
}

/// A span of the network: its tile and its index in that tile's spans.
pub type SpanId = ((i32, i32), usize);

/// Two span ends that meet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WireJoint {
    /// (span, end: 0 start, 1 end) of each side.
    pub a: (SpanId, usize),
    pub b: (SpanId, usize),
    /// Distance between the two end points, metres.
    pub gap: f64,
    /// The two spans run in opposite senses (end meets end, or start meets start).
    pub reversed: bool,
    /// The wire object whose gap the joint bridges (a switch or a crossing draws the wire
    /// between the two ends); None for ends that meet.
    pub via: Option<WireObjectKind>,
}

/// One straight piece of one wire: points `index` and `index + 1` of the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SegmentRef {
    pub span: SpanId,
    pub side: WireSide,
    pub index: usize,
}

/// The wire of the loaded tiles: spans, the joints between them and a grid to find the
/// wire near a point.
#[derive(Debug, Clone, Default)]
pub struct WireNetwork {
    pub(crate) tiles: HashMap<(i32, i32), TileWires>,
    pub(crate) joints: Vec<WireJoint>,
    pub(crate) grid: HashMap<(i64, i64), Vec<SegmentRef>>,
    /// The grid cells each tile's segments are in (to take them out with the tile).
    pub(crate) tile_cells: HashMap<(i32, i32), Vec<(i64, i64)>>,
    /// Span ends by cell of [`JOIN_TOLERANCE`] size, to find the ends a new tile's ends meet.
    pub(crate) end_grid: HashMap<(i64, i64), Vec<(SpanId, usize)>>,
}
