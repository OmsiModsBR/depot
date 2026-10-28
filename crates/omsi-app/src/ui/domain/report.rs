//! The end-of-trip report's state: what it shows, whether the game was paused before it, and
//! the watch on a trip's last stop that opens it.

use omsi_ui::ingame::ReportRow;

/// What the overlay draws of the report, owned so that it can cross to the renderer.
#[derive(Clone, Debug, PartialEq)]
pub struct ReportFrame {
    pub title: String,
    pub note: &'static str,
    pub rows: Vec<ReportRow>,
    pub totals: Vec<(&'static str, String)>,
}

/// The report on the game's side.
#[derive(Debug)]
pub(crate) struct Report {
    pub(in crate::ui) frame: ReportFrame,
    /// Whether the game was paused before the report paused it.
    pub(in crate::ui) paused_before: bool,
    /// A press was taken: its release goes to the report too.
    pub(in crate::ui) pressed: bool,
}

impl Report {
    /// The window lost the mouse: the release of a press it took will not come.
    pub(crate) fn forget_press(&mut self) {
        self.pressed = false;
    }
}

/// A trip at its last stop: (line, tour, trip in the tour), its place in the duty, where
/// the stop is.
pub(crate) type Watch = (TripKey, usize, glam::DVec3);

/// Within this of the last stop, stopped with a passenger door open, the trip is over (the
/// duty's own distance for being at a stop).
const AT_TERMINUS: f64 = 25.0;
/// Further than this from it, the bus has left without opening (no report).
const LEFT_TERMINUS: f64 = 60.0;

/// What becomes of the watch on a trip's end: `Some(true)` the report opens, `Some(false)`
/// the watch ends without one, `None` it goes on. `still_done`: the duty is still on that
/// trip, at its end.
pub fn watch_outcome(distance: f64, doors: bool, still_done: bool) -> Option<bool> {
    if distance < AT_TERMINUS && doors {
        Some(true)
    } else if distance > LEFT_TERMINUS && !still_done {
        Some(false)
    } else {
        None
    }
}

/// A trip: (line, tour, trip in the tour).
pub type TripKey = (String, String, usize);

/// One duty update for the watch: `done` the trip at its last stop (if the duty is on one),
/// `shown` the trip whose report has opened last, `bus` where the bus is, `doors` whether it
/// stands with a passenger door open. The trip whose report opens now, with its place in the
/// duty.
pub fn watch_step(
    watch: &mut Option<Watch>,
    shown: Option<&TripKey>,
    done: Option<Watch>,
    bus: glam::DVec3,
    doors: bool,
) -> Option<(TripKey, usize)> {
    let still_done = done.is_some();
    if let Some(w) = done {
        if shown != Some(&w.0) {
            *watch = Some(w);
        }
    }
    let (_, _, at) = watch.as_ref()?;
    match watch_outcome((bus - *at).length(), doors, still_done)? {
        true => watch.take().map(|(key, trip, _)| (key, trip)),
        false => {
            *watch = None;
            None
        }
    }
}
