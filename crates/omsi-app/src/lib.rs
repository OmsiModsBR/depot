//! `omsi` - load an OMSI 2 map and render it.
//!
//! ```text
//! omsi --root "/path/to/OMSI 2" --map maps/Grundorf/global.cfg            # window
//! omsi --root ... --map ... --offscreen out.png --size 1600x900            # PNG
//! ```
//!
//! The game is this library; `main.rs` calls [`run`], and on Android the NativeActivity
//! calls `android_main` (see `core/implementation/android.rs`), which runs the launcher and
//! the game in one window of one process.

mod about;
mod actions;
mod ambience;
mod api_limits;
mod career;
mod describe;
mod driver;
mod editor;
mod env_file;
mod export;
mod game_controller_menu;
mod game_lists;
mod headtrack;
mod hud;
mod humans;
mod journey;
mod keys;
mod launcher;
mod lights;
#[cfg(target_os = "macos")]
mod mac_game_controller;
#[cfg(target_os = "macos")]
mod mac_hid;
mod map_surface;
mod menu;
mod money;
mod nav;
mod output;
mod placing;
mod radio;
mod rail_drive;
mod screenshot_name;
#[cfg(steam)]
mod steam;
mod touch;
mod tr_fill;
mod ui;

mod condensation;
mod line_times;
mod puddles;
mod quit;
mod rain;
mod real_time;
mod scene;
mod schedule;
mod schedule_paper;
mod settings;
mod tiles;
mod traffic;

mod app_events;
mod bus_service;
mod controllers;
mod core;
mod diagnostics;
#[cfg(windows)]
mod dinput;
mod duty_start;
#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
mod evdev_buttons;
#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
mod evdev_ff;
mod ffb_calibration;
mod frame_stats;
mod ground_gap;
mod hpattern;
mod input;
mod input_script;
mod lan_off;
mod launcher_link;
mod offscreen;
mod on_foot;
mod player;
mod plugins_off;
mod route_arrows;
use lan_off::{admin, lan};

mod services;
mod situation;
mod spawn;
mod state_trace;
mod teleport;
mod traffic_link;
mod tutorial;
mod weather_cycle;
mod weather_model;
mod weather_setup;
mod weather_time;
mod world_load;

/// The crate's insides for the tests in `tests/` (see `testing`).
#[cfg(feature = "test-support")]
pub mod testing;

// the interface's translations (locales/app.yml; the English text is the key)
rust_i18n::i18n!("locales");
// (the tables are read when this crate compiles: this makes cargo compile it again when they
// change - the macro alone left the old texts in the program)
const _LOCALES: &str = include_str!("../locales/app.yml");

/// Show the interface in `code` (the settings' ENG / DEU / FRA / RUS).
pub(crate) fn ui_language(code: &str) {
    omsi_ui::i18n::set_lookup(|lang, text| {
        _rust_i18n_try_translate(lang, text)
            .map(|t| t.into_owned())
            .or_else(|| tr_fill::fill(lang, text))
    });
    let iso = omsi_launcher_lib::language_iso(code);
    omsi_ui::i18n::set_language(iso);
    omsi_sim::vehicle_api::set_locale(iso);
}

use crate::core::cli::Args;
use crate::core::startup::{self, graphics_instance, BUILD, VERSION};
use crate::core::{launcher_statics, make_app, prepare};
use anyhow::Result;
use clap::Parser;
use launcher_link::open_launcher;
use winit::event_loop::EventLoop;

/// The game (and its launcher) from the command line: what `main` does.
pub fn run() -> Result<()> {
    let _ = startup::PROCESS_START.set(std::time::Instant::now());
    omsi_cfg::migrate_legacy_data_dir();
    #[cfg(target_os = "macos")]
    startup::restart_with_allocator_settings();
    #[cfg(windows)]
    // (not the preview helper: its output is the launcher's pipe, and a console taken over
    // from the launcher had no business with it)
    if !std::env::args().any(|a| a == "--preview-helper") {
        startup::attach_parent_console();
    }
    let mut logger =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"));
    // The launcher (started bare, or with --launcher) writes its lines to `launcher.log` as
    // well: a session's go to its `game.log`, but the launcher's own window has no console on
    // Windows, and what it said went nowhere.
    let launcher_process =
        std::env::args().len() == 1 || std::env::args().any(|a| a == "--launcher");
    if launcher_process {
        let file = omsi_launcher_lib::data_dir().join("launcher.log");
        let _ = std::fs::create_dir_all(omsi_launcher_lib::data_dir());
        if let Ok(f) = std::fs::File::create(&file) {
            logger.target(env_logger::Target::Pipe(Box::new(startup::Tee(f))));
        }
    }
    logger.init();
    // a panic goes into the log (which the launcher keeps per session) with where it
    // happened and a backtrace, not only to a terminal that may not be there
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if omsi_render::catching() {
            log::warn!("caught by the renderer: {info}");
            return;
        }
        log::error!(
            "the game stopped on an error (build {BUILD}): {info}\n{}",
            std::backtrace::Backtrace::force_capture()
        );
        default_hook(info);
    }));
    log::info!(
        "Project Depot {VERSION}, build {BUILD}{}",
        if std::env::var_os("MallocLargeCache").is_some() {
            " (large allocations returned at once)"
        } else {
            ""
        }
    );
    let args = Args::parse();
    // Started by a double click or with no arguments at all: that is the launcher's job.
    // The launcher itself runs the game with a full command line (--no-menu, --map, ...).
    let bare = std::env::args().len() == 1;
    let Some((args, server_cfg)) = prepare(args, bare)? else {
        return Ok(());
    };
    if args.preview_helper {
        return launcher::preview::helper_main();
    }
    if args.launcher || (bare && !args.menu) {
        // the launcher window (the game started again by it with a full command line);
        // OMSI_LAUNCHER=<program> still opens another launcher instead
        if omsi_cfg::flags::LAUNCHER.is_set() && open_launcher()? {
            return Ok(());
        }
        if let Some(pid) = args.parent_pid {
            startup::end_with(pid);
        }
        launcher_statics();
        return launcher::run(graphics_instance());
    }
    let Some(app) = make_app(args, server_cfg)? else {
        return Ok(());
    };
    let event_loop = EventLoop::new()?;
    // SIGTERM (the launcher's Stop) and Ctrl+C end the session the way Escape does
    let proxy = event_loop.create_proxy();
    quit::install(move |_| {
        let _ = proxy.send_event(());
    });
    let mut app = app;
    let r = event_loop.run_app(&mut app);
    // the game's GPU resources go with it: letting the device's queue go waits for the GPU,
    // and wgpu panics when a card that hangs does not answer in time - the game had ended in
    // order, and that panic was its crash in the report (#48)
    if !close_app(app) {
        log::warn!("the graphics device did not close in order (the game had ended already)");
    }
    r?;
    Ok(())
}

/// Let `app` go, a panic in letting it go caught (see `run`). Whether it went in order.
pub fn close_app<T>(app: T) -> bool {
    omsi_render::catch(move || drop(app)).is_some()
}
