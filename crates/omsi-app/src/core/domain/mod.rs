//! What the core is about: `App`'s state and the command line it starts from.

pub(crate) mod about;
pub(crate) mod app;
pub(crate) mod cli;
pub(crate) mod launcher_link;
pub(crate) mod lan_off;
#[cfg(steam)]
pub(crate) mod steam;
