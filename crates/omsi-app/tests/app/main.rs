//! The crate's unit tests, one module per module of `src/`, in one binary. What is internal
//! comes through `openomsi_game::testing` (the `test-support` feature).

mod about;
mod actions;
mod api_limits;
mod app_events;
mod bus_service;
mod condensation;
mod controllers;
mod core;
mod crate_root;
mod describe;
mod diagnostics;
#[cfg(windows)]
mod dinput;
mod editor;
mod env_file;
#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
mod evdev_buttons;
#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
mod evdev_ff;
mod ffb_calibration;
mod frame_stats;
mod game_controller_menu;
mod headtrack;
mod hpattern;
mod hud;
mod humans;
mod input;
mod journey;
mod keys;
mod launcher;
mod lights;
mod line_times;
#[cfg(target_os = "macos")]
mod mac_game_controller;
mod map_surface;
mod money;
mod on_foot;
mod output;
mod player;
mod puddles;
#[cfg(unix)]
mod quit;
mod radio;
mod rain;
mod scene;
mod schedule;
mod schedule_paper;
mod screenshot_name;
mod services;
mod settings;
mod situation;
mod tiles;
mod touch;
mod tr_fill;
mod traffic;
mod tutorial;
mod ui;
mod weather_cycle;
mod weather_model;
mod weather_setup;
mod world_load;
