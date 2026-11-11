//! What the tests in `tests/` need to build wire by hand.

use crate::domain::convention::ConventionParams;
use crate::implementation::convention::parse_params;
use crate::interface::output::{MapObject, MapSpline, Tile};

/// Parameters with the given keywords and wires 0.3 m either side, 5.8 m up.
pub fn params(keywords: &str) -> ConventionParams {
    parse_params(&format!(
        "[global]\nspline_keywords={keywords}\nwire_height=5.8\nwire_x_left=-0.3\nwire_x_right=0.3\n"
    ))
    .expect("test parameters")
}

/// A spline record: start (tile-local), heading (degrees, 0 = north, clockwise), length and
/// radius (0 straight, > 0 turning right).
pub fn spline(
    id: i64,
    file: &str,
    pos: [f64; 3],
    heading: f64,
    length: f64,
    radius: f64,
) -> MapSpline {
    MapSpline {
        file: file.to_string(),
        id,
        pos,
        heading,
        length,
        radius,
        ..MapSpline::default()
    }
}

/// The same spline, mirrored.
pub fn mirrored(mut s: MapSpline) -> MapSpline {
    s.mirror = true;
    s
}

/// An object record.
pub fn object(id: i64, file: &str, pos: [f64; 3], heading: f64) -> MapObject {
    MapObject {
        file: file.to_string(),
        id,
        pos,
        rot: [heading, 0.0, 0.0],
        ..MapObject::default()
    }
}

/// A tile with these splines and objects.
pub fn tile(splines: Vec<MapSpline>, objects: Vec<MapObject>) -> Tile {
    Tile {
        splines,
        objects,
        ..Tile::default()
    }
}
