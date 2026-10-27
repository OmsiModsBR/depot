//! The crate's unit tests, one module per module of `src/`, in one binary. What is internal
//! comes through `openomsi_game::testing` (the `test-support` feature).

mod about;
mod actions;
#[cfg(feature = "lan")]
mod admin;
mod bus_service;
mod camera_arm;
mod camera_util;
mod condensation;
mod controllers;
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
mod game_lists;
mod head_idle;
mod headtrack;
mod hpattern;
mod hud;
mod humans;
mod input;
mod journey;
mod keys;
#[cfg(feature = "lan")]
mod lan;
#[cfg(feature = "lan")]
mod lan_mods;
#[cfg(feature = "lan")]
mod lan_world;
mod lights;
mod line_times;
#[cfg(target_os = "macos")]
mod mac_game_controller;
mod map_surface;
mod mirror_hud;
mod money;
mod nav;
mod new_ui;
mod new_ui_add;
mod new_ui_alt;
mod new_ui_loading;
mod new_ui_pages;
mod new_ui_report;
mod new_ui_windows;
mod offscreen;
mod on_foot;
#[cfg(windows)]
mod openxr;
mod player;
#[cfg(feature = "plugins")]
mod plugin_ui;
#[cfg(feature = "plugins")]
mod plugins;
mod puddles;
#[cfg(unix)]
mod quit;
mod radio;
mod rain;
mod scene;
mod schedule;
mod schedule_paper;
mod screenshot_name;
#[cfg(feature = "server")]
mod server;
mod services;
mod situation;
mod startup;
mod tiles;
mod touch;
mod tr_fill;
mod traffic;
mod tutorial;
mod ui;
#[cfg(feature = "lan")]
mod voice;
mod vr_navigator;
mod weather_cycle;
mod weather_model;
mod weather_setup;
mod world_load;
