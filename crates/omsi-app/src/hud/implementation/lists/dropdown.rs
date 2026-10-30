//! The drop-downs over a settings row, with the weather's.

use super::chooser::LIST_DIRTY;
use super::select::{
    preset_now, presets, reload_settings, select_state, settings_file, store_with, sync_live,
};
use super::store::remember_setting;
use super::values::{cloud_index, set_precip};
use super::vehicles::bus_cmp;
use crate::core::app::App;
use crate::hud::domain::lists::{Dropdown, CLOUD_TYPES, CUSTOM_WEATHER, PRECIP_KINDS};

/// The name of the weather in force (the file's name without its ending).
/// The weather files (`Weather/*.owt`) of every content root, by name.
pub(crate) fn weather_files() -> Vec<String> {
    let mut files: Vec<String> = omsi_cfg::read_dir_merged("Weather")
        .into_iter()
        .filter(|p| {
            p.extension()
                .map(|e| e.eq_ignore_ascii_case("owt"))
                .unwrap_or(false)
        })
        .filter_map(|p| {
            p.file_name()
                .map(|n| format!("Weather/{}", n.to_string_lossy()))
        })
        .collect();
    files.sort_by(|a, b| {
        bus_cmp(
            a.trim_start_matches("Weather/").trim_start_matches('#'),
            b.trim_start_matches("Weather/").trim_start_matches('#'),
        )
    });
    files.dedup();
    files
}

/// The drop-down of the row `row` whose action is `id`, if that row has one.
pub(crate) fn dropdown_for(app: &App, row: usize, id: &str) -> Option<Dropdown> {
    let tr = |t: &str| omsi_ui::tr(t).into_owned();
    if app.metar_locked() && matches!(id, "weather" | "cloudkind" | "precipkind") {
        return None;
    }
    let mut current: Option<usize> = None;
    let items: Vec<(String, String)> = match id {
        "weather" => {
            let now = weather_name(app);
            weather_files()
                .into_iter()
                .enumerate()
                .map(|(i, f)| {
                    let stem = f
                        .rsplit('/')
                        .next()
                        .unwrap_or(&f)
                        .rsplit_once('.')
                        .map(|x| x.0)
                        .unwrap_or(&f)
                        .trim_start_matches('#')
                        .to_string();
                    if stem.eq_ignore_ascii_case(&now) {
                        current = Some(i);
                    }
                    (stem, format!("wx {f}"))
                })
                .collect()
        }
        "metar_src" => {
            let mut v = vec![(tr("Automatic (nearest the map)"), "metar_src ".to_string())];
            let own = &app.settings.metar_station;
            current = Some(0);
            for (i, (code, label)) in crate::weather_setup::metar_airports(&app.args.root)
                .into_iter()
                .enumerate()
            {
                if code.eq_ignore_ascii_case(own) {
                    current = Some(i + 1);
                }
                v.push((label, format!("metar_src {code}")));
            }
            v
        }
        "cloudkind" => {
            current = app.weather.as_ref().and_then(|w| cloud_index(&w.clouds.0));
            CLOUD_TYPES
                .iter()
                .enumerate()
                .map(|(i, (_, n))| (tr(*n), format!("cloud {i}")))
                .collect()
        }
        "precipkind" => {
            current = app.weather.as_ref().map(|w| {
                (w.precip.first().copied().unwrap_or(0.0).max(0.0) as usize)
                    .min(PRECIP_KINDS.len() - 1)
            });
            PRECIP_KINDS
                .iter()
                .enumerate()
                .map(|(i, n)| (tr(*n), format!("precip {i}")))
                .collect()
        }
        "preset" => {
            current = preset_now(&settings_file());
            presets()
                .iter()
                .enumerate()
                .map(|(i, p)| (tr(p.0), format!("preset {i}")))
                .collect()
        }
        "gfxprofile" => omsi_launcher_lib::graphics_profiles()
            .into_keys()
            .map(|n| (n.clone(), format!("gfxprofile {n}")))
            .collect(),
        "reset" => vec![
            (tr("Cancel"), "noop".to_string()),
            (tr("Reset all settings"), "reset_all".to_string()),
        ],
        key if key.starts_with("sel ") => {
            let key = &key[4..];
            let (options, at, _) = select_state(&settings_file(), key);
            current = at;
            options
                .iter()
                .map(|o| (tr(o.1), format!("pick {key} {}", o.0)))
                .collect()
        }
        _ => return None,
    };
    if items.is_empty() {
        return None;
    }
    let sel = current.unwrap_or(0);
    Some(Dropdown {
        row,
        items,
        sel,
        top: 0,
        current,
    })
}

