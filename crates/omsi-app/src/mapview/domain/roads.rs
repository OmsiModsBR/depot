//! A map as the launcher's map needs it: the roads, the entry points and the objects.

use glam::DVec2;
use hashbrown::HashMap;
use omsi_sim::traffic::Network;
use std::sync::Arc;

/// An entry point as the map draws it: where its marker stands, its name, and its own place
/// in `global.cfg`'s list. That place - not its place among the drawn markers - is what the
/// choice and the game count (`--entry` is the position in that list), so an entry point
/// whose position cannot be resolved leaves a gap that does not shift every marker after it
/// onto the wrong spawn.
pub struct Entry {
    /// Where the marker stands, in world metres.
    pub at: DVec2,
    /// Its name, as `global.cfg` writes it.
    pub name: String,
    /// Its position in `global.entry_points`.
    pub index: usize,
}

/// A map as the map needs it: the roads the game's own city map draws, the lanes they were
/// grouped from (the chosen trip's route runs on those), where its entry points stand and
/// where its objects do - all in world metres.
#[derive(Default)]
pub struct Roads {
    /// Every carriageway the map has, as the game groups them: points in world metres.
    pub roads: Vec<crate::hud::nav::MapRoad>,
    /// The linked network behind them (a trip's road pieces are lanes of it).
    pub net: Arc<Network>,
    /// (tile x, tile y, spline id, path) → the lane the timetable names.
    pub lanes: HashMap<(i32, i32, i64, u16), usize>,
    /// (tile x, tile y, spline id) → every lane of that spline (for a piece whose exact path
    /// is not there any more).
    pub splines: HashMap<(i32, i32, i64), Vec<usize>>,
    /// The world point the drawn lines are measured from: an f32 cannot hold a map's own
    /// millions of metres to the centimetre, and roads are drawn to the centimetre.
    pub origin: DVec2,
    /// The world rectangle the roads cover (the 1 - 99 % box: see `read_map`).
    pub lo: DVec2,
    pub hi: DVec2,
    /// The entry points that could be placed, in `global.cfg`'s order - each carrying its own
    /// place in that list (see `Entry`).
    pub entries: Vec<Entry>,
    /// Every placed object's world place: the trip's stops are found here.
    pub objects: HashMap<i64, DVec2>,
    /// The streets the map's name signs name, each once (any case), with where its first
    /// sign stands: the Timetable page's search finds them.
    pub streets: Vec<(String, DVec2)>,
    /// The map's own surface (`openomsi-realmap`: water, green, built-up land, rivers, rail,
    /// borders), drawn under the roads when the map's author ships it.
    pub surface: Option<Arc<crate::map_surface::MapSurface>>,
}

impl Roads {
    /// The lanes a trip's road piece runs on: the very `[path]` the timetable names, else -
    /// a map whose timetable still names a spline it no longer has - every lane of that
    /// spline, so a hole in the map's own data is not a hole in the picture.
    pub(crate) fn routes_of(&self, piece: &omsi_launcher_lib::RoadPiece) -> Vec<usize> {
        if let Some(&i) = self
            .lanes
            .get(&(piece.tile_x, piece.tile_y, piece.spline, piece.path))
        {
            return vec![i];
        }
        self.splines
            .get(&(piece.tile_x, piece.tile_y, piece.spline))
            .cloned()
            .unwrap_or_default()
    }
}
