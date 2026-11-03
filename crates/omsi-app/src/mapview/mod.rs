//! The Drive page's map: the chosen map as the game's own city map has it - every spline's
//! and every object's `[path]`, linked to its neighbours, grouped into carriageways with
//! their real widths, where its entry points stand and where its objects are - and the road
//! pieces the chosen trip drives over it.
//!
//! It reads the map the way the navigator does (`scene::navigation_map_of`, then
//! `nav::city_roads`), so what the launcher shows cannot drift from what the game
//! draws on its own map. What it leaves out is what choosing a duty does not need: no
//! meshes, no textures, no traffic, no timetable beyond the chosen trip. That read is the
//! one thing here that costs anything (the same 0.1 - 1.5 s the game's own map costs) and it
//! runs once per map selection, off the interface thread, with the page saying so while it
//! does.
//!
//! It is not a picture but a map: dragged to move, the wheel to zoom where the cursor is, an
//! entry point clicked to choose where the bus is put down. The chosen line's route runs over
//! the roads in red and its stops stand on it, their names and times written beside them by
//! the interface (`Launcher::map_labels`). The lists next to it do what a map cannot say, and
//! they stay.
//!
//! The lines are built once per zoom band - a point every fraction of a pixel is all the eye
//!
//! `domain` holds what a map is to the launcher (its roads, entry points and objects) and the
//! plan its lines are drawn from, `implementation` the view and the reading of a map, and
//! `interface` the game's side of the pages' map (`omsi_launcher_ui::window::Map`).

pub(crate) mod domain;
pub(crate) mod implementation;
pub(crate) mod interface;

pub use implementation::view::*;
pub(crate) use interface::map::GameMap;
