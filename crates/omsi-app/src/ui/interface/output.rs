//! What the screens take from the rest of the game: the game itself (their `impl App`
//! parts), the map's picture as the launcher reads it, the game's folder names and version,
//! and the window input of the new interface.

pub(in crate::ui) use crate::app::App;
pub(in crate::ui) use crate::launcher::MapPicture;
pub(in crate::ui) use crate::new_ui_add::WinInput;
pub(in crate::ui) use crate::screenshot_name::folder_of;
pub(in crate::ui) use crate::startup::VERSION;
