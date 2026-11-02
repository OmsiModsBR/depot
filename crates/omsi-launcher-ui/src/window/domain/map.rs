//! The map a page draws behind its panels (the Timetable page's lines, the Drive page's
//! trip): the game reads it as its own city map does and draws it, so the pages ask it
//! through `Map`, which the game implements.

use super::super::interface::output::{core, Ui};
use glam::{DVec2, Vec2};
use omsi_ui::Rect;
use std::path::PathBuf;

/// What the map should show. Changing `map` reads the tiles again; changing `trip` only walks
/// the timetable; changing `entry` only moves the ring.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct MapLook {
    /// The map as the launcher names it (`maps/<name>/global.cfg`), and where that is.
    pub map: String,
    pub global: PathBuf,
    /// The date the duty starts on (the chrono folders change the map and the timetable).
    pub date: String,
    /// The trip whose route is drawn (empty: the map alone).
    pub trip: String,
    /// The entry point the player picked, -1 = automatic (nothing is ringed).
    pub entry: i32,
}

/// One frame of the mouse over the map. `blocked` is what a panel or a field over it takes.
#[derive(Clone, Copy, Default)]
pub struct Pointer {
    pub at: Vec2,
    pub pressed: bool,
    pub released: bool,
    pub down: bool,
    pub wheel: f32,
    pub blocked: bool,
}

/// The game's map, as a page uses it.
pub trait Map {
    /// The map to show (read again, off the interface thread, when it changed).
    fn want(&mut self, look: MapLook);
    /// The map across `r`, or a word while it is read.
    fn background(&mut self, ui: &mut Ui, r: Rect);
    /// The map has been read.
    fn ready(&self) -> bool;
    /// Which reading of the map is shown (another when it is read again).
    fn roads_key(&self) -> usize;
    /// The lanes the road pieces drive, in world metres, and how many of the pieces the map
    /// has no road for.
    fn lanes_of(&self, pieces: &[core::RoadPiece]) -> (Vec<Vec<DVec2>>, usize);
    /// A world point on the screen.
    fn project(&self, p: DVec2) -> Vec2;
    /// Where the map object `id` stands.
    fn place_of(&self, id: i64) -> Option<DVec2>;
    /// The streets the map's signs name, and where.
    fn streets(&self) -> &[(String, DVec2)];
    fn zoom(&mut self, by: f64);
    /// The whole map in view again.
    fn refit(&mut self);
    /// Look at world point `p`.
    fn look_at(&mut self, p: DVec2);
    /// The mouse this frame: drag to move, the wheel to zoom.
    fn think(&mut self, rect: Rect, window: Rect, scale: f32, p: Pointer);
    /// The entry point clicked, once.
    fn take_clicked(&mut self) -> Option<usize>;
}
