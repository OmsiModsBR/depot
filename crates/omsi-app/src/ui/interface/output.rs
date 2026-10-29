//! What the screens take from the rest of the game: the game itself (their `impl App`
//! parts), the map's picture as the launcher reads it, the game's folder names and version,
//! the menu lists and the duty the Alt bar's windows show, and what the screens still share
//! with the `new_ui*` modules (the scale).

pub(in crate::ui) use crate::app::App;
pub(in crate::ui) use crate::game_lists::{self, ListKind, Move};
pub(in crate::ui) use crate::hud::{Frame, Hits, MenuKind};
pub(in crate::ui) use crate::input::ROUTE_NUMBER_MAX;
pub(in crate::ui) use crate::launcher::MapPicture;
pub(in crate::ui) use crate::new_ui::{scale_for, Area, MenuFrame};
pub(in crate::ui) use crate::schedule::PlayerDuty;
pub(in crate::ui) use crate::screenshot_name::folder_of;
pub(in crate::ui) use crate::startup::VERSION;
