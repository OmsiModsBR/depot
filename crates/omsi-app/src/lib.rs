//! `omsi` - load an OMSI 2 map and render it.
//!
//! ```text
//! omsi --root "/path/to/OMSI 2" --map maps/Grundorf/global.cfg            # window
//! omsi --root ... --map ... --offscreen out.png --size 1600x900            # PNG
//! ```
//!
//! The game is this library; `main.rs` calls [`run`], and on Android the NativeActivity
//! calls `android_main` (see `android.rs`), which runs the launcher and the game in one
//! window of one process.

mod about;
mod actions;
mod ambience;
#[cfg(target_os = "android")]
mod android;
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
mod platform;
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
mod threads;
mod tiles;
mod traffic;

// the game itself, split by what each part does
mod app;
mod app_events;
mod applog;
mod bus_service;
mod cli;
mod controllers;
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
mod memory;
mod offscreen;
mod on_foot;
mod player;
mod plugins_off;
mod route_arrows;
use lan_off::{admin, lan};

/// The dedicated server's settings: a build without the `server` feature has none, so an
/// `Option<ServerCfg>` is always None.
pub(crate) enum ServerCfg {}
mod services;
mod situation;
mod spawn;
mod startup;
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

use anyhow::Result;
use app::App;
use clap::Parser;
use cli::Args;
use duty_start::place_on_duty;
use input_script::parse_input_script;
use launcher_link::open_launcher;
use offscreen::run_offscreen;
use output::ORBIT_DEFAULT;
use services::run_export;
use situation::apply_situation;
use startup::{
    content_dir, fatal_dialog, find_root, graphics_instance, is_omsi_root, root_memo, BUILD,
    CLASSIC, CLOUDS, ENHANCED, ENHANCED_PLUS, FAIR_CLOUDS, MIRROR_SIZE, SOUND_AI, SOUND_SCENERY,
    VERSION,
};
use std::time::Instant;
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

/// The showroom is drawn the way the game will be.
pub(crate) fn launcher_statics() {
    let s = settings::Settings::load();
    ENHANCED.store(
        startup::enhanced_wanted(&s, false, false).0,
        std::sync::atomic::Ordering::Relaxed,
    );
    CLASSIC.store(s.classic(), std::sync::atomic::Ordering::Relaxed);
    CLOUDS.store(
        s.clouds && !omsi_cfg::flags::NO_CLOUDS.is_set(),
        std::sync::atomic::Ordering::Relaxed,
    );
    FAIR_CLOUDS.store(
        s.fair_weather_clouds(),
        std::sync::atomic::Ordering::Relaxed,
    );
}

