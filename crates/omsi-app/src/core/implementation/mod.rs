//! The core's procedures: `App`'s frame and loading work, the start-up before a window and
//! `make_app`, the threads, the log, the memory budget, the platform's few differences and
//! Android's entry point.

pub(crate) mod about;
#[cfg(target_os = "android")]
pub(crate) mod android;
pub(crate) mod app;
pub(crate) mod applog;
pub(crate) mod env_file;
pub(crate) mod lan_off;
pub(crate) mod launcher_link;
pub(crate) mod make;
pub(crate) mod memory;
pub(crate) mod platform;
pub(crate) mod plugins_off;
pub(crate) mod real_time;
pub(crate) mod screenshot_name;
pub(crate) mod services;
pub(crate) mod situation;
pub(crate) mod startup;
#[cfg(steam)]
pub(crate) mod steam;
pub(crate) mod threads;
