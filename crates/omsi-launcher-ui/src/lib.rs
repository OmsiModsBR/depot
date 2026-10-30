//! The launcher's look and its new pages, drawn with `omsi-ui`, with no 3D world in them.
//!
//! The game's launcher window (`omsi-app::launcher`) draws the sidebar (`shell::Sidebar`) and
//! the Vehicles and Maps pages from here; the Maps page and the game's loading screen read a
//! map's picture with `picture`. `Shell` puts the same sidebar and pages together without the
//! game, for pictures (`offscreen`, the `launcher_shot` example) and tests.

pub mod kit;
pub mod look;
pub mod maps;
pub mod offscreen;
pub mod pages;
pub mod picture;
pub mod sample;
pub mod shell;
pub mod vehicles;

pub use pages::Page;
pub use shell::Shell;
