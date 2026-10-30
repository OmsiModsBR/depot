//! The game made ready for its window: what is set up before one (the language, the
//! session's seed, the installation and the content roots), the launcher's statics and `App`
//! itself.

use super::super::domain::app::App;
use super::super::domain::cli::Args;
use super::super::interface::output::Settings;
use super::applog;
use super::startup::{
    self, content_dir, fatal_dialog, find_root, is_omsi_root, root_memo, CLASSIC, CLOUDS, ENHANCED,
    ENHANCED_PLUS, FAIR_CLOUDS, MIRROR_SIZE, SOUND_AI, SOUND_SCENERY,
};
use crate::duty_start::place_on_duty;
use crate::input_script::parse_input_script;
use crate::offscreen::run_offscreen;
use crate::output::ORBIT_DEFAULT;
use crate::services::run_export;
use crate::situation::apply_situation;
use crate::ui_language;
use crate::{hud, puddles, radio, rain, real_time, touch, tutorial};
use anyhow::Result;
use std::time::Instant;

/// The dedicated server's settings: a build without the `server` feature has none, so an
/// `Option<ServerCfg>` is always None.
pub(crate) enum ServerCfg {}

/// The showroom is drawn the way the game will be.
pub(crate) fn launcher_statics() {
    let s = Settings::load();
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
    ui_language(&Settings::load().language);
    // (the launcher and every menu of the game wear the accent chosen in Settings > Interface)
    Settings::load().apply_accent();
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
    if Settings::load().time_sync
        && args.lan_join.is_none()
        && args.server.is_none()
        && args.offscreen.is_none()
    {
        real_time::start_at_now(&mut args);
    }
    if args.export_glb.is_none() && args.lan_join.is_none() {
        place_on_duty(&mut args);
    }
    let settings = Settings::load();
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
    let info_collapsed = settings.info_bar_collapsed;
    let mut app = App {
        args,
        instance: crate::core::startup::game_instance(),
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
        hover_split: Default::default(),
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
        game_keys: omsi_content::KeyboardCfg::load(&crate::core::startup::keyboard_cfg(
            &args_root_for_keys,
        ))
        .unwrap_or_default()
        .with_game_defaults()
        .with_vr_defaults()
        .game,
        own_keys: crate::core::startup::own_keys(&args_root_for_keys),
        own_shift: crate::core::startup::own_bindings(
            &args_root_for_keys,
            omsi_content::input::KEY_SHIFT,
        ),
        key_capture: None,
        menu_prev_pause: false,
        info_bar,
        info_collapsed,
        info_chev_hover: 0.0,
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
