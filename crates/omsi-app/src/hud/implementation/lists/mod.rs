//! The game menu's own windows besides the administration (see `admin`), as OMSI has them
//! in its menus: the options that can change while driving, the line and tour to drive,
//! the driver whose personnel file the run goes into, and the bus's fleet number. Each is a
//! list of (label, action) lines in the menu's chooser; choosing a line does it and shows
//! the list again (or the next one: a line's tours).

pub(crate) mod chooser;
pub(crate) mod dropdown;
pub(crate) mod duty;
pub(crate) mod keyboard;
pub(crate) mod pages;
pub(crate) mod route;
pub(crate) mod rows;
pub(crate) mod select;
pub(crate) mod store;
pub(crate) mod values;
pub(crate) mod vehicles;