/// Everything before a window: the language, the session's random seed, the original
/// installation and the content roots (mods, archives). None when the program has
/// nothing more to do (a fatal error was shown).
pub(crate) fn prepare(mut args: Args, bare: bool) -> Result<Option<(Args, Option<ServerCfg>)>> {
    ui_language(&settings::Settings::load().language);
    // (the launcher and every menu of the game wear the accent chosen in Settings > Interface)
    settings::Settings::load().apply_accent();
    // the dedicated server: server.cfg decides the world, the rest is a host without a window
    if args.server.is_some() {
        eprintln!("server: the dedicated server is no longer built or supported (see docs/removed/MULTIPLAYER_DISABLED.md)");
        std::process::exit(2);
    }
    let server_cfg: Option<ServerCfg> = None;
    // the scripts' `random` differs from session to session (starting air pressure, part
    // lifetimes ...); OMSI_SEED=n repeats a session's numbers
    let seed = omsi_cfg::flags::SEED
        .text()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or_else(|| {
            let t = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(1);
            (t ^ ((std::process::id() as u64) << 32)) % 1_000_000_000
        });
    omsi_script::set_session_seed(seed);
    log::info!("script random seed {seed} (OMSI_SEED={seed} repeats it)");
    let launcher_mode = args.launcher || args.preview_helper || (bare && !args.menu);
    if !is_omsi_root(&args.root) {
        match find_root() {
            Some(p) => {
                log::info!("OMSI 2 found at {}", p.display());
                args.root = p;
            }
            None if launcher_mode => {
                // the launcher opens without the game: its Setup page is where the folder is
                // chosen (only a session needs the original installation)
                log::warn!("the original OMSI 2 was not found; the launcher asks for it");
            }
            None => {
                // openOMSI plays on the original game's content: without a complete
                // installation of it there is nothing to play on, and every player must have
                // the same base whatever copy of OMSI 2 they own
                let missing = omsi_cfg::missing_original_essentials(&args.root);
                let text = format!(
                    "The original OMSI 2 was not found.\n\n\
                     openOMSI needs a complete installation of the original game (any version). \
                     Choose its folder in the launcher (Setup), or start once with \
                     --root \"/path/to/OMSI 2\", the folder with Omsi.exe, maps and Vehicles in it.\n\n\
                     Missing in {}: {}",
                    args.root.display(),
                    missing.join(", ")
                );
                if server_cfg.is_some() {
                    eprintln!("{text}");
                } else {
                    fatal_dialog("Project Depot cannot start", &text);
                }
                if cfg!(target_os = "android") {
                    // (a phone's app is not ended from inside: back to the launcher)
                    return Ok(None);
                }
                std::process::exit(1);
            }
        }
    }
    if let Some(memo) = root_memo().filter(|_| is_omsi_root(&args.root)) {
        let _ = std::fs::write(memo, args.root.to_string_lossy().as_bytes());
    }
    // content roots: the game's own folder (mods) first, then the original installation
    if let Some(c) = content_dir() {
        match omsi_cfg::ensure_content_layout(&c) {
            Ok(()) => {
                omsi_cfg::add_content_root(c.clone());
                log::info!("content folder (mods): {}", c.display());
            }
            Err(e) => log::warn!("content folder {}: {e}", c.display()),
        }
    }
    // archives read in place: searched after the content folder, before the installation
    // (`--content-zip`, `OMSI_CONTENT_ZIP`, and every .zip in the content folder's `Archives`)
    for z in &args.content_zip {
        if let Err(e) = omsi_cfg::add_content_zip(z) {
            log::warn!("content zip {}: {e}", z.display());
        }
    }
    omsi_cfg::vfs::mount_env_zips();
    if let Some(c) = content_dir() {
        omsi_cfg::vfs::mount_dir_zips(&c.join("Archives"));
    }
    omsi_cfg::add_content_root(args.root.clone());
    Ok(Some((args, server_cfg)))
}

