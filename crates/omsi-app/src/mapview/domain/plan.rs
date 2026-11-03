//! The plan: the roads and the route as one vertex list, drawn in pieces.

use omsi_ui::Vertex;
use std::path::PathBuf;

/// The largest vertex buffer the plan asks for. The launcher's device only promises the
/// downlevel 256 MB, and a buffer is made at the next power of two above what it holds: a
/// big map's plan (Mapa Transcol: 7.9 million vertices, 478 MB) asked for 512 MB, was refused
/// and drew no roads (#536). Half of the promise leaves room for that rounding.
pub const MAX_PLAN_BUFFER_BYTES: usize = 128 * 1024 * 1024;

/// The markers' buffer; the plan's pieces follow it, from [`PLAN_FIRST_BUFFER`].
pub(crate) const MARKS_BUFFER: usize = 0;
pub(crate) const PLAN_FIRST_BUFFER: usize = 1;

/// Where a list of `total` vertices is cut so that no piece passes `max_bytes`: every cut falls
/// between whole triangles (a multiple of three vertices), and the pieces drawn in order are
/// the list drawn at once.
pub fn plan_pieces(total: usize, max_bytes: usize) -> Vec<std::ops::Range<usize>> {
    let per = (max_bytes / std::mem::size_of::<Vertex>()).max(3) / 3 * 3;
    (0..total)
        .step_by(per)
        .map(|a| a..(a + per).min(total))
        .collect()
}

/// The plan (buffers from [`PLAN_FIRST_BUFFER`]): the roads and the route as one vertex list
/// drawn in pieces, and what it was built for - another map, another trip, or another zoom
/// band (whose own thinning it is).
pub struct Plan {
    pub key: Key,
    pub verts: Vec<Vertex>,
    pub uploaded: bool,
}

#[derive(Clone, PartialEq)]
pub struct Key {
    pub global: PathBuf,
    pub trip: String,
    /// The simplification the plan was built with, as the bit pattern of its metres.
    pub tolerance: u32,
}