/// Do an entry of a drop-down.
pub(crate) fn dropdown_apply(app: &mut App, action: &str) {
    let (verb, arg) = action.split_once(' ').unwrap_or((action, ""));
    if app.metar_locked() && matches!(verb, "wx" | "cloud" | "precip") {
        app.service_msg = Some((
            "The weather cannot be changed while the METAR sync is on".into(),
            3.0,
        ));
        return;
    }
    match verb {
        "wx" => {
            if app.is_lan_client() {
                app.service_msg = Some(("In a LAN session the host sets the weather".into(), 3.0));
            } else {
                app.change_weather(Some(arg.to_string()), true, 1.0);
            }
        }
        "metar_src" => {
            let code: String = arg
                .trim()
                .chars()
                .filter(|c| c.is_ascii_alphabetic())
                .take(4)
                .collect::<String>()
                .to_ascii_uppercase();
            app.settings.metar_station = code.clone();
            remember_setting("metar_station", &code);
            app.metar_rx = None;
            app.metar_once = false;
            // With sync on, the new station is fetched at once. With it off this simply
            // selects the station for "Load current METAR once".
            app.metar_next = 0.0;
        }
        "cloud" => {
            if let Some(i) = arg
                .trim()
                .parse::<usize>()
                .ok()
                .filter(|i| *i < CLOUD_TYPES.len())
            {
                app.edit_weather(|w| {
                    w.clouds.0 = CLOUD_TYPES[i].0.to_string();
                    if i == 0 {
                        w.clouds.1 = 0.0;
                    }
                });
            }
        }
        "precip" => {
            if let Some(i) = arg
                .trim()
                .parse::<usize>()
                .ok()
                .filter(|i| *i < PRECIP_KINDS.len())
            {
                set_precip(app, i);
            }
        }
        "pick" => {
            if let Some((key, value)) = arg.split_once(' ') {
                remember_setting(key, value);
                reload_settings(app);
            }
        }
        "preset" => {
            if let Some(p) = arg
                .trim()
                .parse::<usize>()
                .ok()
                .and_then(|i| presets().into_iter().nth(i))
            {
                store_with(app, |v| {
                    if let Some(o) = p.1.as_object() {
                        for (k, x) in o {
                            v[k.as_str()] = x.clone();
                        }
                    }
                });
            }
        }
        "gfxprofile" => {
            let name = arg.trim();
            match omsi_launcher_lib::graphics_profiles().get(name) {
                Some(p) => {
                    store_with(app, |v| omsi_launcher_lib::apply_graphics_profile(p, v));
                    sync_live(app);
                    LIST_DIRTY.store(true, std::sync::atomic::Ordering::Relaxed);
                    app.service_msg = Some((format!("Graphics profile \"{name}\" loaded: graphics settings apply when the game starts the next time"), 5.0));
                }
                None => {
                    app.service_msg = Some((format!("Graphics profile \"{name}\" not found"), 4.0))
                }
            }
        }
        "reset_all" => store_with(app, |v| {
            let language = v.get("language").cloned();
            *v = omsi_launcher_lib::settings_from_text(None);
            if let Some(l) = language {
                v["language"] = l;
            }
        }),
        _ => {}
    }
}

pub(crate) fn weather_name(app: &App) -> String {
    if app
        .weather
        .as_ref()
        .is_some_and(|w| w.name == CUSTOM_WEATHER)
    {
        return CUSTOM_WEATHER.to_string();
    }
    // a METAR report's weather
    if app
        .args
        .weather
        .as_deref()
        .is_some_and(|p| p.starts_with(crate::weather_setup::REPORT))
    {
        if let Some(n) = app
            .weather
            .as_ref()
            .map(|w| w.name.trim().to_string())
            .filter(|n| !n.is_empty())
        {
            return n;
        }
    }
    match app.args.weather.as_deref() {
        Some(p) => {
            let p = p.replace('\\', "/");
            let file = p.rsplit('/').next().unwrap_or("");
            let stem = file.rsplit_once('.').map(|x| x.0).unwrap_or(file);
            stem.trim_start_matches('#').to_string()
        }
        None => app
            .weather
            .as_ref()
            .map(|w| w.name.trim().to_string())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "Map default".to_string()),
    }
}