/// The game for `args`, ready for its window; None when there is no window to open (a
/// picture or a model was written instead).
pub(crate) fn make_app(mut args: Args, server_cfg: Option<ServerCfg>) -> Result<Option<App>> {
    // OMSI's tutorials: the lesson's own situation
    if let Some(n) = args.tutorial.filter(|n| (1..=4).contains(n)) {
        args.situation = Some(tutorial::SITUATIONS[n - 1].to_string());
    }
    apply_situation(&mut args)?;
    // a duty starts at its trip, as in OMSI (not at the map's entry point); a joining
    // player's once the host's world is known (below): it was never placed at all, and
    // "Automatic" put it at the map's first entry point, the depot
    // real-time sync: the game starts at this device's date and time (a joining player's
    // clock is the host's, a server's is its server.cfg's); a duty does not move it
    if settings::Settings::load().time_sync
        && args.lan_join.is_none()
        && args.server.is_none()
        && args.offscreen.is_none()
    {
        real_time::start_at_now(&mut args);
    }
    if args.export_glb.is_none() && args.lan_join.is_none() {
        place_on_duty(&mut args);
    }
    let settings = settings::Settings::load();
    applog::log_system(&settings);
    if args.drive_keys.eq_ignore_ascii_case("simple")
        && !settings.drive_keys.eq_ignore_ascii_case("simple")
    {
        args.drive_keys = settings.drive_keys.clone();
    }
    let (enhanced, plus) = startup::enhanced_wanted(&settings, args.enhanced, args.enhanced_plus);
    ENHANCED_PLUS.store(plus, std::sync::atomic::Ordering::Relaxed);
    ENHANCED.store(enhanced, std::sync::atomic::Ordering::Relaxed);
    CLOUDS.store(
        settings.clouds && !omsi_cfg::flags::NO_CLOUDS.is_set(),
        std::sync::atomic::Ordering::Relaxed,
    );
    FAIR_CLOUDS.store(
        settings.fair_weather_clouds(),
        std::sync::atomic::Ordering::Relaxed,
    );
    SOUND_AI.store(
        settings.vol_ai.to_bits(),
        std::sync::atomic::Ordering::Relaxed,
    );
    SOUND_SCENERY.store(
        settings.vol_scenery.to_bits(),
        std::sync::atomic::Ordering::Relaxed,
    );
    MIRROR_SIZE.store(settings.mirror_size, std::sync::atomic::Ordering::Relaxed);
    omsi_audio::DOPPLER.store(settings.doppler, std::sync::atomic::Ordering::Relaxed);
    omsi_audio::MUFFLE_OUTSIDE.store(
        settings.muffle_outside,
        std::sync::atomic::Ordering::Relaxed,
    );
    CLASSIC.store(
        settings.classic() && !ENHANCED.load(std::sync::atomic::Ordering::Relaxed),
        std::sync::atomic::Ordering::Relaxed,
    );
    let _ = server_cfg;
    if args.export_glb.is_none() && args.lan_join.is_some() {
        place_on_duty(&mut args);
    }
    if let Some(out) = args.export_glb.clone() {
        return run_export(&args, &out).map(|_| None);
    }
    if let Some(out) = args.offscreen.clone() {
        let r = run_offscreen(&args, &out);
        return r.map(|_| None);
    }
    let view = args.view.clone();
    let args_root_for_keys = args.root.clone();
    let clock_note = args.clock_moved.clone();
    // (as the last session left it, #1164)
    let info_bar = settings.info_bar;
    let mut app = App {
        args,
        instance: crate::startup::game_instance(),
        window: None,
        surface: None,
        renderer: None,
        output: Default::default(),
        #[cfg(windows)]
        vr: None,
        scene: None,
        camera: None,
        player: None,
        placed: Vec::new(),
        chooser: None,
        editor: None,
        vehicle_list: Vec::new(),
        dropdown: None,
        vehicle_meta: std::collections::HashMap::new(),
        world: None,
        streamer: None,
        starting: None,
        traffic: None,
        schedule: None,
        humans: None,
        duty: None,
        duty_places: false,
        hud: None,
        navigator: None,
        vr_nav_profiles: crate::output::Profiles::load(),
        vr_nav_edit: None,
        spanned: false,
        ui: hud::Ui::new(),
        fps: 0.0,
        frame_stats: Default::default(),
        fps_low: None,
        rain: rain::Rain::new(),
        cabin_air: crate::condensation::CabinAir::new(),
        spray: puddles::Spray::new(),
        lamps_on: None,
        menu: None,
        populate_t: 0.0,
        humans_populate_t: 0.0,
        radio: radio::Radio::load(&args_root_for_keys),
        profile: Default::default(),
        profile_prev: Default::default(),
        first_populate: true,
        envir: None,
        weather: None,
        clock: omsi_sim::SimClock::default(),
        started: Instant::now(),
        total_frames: 0,
        mirror_budget: 1.0,
        mirrors_seen: 2,
        mirror_turn: 0,
        frozen_mirrors: None,
        mirror_hud: Default::default(),
        hover_key: None,
        view,
        audio: None,
        ambience: None,
        cursor: (0.0, 0.0),
        vr_cursor_physical: None,
        vr_cursor_warp_pending: None,
        window_focused: false,
        input_away: false,
        window_hidden: false,
        keys: Default::default(),
        door_key_triggers: Default::default(),
        last: Instant::now(),
        speed: 30.0,
        mouse_look: false,
        buttons_held: (false, false),
        mmb_held: false,
        both_drag: None,
        f1_reset: None,
        vr_zoom_active: false,
        hover: None,
        hover_part: None,
        hover_hand: false,
        head_idle_hold: Default::default(),
        input_script: parse_input_script(),
        shot: None,
        paused: false,
        game_menu: None,
        menu_top: None,
        menu_scroll_drag: false,
        dd_scroll_drag: None,
        pane_scroll_drag: None,
        pane_scroll: None,
        clock_hold: 0.0,
        clock_jump: 0.0,
        seat_bus: String::new(),
        pad_look: [false; 4],
        pad_voice_radio: false,
        arrow_glance: false,
        teleport_pick: false,
        #[cfg(steam)]
        steam: None,
        headtrack: None,
        headtrack_failed: None,
        headtrack_scale_last: None,
        headtrack_scale_bias: [0.0; 6],
        headtrack_invert_last: None,
        controllers: None,
        mouse_drive: false,
        mouse_steer: (0.0, 0.0),
        steer_cursor: None,
        center_cursor: false,
        cursor_hidden: None,
        last_ctl_steer: None,
        mouse_pedals: (0.0, 0.0),
        mouse_kmh: 0.0,
        pad_kmh: 0.0,
        pad_steer_target: 0.0,
        tutorial: None,
        ego: false,
        on_foot: None,
        in_cab: false,
        safe_pose: None,
        safe_age: 0.0,
        wheel_acc: 0.0,
        editor_drag: false,
        editor_sync_t: 0.0,
        placing: None,
        admin_list: None,
        list_kind: None,
        route_arrows: Default::default(),
        game_keys: omsi_content::KeyboardCfg::load(&crate::startup::keyboard_cfg(
            &args_root_for_keys,
        ))
        .unwrap_or_default()
        .with_game_defaults()
        .with_vr_defaults()
        .game,
        own_keys: crate::startup::own_keys(&args_root_for_keys),
        own_shift: crate::startup::own_bindings(
            &args_root_for_keys,
            omsi_content::input::KEY_SHIFT,
        ),
        key_capture: None,
        menu_prev_pause: false,
        info_bar,
        pending_time: None,
        world_day: None,
        autosave_t: 0.0,
        new_ui: Default::default(),
        timetable: false,
        dragging: false,
        html_pressed: None,
        html_object_pressed: None,
        drag_delta: (0.0, 0.0),
        look: (0.0, 0.0),
        look_smooth: (0.0, 0.0),
        view_looks: Default::default(),
        look_view: String::new(),
        cam_blend: Default::default(),
        view_zoom: Default::default(),
        orbit: ORBIT_DEFAULT,
        frames: 0,
        fps_t: Instant::now(),
        service_msg: clock_note.map(|m| (m, 10.0)),
        pumping: None,
        // (a server counts its players by their own games, not itself)
        log_state: Default::default(),
        career: Default::default(),
        journey: None,
        wetness: 0.0,
        cloud_drift: [0.0; 2],
        menu_edit: None,
        menu_edit_icao: false,
        swap_pending: false,
        menu_drag: None,
        menu_kbd: true,
        weather_blend: None,
        weather_cycle: None,
        metar_rx: None,
        metar_once: false,
        metar_next: 0.0,
        cursor_kind: 0,
        settings,
        spikes: 0,
        worst_ms: 0.0,
        governor: (0.0, 0, 0.0),
        governor_low: 0,
        governor_wait_prev: 0.0,
        hidden_frames: 0,
        exiting: false,
        stand_in: None,
        cpu_mark: None,
        touch: touch::Touch::new(),
    };
    // mouse steering as the player left it (the wheel eases to the cursor for a second)
    if app.settings.mouse_steering {
        app.mouse_drive = true;
        app.mouse_steer = (0.0, 1.0);
        app.center_cursor = true;
    }
    Ok(Some(app))
}
