//! The catalogue laid out for a sky shader: the sphere cut into the six faces of a cube,
//! each face into cells, up to two stars a cell, so that a pixel finds the stars about it
//! by reading the few cells it lies between.

use super::{Star, StarCatalog};

/// Cells across a face of the cube (about 0.35 degrees each at a face's middle): big
/// enough that a star a couple of pixels wide covers at most the four cells about a
/// pixel. With two stars a cell only the stock catalogue's few triples lose one.
pub const STAR_MAP_CELLS: u32 = 256;

/// The star map: RGBA8, `6 * cells` wide and `2 * cells` high: face after face (+x, -x,
/// +y, -y, +z, -z of the equatorial frame) across, and two layers down (a cell's brighter
/// star in the top one, its second in the bottom one). A cell's texel holds its star's
/// place inside the cell (R across, G up: `(byte + 0.5) / 256` of the cell), its size (B:
/// 0 an empty cell, else `1 + 254 * min(4 * sqrt(flux), 8) / 8`) and its class
/// ([`super::StarClass::code`]).
#[derive(Debug, Clone, PartialEq)]
pub struct StarMap {
    pub cells: u32,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// Stars left out because two brighter ones already held their cell.
    pub crowded: usize,
}

/// The face of the cube a direction falls on and its place on that face (-1 .. 1 each),
/// as the sky shaders find them (`star_face`).
pub fn face_of(d: [f64; 3]) -> (usize, f64, f64) {
    let a = [d[0].abs(), d[1].abs(), d[2].abs()];
    if a[0] >= a[1] && a[0] >= a[2] {
        (usize::from(d[0] <= 0.0), d[1] / a[0], d[2] / a[0])
    } else if a[1] >= a[2] {
        (2 + usize::from(d[1] <= 0.0), d[0] / a[1], d[2] / a[1])
    } else {
        (4 + usize::from(d[2] <= 0.0), d[0] / a[2], d[1] / a[2])
    }
}

/// How large a star is drawn: `4 * sqrt(flux)`, at most 8 (a star of magnitude 0 gives 4).
pub fn star_size(s: &Star) -> f32 {
    (4.0 * s.flux.max(0.0).sqrt()).min(8.0)
}

impl StarMap {
    pub fn new(catalog: &StarCatalog, cells: u32) -> StarMap {
        let n = cells.max(1);
        let width = 6 * n;
        let height = 2 * n;
        let mut rgba = vec![0u8; (width * height * 4) as usize];
        let mut crowded = 0;
        // the brightest first: a cell keeps the first two stars that come to it
        let mut order: Vec<&Star> = catalog.stars.iter().collect();
        order.sort_by(|a, b| b.flux.total_cmp(&a.flux));
        for s in order {
            let (face, u, v) = face_of(s.direction());
            let place = |t: f64| {
                let g = ((t + 1.0) * 0.5 * n as f64).clamp(0.0, n as f64 - 1e-9);
                let cell = g.floor();
                (cell as u32, ((g - cell) * 256.0).floor().min(255.0) as u8)
            };
            let (i, fu) = place(u);
            let (j, fv) = place(v);
            let texel = |layer: u32| (((layer * n + j) * width + face as u32 * n + i) * 4) as usize;
            let Some(at) = [texel(0), texel(1)]
                .into_iter()
                .find(|&at| rgba[at + 2] == 0)
            else {
                crowded += 1;
                continue;
            };
            let size = 1 + (254.0 * star_size(s) / 8.0).round() as u8;
            rgba[at..at + 4].copy_from_slice(&[fu, fv, size, s.class.code()]);
        }
        StarMap {
            cells: n,
            width,
            height,
            rgba,
            crowded,
        }
    }
}
