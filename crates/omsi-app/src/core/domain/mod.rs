//! What the core is about: `App`'s state and the command line it starts from.

pub(crate) mod about;
pub(crate) mod app;
pub(crate) mod cli;
pub(crate) mod env_file;
pub(crate) mod lan_off;
pub(crate) mod launcher_link;
pub(crate) mod plugins_off;
pub(crate) mod real_time;
pub(crate) mod screenshot_name;
pub(crate) mod services;
#[cfg(steam)]
pub(crate) mod steam;
