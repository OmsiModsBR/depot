//! What the rest of the game reaches of the core, by subject: `App` and its procedures, the
//! command line, the start-up, the threads, the log, the memory budget, the platform and
//! Android's entry point.

/// The project's addresses, and opening its links in the browser.
pub(crate) mod about {
    pub(crate) use super::super::super::domain::about::*;
    pub(crate) use super::super::super::implementation::about::*;
}

/// `App`, its state and its procedures.
pub(crate) mod app {
    pub(crate) use super::super::super::domain::app::*;
    pub(crate) use super::super::super::implementation::app::*;
}
pub(crate) use super::super::domain::cli;
pub(crate) use super::super::implementation::launcher_link;

/// `depot/env.txt` on a phone: the switches it may set.
#[cfg(target_os = "android")]
pub(crate) mod env_file {
    pub(crate) use super::super::super::domain::env_file::*;
    pub(crate) use super::super::super::implementation::env_file::*;
}

/// File names of screenshots, and the notice raised once one is saved.
pub(crate) mod screenshot_name {
    pub(crate) use super::super::super::domain::screenshot_name::*;
    pub(crate) use super::super::super::implementation::screenshot_name::*;
}

/// The device's clock and the real-time sync (the game's date and time follow it).
pub(crate) mod real_time {
    pub(crate) use super::super::super::domain::real_time::*;
    pub(crate) use super::super::super::implementation::real_time::*;
}

/// What the game answers of a LAN session and the server admin list: none without LAN play.
pub(crate) mod lan {
    pub(crate) use super::super::super::domain::lan_off::lan::*;
}

/// The server admin list: never opened without LAN play.
pub(crate) mod admin {
    pub(crate) use super::super::super::domain::lan_off::admin::*;
}
#[cfg(steam)]
pub(crate) mod steam {
    pub(crate) use super::super::super::implementation::steam::*;
}
#[cfg(target_os = "android")]
pub(crate) use super::super::implementation::android;
pub(crate) use super::super::implementation::applog;
pub(crate) use super::super::implementation::make::{launcher_statics, make_app, prepare};
pub(crate) use super::super::implementation::memory;
pub(crate) use super::super::implementation::platform;
pub(crate) use super::super::implementation::startup;
pub(crate) use super::super::implementation::threads;
