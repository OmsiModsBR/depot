//! The crate's unit tests, one module per module of `src/`, in one binary. What is internal
//! comes through `openomsi_game::testing` (the `test-support` feature).

mod about;
mod app_events;
mod core;
mod crate_root;
mod describe;
mod diagnostics;
mod env_file;
mod frame_stats;
mod gameplay;
mod hud;
mod input;
mod launcher;
mod lights;
mod mapview;
mod on_foot;
mod output;
mod player;
#[cfg(unix)]
mod quit;
mod radio;
mod screenshot_name;
mod services;
mod settings;
mod showroom;
mod situation;
mod tr_fill;
mod tutorial;
mod ui;
mod weather;
mod world;
