//! The launcher's settings file as the windows show it: its choices, presets and keeping it in step with the game.

use super::rows::row;
use super::store::{apply_pending, flush_settings, PENDING_SETTINGS, SETTINGS_CACHE};
use crate::core::app::App;

/// The launcher's settings file as the lists show it: read once (until something is
/// written), with the keys still waiting to be written on top.
pub(in crate::hud) fn settings_file() -> serde_json::Value {
    let mut v = SETTINGS_CACHE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_or_insert_with(|| {
            let text =
                std::fs::read_to_string(omsi_launcher_lib::data_dir().join("settings.cfg")).ok();
            omsi_launcher_lib::settings_from_text(text.as_deref())
        })
        .clone();
    let pending = PENDING_SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
    apply_pending(&mut v, &pending.0);
    v
}

pub(in crate::hud) fn value_text(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(x) => x.clone(),
        serde_json::Value::Bool(b) => (*b as u8).to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

pub(in crate::hud) fn same_value(a: &str, b: &str) -> bool {
    a == b
        || a.parse::<f64>()
            .ok()
            .zip(b.parse::<f64>().ok())
            .is_some_and(|(x, y)| (x - y).abs() < 1e-6)
}

pub(in crate::hud) fn select_options(key: &str) -> Vec<(&'static str, &'static str)> {
    match key {
        "max_fps" => vec![
            ("0", "Screen refresh rate"),
            ("30", "30 fps"),
            ("45", "45 fps"),
            ("60", "60 fps"),
            ("120", "120 fps"),
            ("144", "144 fps"),
            ("1000", "Unlimited"),
        ],
        "drive_keys" => vec![
            ("omsi", "Custom controls (Controls page)"),
            ("simple", "W A S D + arrows"),
            ("wasd", "W A S D only"),
            ("arrows", "Arrow keys only"),
        ],
        "resolution" => omsi_launcher_lib::settings::RESOLUTIONS.to_vec(),
        "navigator_corner" => vec![
            ("top-left", "Top left"),
            ("top-right", "Top right"),
            ("bottom-left", "Bottom left"),
            ("bottom-right", "Bottom right"),
        ],
        "info_bar_mode" => vec![
            ("full", "Full bar"),
            ("compact", "Compact bar"),
            ("essential", "Essentials only"),
            ("custom", "Custom bar"),
            ("twotier", "Two-tier bar"),
            ("split", "Split bar"),
        ],
        "info_bar_pos" => vec![
            ("top", "Top of the screen"),
            ("bottom", "Bottom of the screen"),
        ],
        "nav_view" => vec![("perspective", "Perspective"), ("top", "Top view")],
        "nav_orientation" => vec![
            ("map", "As the map is drawn"),
            ("north", "North up"),
            ("heading", "Heading up"),
        ],
        "boarding" => vec![
            ("auto", "Pay and take the ticket"),
            ("pay", "The driver sells the ticket"),
            ("walk", "Just walk in"),
        ],
        "pax_voices" => vec![
            ("all", "Greetings and tickets"),
            ("tickets", "Only the ticket asked for"),
            ("off", "Silent"),
        ],
        "maintenance" => vec![
            ("0", "Infinite (no wear)"),
            ("1", "Very bad"),
            ("2", "Bad"),
            ("3", "Normal"),
            ("4", "Good"),
        ],
        "ai_unsched_factor" => vec![
            ("25", "25%"),
            ("50", "50%"),
            ("75", "75%"),
            ("100", "100%"),
            ("150", "150%"),
            ("200", "200%"),
            ("300", "300%"),
        ],
        "ai_max_scheduled" | "ai_max_random" => vec![
            ("0", "None"),
            ("10", "At most 10"),
            ("25", "At most 25"),
            ("50", "At most 50"),
            ("100", "At most 100"),
            ("250", "At most 250"),
            ("500", "At most 500"),
            ("1000", "At most 1000"),
        ],
        "ai_priority_scheduled" => vec![
            ("1", "1: only the most important lines"),
            ("2", "2"),
            ("3", "3"),
            ("4", "4: all lines"),
        ],
        "ai_max_parked" => vec![
            ("0", "None"),
            ("25", "25%"),
            ("50", "50%"),
            ("75", "75%"),
            ("100", "100%"),
        ],
        "autosave_minutes" => vec![
            ("0", "Off"),
            ("2", "Every 2 minutes"),
            ("5", "Every 5 minutes"),
            ("10", "Every 10 minutes"),
            ("15", "Every 15 minutes"),
        ],
        "ai_max_humans" => vec![
            ("0", "None"),
            ("50", "At most 50"),
            ("100", "At most 100"),
            ("200", "At most 200"),
            ("400", "At most 400"),
            ("800", "At most 800"),
            ("1000", "At most 1000"),
        ],
        "language" => omsi_launcher_lib::LANGUAGES
            .iter()
            .map(|l| (l.0, l.1))
            .collect(),
        // (anything else: the launcher's own choices for the option)
        _ => omsi_launcher_ui::window::OPTS
            .iter()
            .find(|o| o.key == key)
            .and_then(|o| match o.kind {
                omsi_launcher_ui::window::Kind::Pick(list) => Some(list.to_vec()),
                _ => None,
            })
            .unwrap_or_default(),
    }
}

/// The launcher's graphics options (its Graphics and Advanced graphics pages, in its order)
/// the game's menu offers: the ones the running mode offers whose change takes effect at once
/// (`omsi_cfg::graphics_options::offered_in_game`), less a row that waits for another (the
/// upscaling for a scale below 100 %, the people in the rain reflections for the reflections).
/// The mode is the one the game runs in, not the one saved since.
pub fn ingame_graphics_options(
    s: &crate::settings::Settings,
) -> Vec<&'static omsi_launcher_ui::window::Opt> {
    use omsi_launcher_ui::window::{OPTS, SECTIONS};
    OPTS.iter()
        .filter(|o| matches!(SECTIONS[o.sec], "Graphics" | "Advanced graphics"))
        .filter(|o| omsi_cfg::graphics_options::offered_in_game(o.key, s.graphics))
        .filter(|o| match o.key {
            "dx9_upscale" => s.render_scale > 0.0 && s.render_scale < 0.995,
            "humans_in_rain_reflections" => s.rain_reflections,
            _ => true,
        })
        .collect()
}

