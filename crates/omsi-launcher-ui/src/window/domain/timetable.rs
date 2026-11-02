//! What the Timetable page keeps between frames: the map's lines as read and as edited, the
//! plan the Map and Schematic tabs draw, and what is chosen on them.

use super::super::interface::output::core;
use super::timetable_plan::Plan;
use glam::DVec2;
use omsi_timetable::TimetableData;
use omsi_ui::Color;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Default)]
pub struct TimetableView {
    pub map: usize,
    /// The map the data was read for (its `global.cfg`).
    pub(crate) loaded: Option<String>,
    pub(crate) data: Option<TimetableData>,
    pub(crate) map_dir: PathBuf,
    /// The AI groups the map's own `ailists.cfg` has (name, a depot's group), read with the
    /// map: the groups a tour can take its buses from.
    pub(crate) ai_groups: Vec<(String, bool)>,
    pub(crate) line: usize,
    pub(crate) tour: usize,
    /// The departures being typed, one per trip of the shown tour.
    pub(crate) times: Vec<String>,
    pub(crate) times_for: Option<(usize, usize)>,
    /// Minutes a copied tour runs after the one it copies (and the repeat's interval).
    pub(crate) offset: String,
    /// The repeat's last departure ("h:mm").
    pub(crate) until: String,
    /// The name typed for a new line.
    pub(crate) new_line: String,
    /// The lines changed and not saved yet (by name): kept while the player moves between
    /// lines - the page used to drop a line's changes when another was clicked, and a day's
    /// timetable took a save after every line.
    pub(crate) dirty: std::collections::BTreeSet<String>,
    /// The reset button was pressed once: the next press puts the map's own timetable back.
    pub(crate) reset_armed: bool,
    /// The tab: 0 the map, 1 the schematic, 2 the trips.
    pub tab: usize,
    /// The map's `global.cfg` (the map tab reads it).
    pub(crate) global: PathBuf,
    /// What the lines run (their stops and times) for the map and the start's date, read in
    /// the background; what it was read for, and why it could not be.
    pub(crate) plan: Arc<Plan>,
    pub(crate) plan_for: Option<(String, String)>,
    pub(crate) plan_rx: Option<std::sync::mpsc::Receiver<Result<Plan, String>>>,
    pub(crate) plan_error: Option<String>,
    /// Every line's name the page knows (the map's and those the date runs), lower case and
    /// sorted: a line's colour is its place here, the same in every list and on the map.
    pub(crate) palette: Vec<String>,
    /// The line the Map and Schematic tabs have chosen (by name): one the date runs may not
    /// be one of the map's own files (a chrono line).
    pub(crate) chosen: String,
    /// The lines hidden on the map (by name).
    pub(crate) hidden: std::collections::BTreeSet<String>,
    /// The stop chosen on the map (by name), what the search holds, and whether its list of
    /// what it found is open.
    pub(crate) stop: Option<String>,
    pub(crate) find: String,
    pub(crate) find_open: bool,
    /// The direction the schematic draws: which of the line's termini it goes to.
    pub(crate) dir: usize,
    /// The lines' roads on the map, worked out once per plan and reading of the map.
    pub(crate) roads: Option<LineRoads>,
}

/// Every line's roads in world metres (the lanes its drawn trips drive, by road piece, a piece
/// two ways share once), for the plan and the map reading they were worked out for; and how
/// many stretches of the routes the map or its timetable has no road for (they are not
/// drawn).
pub(crate) struct LineRoads {
    pub(crate) plan: usize,
    pub(crate) map: usize,
    pub(crate) lines: Vec<(String, Vec<PieceLanes>)>,
    pub(crate) missing: usize,
}

/// A row of the lines card: name, title, what is under it, colour, hidden.
pub(crate) type LineRow = (String, String, String, Color, bool);

/// A road piece and the lanes drawn for it.
pub(crate) type PieceLanes = (core::RoadPiece, Vec<Vec<DVec2>>);

impl TimetableView {
    /// The stop `name` chosen on the map.
    pub fn set_stop(&mut self, name: &str) {
        self.stop = Some(name.to_string());
    }

    /// The line `name` chosen on the Map and Schematic tabs.
    pub fn set_line(&mut self, name: &str) {
        self.chosen = name.to_string();
    }

    /// `q` typed in the map's search, its list open.
    pub fn set_find(&mut self, q: &str) {
        self.find = q.to_string();
        self.find_open = !q.trim().is_empty();
    }
}
