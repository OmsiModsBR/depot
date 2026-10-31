//! Keeping the settings the menu changes in the launcher's file, a little later and together.

use crate::core::app::App;

pub(in crate::hud) static PENDING_SETTINGS: std::sync::Mutex<(
    Vec<(String, String)>,
    Option<std::time::Instant>,
)> = std::sync::Mutex::new((Vec::new(), None));

pub(in crate::hud) const SETTINGS_FLUSH_MS: u128 = 250;

pub(in crate::hud) static SETTINGS_CACHE: std::sync::Mutex<Option<serde_json::Value>> =
    std::sync::Mutex::new(None);

/// Write one key of `~/.depot/settings.cfg` (the launcher's file; the other lines
/// stay as they are). The write is delayed a moment and joined with the ones that follow.
/// The key the seat of the player's bus is kept under (`settings::bus_seats`): its `.bus`
/// file, relative to the installation where it lies in it.
pub(crate) fn seat_key(p: &crate::player::Player) -> String {
    // (from its `Vehicles` folder on, so that a copy of the installation finds it)
    let path = p.vehicle.ty.def.path.to_string_lossy().replace('\\', "/");
    match path.to_ascii_lowercase().rfind("/vehicles/") {
        Some(i) => path[i + 1..].to_string(),
        None => path,
    }
}

/// The seat as the menu has it now, kept for the player's bus.
pub(in crate::hud) fn remember_bus_seat(app: &App) {
    if let Some(p) = app.player.as_ref() {
        crate::settings::bus_seats::remember(
            &seat_key(p),
            app.settings.seat,
            app.settings.seat_pitch_deg,
        );
    }
}

pub(crate) fn remember_setting(key: &str, value: &str) {
    {
        let mut p = PENDING_SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
        match p.0.iter_mut().find(|(k, _)| k == key) {
            Some(e) => e.1 = value.to_string(),
            None => p.0.push((key.to_string(), value.to_string())),
        }
    }
    flush_settings(false);
}

/// Write the remembered keys out: all of them when `force`, else only when the last write
/// is `SETTINGS_FLUSH_MS` ago. Called every frame, before the file is read and on exit.
pub(crate) fn flush_settings(force: bool) {
    let pending = {
        let mut p = PENDING_SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
        if p.0.is_empty() {
            return;
        }
        if !force
            && p.1
                .is_some_and(|t| t.elapsed().as_millis() < SETTINGS_FLUSH_MS)
        {
            return;
        }
        p.1 = Some(std::time::Instant::now());
        std::mem::take(&mut p.0)
    };
    let Ok(mut v) = omsi_launcher_lib::get_settings() else {
        return;
    };
    apply_pending(&mut v, &pending);
    if let Err(e) = omsi_launcher_lib::save_settings(&v) {
        log::warn!("settings not saved: {e:#}");
    }
    *SETTINGS_CACHE.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

pub(in crate::hud) fn apply_pending(v: &mut serde_json::Value, pending: &[(String, String)]) {
    for (key, value) in pending {
        let parsed: serde_json::Value = value
            .parse::<f64>()
            .map(serde_json::Value::from)
            .unwrap_or_else(|_| serde_json::Value::from(value.as_str()));
        // a switch goes in as true/false, as the launcher's own values are: written as 1 it
        // was read as not set and saved back as its default (the pause menu's options were
        // lost with the next game)
        let parsed = match (&v[key.as_str()], &parsed) {
            (serde_json::Value::Bool(_), serde_json::Value::Number(n)) => {
                serde_json::Value::Bool(n.as_f64().unwrap_or(0.0) > 0.5)
            }
            _ if key == "time_speed" => serde_json::Value::from(value.as_str()),
            _ => parsed,
        };
        v[key.as_str()] = parsed;
    }
}
