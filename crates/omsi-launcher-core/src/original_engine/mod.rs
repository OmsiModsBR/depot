//! Opening the original OMSI 2 from Depot through OmsiLaunch (Windows only in the UI).
//!
//! OmsiLaunch is a separate program the user installs into their OMSI 2 folder; Depot only
//! runs it as a process (command line and JSON) and never links, copies or embeds it. The
//! pieces here are plain functions so they are tested on every platform; the launcher shows
//! the feature on Windows only and keeps it off until the player turns it on under Setup.

pub mod config;
pub mod detect;
pub mod runner;

pub use config::{Engine, OriginalEngineConfig};