pub fn select_state(
    file: &serde_json::Value,
    key: &str,
) -> (Vec<(&'static str, &'static str)>, Option<usize>, String) {
    let mut options = select_options(key);
    // (a graphics mode may leave some choices out, `omsi_cfg::graphics_options`)
    let mode = omsi_cfg::GraphicsMode::parse(
        file.get("graphics")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
    );
    options.retain(|o| omsi_cfg::graphics_options::allows(key, mode, o.0));
    let cur = value_text(file.get(key).unwrap_or(&serde_json::Value::Null));
    let at = options.iter().position(|o| same_value(o.0, &cur));
    (options, at, cur)
}

pub(in crate::hud) fn select_row(
    file: &serde_json::Value,
    key: &str,
    name: &str,
    desc: &str,
) -> Option<(String, String)> {
    let (options, at, cur) = select_state(file, key);
    if options.is_empty() {
        return None;
    }
    let label = at
        .map(|i| omsi_ui::tr(options[i].1).into_owned())
        .unwrap_or(cur);
    Some((row(name, 'o', &label, desc, None), format!("sel {key}")))
}

pub(in crate::hud) fn store_with(app: &mut App, change: impl FnOnce(&mut serde_json::Value)) {
    flush_settings(true);
    let Ok(mut v) = omsi_launcher_lib::get_settings() else {
        return;
    };
    change(&mut v);
    *SETTINGS_CACHE.lock().unwrap_or_else(|e| e.into_inner()) = None;
    match omsi_launcher_lib::save_settings(&v) {
        Ok(()) => reload_settings(app),
        Err(e) => log::warn!("settings not saved: {e:#}"),
    }
}

pub(in crate::hud) fn reload_settings(app: &mut App) {
    flush_settings(true);
    // (the seat is the bus's, see `bus_seats`: it is read again for the bus)
    app.settings = crate::settings::Settings::load();
    app.seat_bus.clear();
    crate::ui_language(&app.settings.language);
    sync_live(app);
}

pub(in crate::hud) fn sync_live(app: &mut App) {
    let s = &app.settings;
    crate::core::startup::SOUND_AI.store(s.vol_ai.to_bits(), std::sync::atomic::Ordering::Relaxed);
    crate::core::startup::SOUND_SCENERY.store(
        s.vol_scenery.to_bits(),
        std::sync::atomic::Ordering::Relaxed,
    );
    omsi_audio::DOPPLER.store(s.doppler, std::sync::atomic::Ordering::Relaxed);
    omsi_audio::MUFFLE_OUTSIDE.store(s.muffle_outside, std::sync::atomic::Ordering::Relaxed);
    if let Some(n) = app.navigator.as_mut() {
        n.arrows = s.nav_arrows;
        n.view_top = s.nav_view == "top";
    }
    if let Some(h) = app.session.humans.as_mut() {
        h.exact_fare = s.exact_fare;
        h.boarding = s.boarding.clone();
        h.voices = match s.pax_voices.as_str() {
            "off" => 2,
            "tickets" => 1,
            _ => 0,
        };
    }
}
