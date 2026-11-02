//! The mouse over the game's map, as the pages that pick a start point on it share it.

use super::super::domain::frame::Frame;
use super::super::domain::map::{Map, Pointer};
use omsi_ui::Rect;

/// The mouse over the map in `r`: what it drags, where it zooms, and the entry point a click
/// takes. `panels_first`: the map lies under the page's panels, which have the first claim on
/// the mouse (a map inside a window of its own: the window's darkened frame claims the whole
/// screen, so only the map's own rectangle decides).
pub(crate) fn map_pointer(
    f: &mut Frame,
    mv: &mut dyn Map,
    r: Rect,
    window: Rect,
    panels_first: bool,
) {
    let p = Pointer {
        at: f.ui.input.mouse,
        pressed: f.ui.input.pressed,
        released: f.ui.input.released,
        down: f.ui.input.down,
        wheel: f.ui.input.wheel.y,
        blocked: (panels_first && f.ui.over_ui) || !r.contains(f.ui.input.mouse),
    };
    mv.think(r, window, f.ui.scale, p);
    if let Some(i) = mv.take_clicked() {
        if f.state.choice.entry != i as i32 {
            log::info!(
                "launcher map: entry point {} of the map's list taken from the map",
                i + 1
            );
            f.state.choice.entry = i as i32;
            f.state.touched();
        }
    }
}
