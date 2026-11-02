//! What the Diagnostics page's AI traffic tab keeps between frames.

use super::ai_lists::Editor;
use omsi_integrity::AiTraffic;
use omsi_ui::Rect;
use std::sync::mpsc::Receiver;
use std::sync::Arc;

/// The AI traffic tab's map, day, tab and choices, what was read, and the editing.
#[derive(Default)]
pub struct AiView {
    /// The map looked at (its place in the map list), once the launcher's own was taken.
    pub(crate) item: Option<usize>,
    /// The day, YYYY-MM-DD.
    pub(crate) date: String,
    /// The map's folder and day being read, and the answer.
    pub(crate) loading: Option<(String, i32, Receiver<AiTraffic>)>,
    /// The map's folder and day read, and what was read.
    pub(crate) data: Option<(String, i32, Arc<AiTraffic>)>,
    /// The OMSI 2 folder and the content's generation (`omsi_cfg::content_generation`) of what
    /// was read: another folder, or a change to the content (a timetable saved, a mod
    /// installed), reads it again.
    pub(crate) seen: Option<(String, u64)>,
    /// 0 car groups, 1 bus fleet and lines, 2 parked cars, 3 people, 4 traffic density.
    pub(crate) tab: usize,
    pub(crate) group: usize,
    pub(crate) vehicle: Option<usize>,
    pub(crate) line: usize,
    pub(crate) tour: usize,
    /// (vehicle type, entry) of the fleet.
    pub(crate) bus: Option<(usize, usize)>,
    pub(crate) park: usize,
    pub(crate) car: Option<usize>,
    /// 0 `humans.txt`, 1 `drivers.txt`.
    pub(crate) people: usize,
    pub(crate) person: Option<usize>,
    /// 0 weekdays, 1 Saturday, 2 Sunday or holiday.
    pub(crate) day: usize,
    /// The map's `global.cfg` (as the map list names it), for the preview's light.
    pub(crate) map_file: String,
    /// The last frame showed a vehicle's preview (the page's own bus is not asked for then).
    pub(crate) showing: bool,
    /// The map's lists being edited.
    pub edit: Option<Editor>,
    /// The vehicle this frame wants in the preview: the game draws it once the tab is drawn.
    pub picture: Option<Picture>,
}

/// A vehicle shown in the game's preview: where, the map whose light it stands in, the
/// vehicle file and its paint (empty: its own), and the day.
pub struct Picture {
    pub rect: Rect,
    pub map: String,
    pub bus: String,
    pub paint: String,
    pub date: String,
}

impl AiView {
    /// The tab shows the preview of one of its vehicles.
    pub fn shows_vehicle(&self) -> bool {
        self.showing
    }

    /// A window of the editing lies over the tab.
    pub fn dialog_open(&self) -> bool {
        self.edit.as_ref().is_some_and(|e| e.dialog_open())
    }
}
