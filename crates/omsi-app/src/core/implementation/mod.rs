//! The core's procedures: `App`'s frame and loading work, the start-up before a window and
//! `make_app`, the threads, the log, the memory budget, the platform's few differences and
//! Android's entry point.

#[cfg(target_os = "android")]
pub(crate) mod android;
pub(crate) mod about;
pub(crate) mod app;
pub(crate) mod applog;
pub(crate) mod launcher_link;
pub(crate) mod lan_off;
pub(crate) mod make;
pub(crate) mod memory;
pub(crate) mod platform;
#[cfg(steam)]
pub(crate) mod steam;
pub(crate) mod startup;
pub(crate) mod threads;
