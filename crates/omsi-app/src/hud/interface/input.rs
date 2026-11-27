//! What the game hands the HUD each frame and calls on it, and what it reads back (where
//! the menu's lines and controls were drawn, for the mouse).

pub use super::super::domain::{
    ChatView, DropdownView, Frame, Hits, InfoData, InfoMode, MenuKind, Preview,
};
pub use super::super::implementation::info_bar::info_bar;
pub(crate) use super::super::implementation::legacy::shift_overlays;
pub use super::super::implementation::legacy::{backdrop, size_factor, Hud, Ui};

/// The readable names of the cockpit controls (the language files and the translation).
pub(crate) mod describe {
    pub(crate) use super::super::super::domain::describe::*;
    pub(crate) use super::super::super::implementation::describe::*;
}

/// The text templates of the tables: the values of a text the game built, and its translation.
pub(crate) mod tr_fill {
    pub(crate) use super::super::super::implementation::tr_fill::*;
}

/// The start menu: its choices and the reading of the folders behind them.
pub(crate) mod start_menu {
    pub(crate) use super::super::super::domain::menu::*;
}

/// The menu's lists and settings windows, by subject.
pub(crate) mod lists {
    pub(crate) use super::super::super::domain::lists::*;
    pub(crate) use super::super::super::implementation::lists::{
        chooser::*, dropdown::*, duty::*, keyboard::*, pages::*, route::*, rows::*, store::*,
        values::*, vehicles::*,
    };
}

/// The route navigator, the minimap and the city map.
pub(crate) mod nav {
    pub(crate) use super::super::super::domain::nav::*;
    pub(crate) use super::super::super::implementation::nav::*;
}
