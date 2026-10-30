//! What the rest of the game reaches of the core, by subject: `App` and its procedures, the
//! command line, the start-up, the threads, the log, the memory budget, the platform and
//! Android's entry point.

/// `App`, its state and its procedures.
pub(crate) mod app {
    pub(crate) use super::super::super::domain::app::*;
    pub(crate) use super::super::super::implementation::app::*;
}
pub(crate) use super::super::domain::cli;
#[cfg(target_os = "android")]
pub(crate) use super::super::implementation::android;
pub(crate) use super::super::implementation::applog;
pub(crate) use super::super::implementation::make::{launcher_statics, make_app, prepare};
pub(crate) use super::super::implementation::memory;
pub(crate) use super::super::implementation::platform;
pub(crate) use super::super::implementation::startup;
pub(crate) use super::super::implementation::threads;
