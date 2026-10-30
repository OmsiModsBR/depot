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
        "graphics" => vec![
            ("vanilla", "Vanilla (as OMSI 2)"),
            ("vanilla_plus", "Vanilla+"),
            ("enhanced", "Enhanced"),
            ("enhanced_plus", "Enhanced+"),
        ],
        "msaa" => vec![
            ("1", "Off"),
            ("2", "2x MSAA"),
            ("4", "4x MSAA"),
            ("8", "8x MSAA"),
        ],
        "render_scale" => vec![
            ("auto", "Auto"),
            ("1", "100%"),
            ("0.85", "85%"),
            ("0.75", "75%"),
            ("0.67", "67%"),
            ("0.5", "50%"),
        ],
        "anisotropy" => vec![
            ("1", "Off"),
            ("2", "2x"),
            ("4", "4x"),
            ("8", "8x"),
            ("16", "16x"),
        ],
        "shadow_size" => vec![("1024", "1024"), ("2048", "2048"), ("4096", "4096")],
        "shadow_casters" => vec![
            ("all", "Every solid mesh"),
            ("omsi", "[shadow] meshes, as OMSI"),
        ],
        "max_fps" => vec![
            ("0", "Screen refresh rate"),
            ("30", "30 fps"),
            ("45", "45 fps"),
            ("60", "60 fps"),
            ("120", "120 fps"),
            ("144", "144 fps"),
            ("1000", "Unlimited"),
        ],
        "view_distance" => vec![
            ("auto", "Default (1200 m)"),
            ("600", "600 m - fastest"),
            ("900", "900 m"),
            ("1200", "1200 m"),
            ("1500", "1500 m"),
            ("2000", "2000 m"),
            ("2500", "2500 m"),
        ],
        "max_obj_dist" => vec![
            ("auto", "Automatic"),
            ("500", "500 m"),
            ("750", "750 m"),
            ("900", "900 m"),
            ("1500", "1500 m"),
            ("3000", "3000 m"),
        ],
        "min_obj_size" => vec![
            ("0.005", "All"),
            ("0.013", "Normal"),
            ("0.02", "Fewer (faster)"),
            ("0.03", "Few (fastest)"),
        ],
        "mirror_size" => vec![
            ("0", "Off"),
            ("128", "Low (128)"),
            ("256", "Normal (256)"),
            ("512", "High (512)"),
            ("1024", "Very high (1024)"),
        ],
        "texture_memory" => vec![
            ("0", "Automatic"),
            ("500", "500 MB"),
            ("1000", "1 GB"),
            ("1500", "1.5 GB"),
            ("2000", "2 GB"),
            ("3000", "3 GB"),
            ("4000", "4 GB"),
            ("6000", "6 GB"),
        ],
        "drive_keys" => vec![
            ("omsi", "Custom controls (Controls page)"),
            ("simple", "W A S D + arrows"),
            ("wasd", "W A S D only"),
            ("arrows", "Arrow keys only"),
        ],
        "resolution" => crate::launcher::settings::RESOLUTIONS.to_vec(),
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
        "vr_scale" => vec![
            ("0.5", "50%"),
            ("0.65", "65%"),
            ("0.8", "80%"),
            ("1", "100%"),
        ],
        "vr_head_smoothing_ms" => vec![
            ("0", "Off"),
            ("5", "5 ms"),
            ("10", "10 ms"),
            ("20", "20 ms"),
            ("30", "30 ms"),
        ],
        "vr_mirror_rate" => vec![
            ("0", "Off"),
            ("8", "8/s"),
            ("16", "16/s"),
            ("24", "24/s"),
            ("32", "32/s"),
            ("48", "48/s"),
            ("60", "60/s"),
            ("90", "90/s"),
            ("120", "120/s"),
            ("180", "180/s"),
            ("240", "240/s"),
            ("360", "360/s"),
            ("-1", "Every frame"),
        ],
        _ => Vec::new(),
    }
}

pub fn select_state(
    file: &serde_json::Value,
    key: &str,
) -> (Vec<(&'static str, &'static str)>, Option<usize>, String) {
    let mut options = select_options(key);
    // (under DirectX9 an option may leave some choices out, see `api_limits`)
    if crate::launcher::settings::d3d9_fixes_vanilla(file) {
        if let Some(l) = crate::api_limits::d3d9(key) {
            options.retain(|o| l.allows(o.0));
        }
    }
    let mut cur = value_text(file.get(key).unwrap_or(&serde_json::Value::Null));
    // anisotropy 0 is off, the same entry as 1
    if key == "anisotropy" && cur == "0" {
        cur = "1".to_string();
    }
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

pub(in crate::hud) fn presets() -> [(&'static str, serde_json::Value); 4] {
    [
        (
            "Low",
            serde_json::json!({"msaa": 1, "anisotropy": 2, "shadow_size": 1024, "ssao": false, "shadows": false, "detail_textures": false, "clouds": false, "view_distance": "600", "min_obj_size": 0.03, "max_obj_dist": "500", "mirror_size": 128, "render_scale": "0.75", "texture_memory": 800}),
        ),
        (
            "Medium",
            serde_json::json!({"msaa": 2, "anisotropy": 4, "shadow_size": 2048, "ssao": false, "shadows": true, "detail_textures": true, "clouds": true, "view_distance": "900", "min_obj_size": 0.02, "max_obj_dist": "750", "mirror_size": 256, "render_scale": "auto", "texture_memory": 1200}),
        ),
        (
            "High",
            serde_json::json!({"msaa": 4, "anisotropy": 8, "shadow_size": 2048, "ssao": true, "shadows": true, "detail_textures": true, "clouds": true, "view_distance": "auto", "min_obj_size": 0.013, "max_obj_dist": "auto", "mirror_size": 256, "render_scale": "auto", "texture_memory": 0}),
        ),
        (
            "Ultra",
            serde_json::json!({"msaa": 4, "anisotropy": 8, "shadow_size": 4096, "ssao": true, "shadows": true, "detail_textures": true, "clouds": true, "view_distance": "2000", "min_obj_size": 0.005, "max_obj_dist": "1500", "mirror_size": 512, "render_scale": "auto", "texture_memory": 0}),
        ),
    ]
}

pub(in crate::hud) fn preset_now(file: &serde_json::Value) -> Option<usize> {
    presets().iter().position(|p| {
        p.1.as_object().is_some_and(|o| {
            o.iter().all(|(k, v)| {
                same_value(
                    &value_text(v),
                    &value_text(file.get(k).unwrap_or(&serde_json::Value::Null)),
                )
            })
        })
    })
}

pub(in crate::hud) fn preset_row(
    file: &serde_json::Value,
    name: &str,
    desc: &str,
) -> Option<(String, String)> {
    let label =
        omsi_ui::tr(preset_now(file).map(|i| presets()[i].0).unwrap_or("Custom")).into_owned();
    Some((row(name, 'o', &label, desc, None), "preset".to_string()))
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
    if let Some(h) = app.humans.as_mut() {
        h.exact_fare = s.exact_fare;
        h.boarding = s.boarding.clone();
        h.voices = match s.pax_voices.as_str() {
            "off" => 2,
            "tickets" => 1,
            _ => 0,
        };
    }
}
