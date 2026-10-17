//! `settings.cfg` between the game and the launcher. The launcher's settings page writes
//! the file and the game reads it, and both read what the other one means: this writes it
//! from one side and reads it from the other, in both directions, one key at a time.
//!
//! The differences found today are listed in [`KNOWN`] and are not fixed here: each one
//! becomes a `fix/` change, which then takes its line out of the list. The test fails both
//! on a new difference and on a listed one that is gone, so the list stays what is true.

use super::Settings;
use omsi_launcher_lib::{settings_from_text, settings_to_text};
use serde_json::Value;
use std::collections::BTreeMap;

/// One value per key the game reads, different from its default and inside the range both
/// sides accept. A key the game reads that is missing here fails [`every_key_is_varied`].
const VARIED: &[(&str, &str)] = &[
    ("msaa", "8"),
    ("anisotropy", "16"),
    ("ssao", "0"),
    ("shadows", "0"),
    ("shadow_size", "4096"),
    ("shadow_blobs", "0"),
    ("navigator", "0"),
    ("ui_opacity", "0.5"),
    ("navigator_corner", "top-right"),
    ("boarding", "pay"),
    ("detail_textures", "0"),
    ("exact_fare", "0"),
    ("enhanced", "1"),
    ("graphics", "enhanced"),
    ("graphics", "enhanced_plus"),
    ("graphics", "vanilla"),
    ("vr", "1"),
    ("vr_scale", "0.8"),
    ("vr_head_smoothing_ms", "12"),
    ("vr_mirror_rate", "30"),
    ("vr_desktop_mirror", "0"),
    ("triple_screen", "1"),
    ("triple_span", "0"),
    ("triple_hud_center", "0"),
    ("triple_fov_deg", "60"),
    ("triple_width_mm", "700"),
    ("triple_distance_mm", "800"),
    ("triple_bezel_mm", "10"),
    ("triple_left_angle_deg", "30"),
    ("triple_right_angle_deg", "35"),
    ("triple_eye_height_mm", "-120"),
    ("fullscreen", "1"),
    ("vsync", "0"),
    ("volume", "0.3"),
    ("render_scale", "0.75"),
    ("language", "DEU"),
    ("pax_voices", "tickets"),
    ("nav_arrows", "1"),
    ("nav_ai", "0"),
    ("nav_surface", "0"),
    ("nav_light", "1"),
    ("nav_contours", "1"),
    ("nav_orientation", "north"),
    ("get_up", "1"),
    ("texture_compression", "0"),
    ("auto_clutch", "0"),
    ("momentary_gears", "1"),
    ("auto_shift", "1"),
    ("min_obj_size", "0.05"),
    ("max_obj_dist", "1500"),
    ("max_fps", "60"),
    ("chat", "0"),
    ("tooltips", "0"),
    ("name_tags", "0"),
    ("driver", "0"),
    ("show_fps", "1"),
    ("clouds", "0"),
    ("windy_trees", "0"),
    ("pax_density", "0.5"),
    ("vol_ai", "0.4"),
    ("vol_scenery", "0.7"),
    ("doppler", "0"),
    ("muffle_outside", "1"),
    ("mirror_hud", "2"),
    ("mirror_size", "512"),
    ("mirror_refresh", "eco"),
    ("texture_memory", "2048"),
    ("maintenance", "3"),
    ("ai_unsched_factor", "150"),
    ("ai_max_scheduled", "40"),
    ("ai_max_humans", "80"),
    ("ai_max_parked", "-1"),
    ("collision_vehicles", "0"),
    ("collision_objects", "0"),
    ("collision_pedestrians", "0"),
    ("head_movement", "0"),
    ("driverview_smooth", "0"),
    ("hands_in_cab", "1"),
    ("alt_view", "0"),
    ("precision_zoom", "1"),
    ("time_speed", "4"),
    ("time_sync", "1"),
    ("metar_sync", "1"),
    ("metar_station", "EDDB"),
    ("machine_translation", "1"),
    ("ctrl_deadzone", "0.1"),
    ("right_stick_look", "0"),
    ("pad_steer_linear", "1"),
    ("arrows_switch_cams", "1"),
    ("pad_steer_smooth", "200"),
    ("reflections", "0"),
    ("led_glow", "10"),
    ("night_brightness", "1.5"),
    ("led_mips", "2.5"),
    ("graphics_api", "vulkan"),
    ("ctrl_off", "Pad 1"),
    ("steering_linear", "1"),
    ("old_steering", "1"),
    ("red_steer_spd", "1"),
    ("ff_invert", "1"),
    ("ff_enabled", "0"),
    ("ff_road_vib", "2"),
    ("ff_engine_vib", "0.5"),
    ("ff_fade", "1"),
    ("brake_hold", "0"),
    ("mouse_steering", "1"),
    ("mouse_right_off", "1"),
    ("mouse_smooth", "0"),
    ("blinker_cancel", "0"),
    ("look_sens", "1.5"),
    ("look_smoothing_ms", "50"),
    ("wheel_range", "540"),
    ("wheel_lock", "450"),
    ("camera_collision", "0"),
    ("steer_look", "1"),
    ("steer_look_angle", "45"),
    ("steer_look_response", "0.5"),
    ("head_idle", "0.5"),
    ("head_idle_pace", "1.5"),
    ("head_tracking", "1"),
    ("head_tracking_invert", "yaw"),
    ("head_tracking_yaw_sens", "50"),
    ("head_tracking_pitch_sens", "50"),
    ("head_tracking_roll_sens", "50"),
    ("head_tracking_x_sens", "50"),
    ("head_tracking_y_sens", "50"),
    ("head_tracking_z_sens", "50"),
    ("head_tracking_invert_yaw", "1"),
    ("head_tracking_invert_pitch", "1"),
    ("head_tracking_invert_roll", "1"),
    ("head_tracking_invert_x", "1"),
    ("head_tracking_invert_y", "1"),
    ("head_tracking_invert_z", "1"),
    ("head_tracking_port", "5000"),
    ("head_tracking_bind", "127.0.0.1"),
    ("voice_chat", "0"),
    ("pedal_throttle", "2"),
    ("pedal_brake", "0.5"),
    ("seat_x", "0.25"),
    ("seat_y", "-0.5"),
    ("seat_z", "0.75"),
    ("seat_pitch_deg", "10"),
    ("fov", "65"),
    ("outside_fov", "70"),
    ("mouse_sens", "2"),
    ("ui_scale", "1.25"),
    ("ui_scale_window", "0"),
    ("chat_size", "1.5"),
    ("notes", "0"),
    ("info_bar", "1"),
    ("autosave_minutes", "10"),
    ("shadow_casters", "omsi"),
    ("post_aa", "off"),
    ("drive_keys", "wasd"),
];

/// Values at and past the ends of the ranges, and ones that are no number, tried on every
/// key of [`VARIED`]: where the two sides clamp or fall back differently.
const EDGES: &[&str] = &[
    "0", "-1", "-1000", "100000", "0.001", "1.5", "abc", "", "NaN",
];

/// The differences between the game and the launcher known today, all of them at the edges
/// ([`EDGES`]): inside the ranges both sides agree on every key, and on the defaults. Per
/// key and direction (`game>launcher`: the game's file, read and saved by the launcher,
/// then read by the game; `launcher>game`: the launcher's file, read by the game and
/// written by it, then read by the launcher), with what goes wrong.
const KNOWN: &[(&str, &str, &str)] = &[
    // the launcher reads these numbers without the range or the fallback the game has (its
    // page then shows a value the game does not use)
    ("ui_opacity", "launcher>game", "no 0.2-1 range, and 0 instead of 0.85 for what is no number"),
    ("min_obj_size", "launcher>game", "no 0-0.2 range"),
    ("max_obj_dist", "launcher>game", "a negative distance is kept (the game reads it as 0, no limit)"),
    ("vol_scenery", "launcher>game", "no 0-1 range"),
    // the launcher keeps these texts as written; the game reads them to its own choices
    ("render_scale", "launcher>game", "kept as text; the game makes 0 or less auto, takes more than 1.5 as a percentage and clamps to 0.5-1"),
    ("pax_voices", "launcher>game", "kept as text; the game reads anything but tickets and off as all"),
    ("drive_keys", "launcher>game", "kept as text; the game keeps simple unless it is wasd, arrows, omsi or simple"),
    ("boarding", "launcher>game", "kept as written; the game lowercases it"),
    // `NaN` is a number to the launcher here; the game falls back to the default
    ("ff_road_vib", "launcher>game", "NaN leaves the key empty"),
    ("pedal_throttle", "launcher>game", "NaN leaves the key empty"),
    ("pedal_brake", "launcher>game", "NaN leaves the key empty"),
    ("seat_y", "launcher>game", "NaN leaves the key empty"),
    ("seat_z", "launcher>game", "NaN leaves the key empty"),
    // and the other way: the game takes NaN, the launcher saves the default over it
    ("volume", "launcher>game", "NaN leaves the key empty"),
    // the launcher keeps whole numbers where the game keeps fractions
    ("vr_head_smoothing_ms", "game>launcher", "the launcher cuts the fraction of a millisecond"),
    ("max_obj_dist", "game>launcher", "the launcher rounds to whole metres (0.001 m becomes 0, no limit)"),
    // the two sides read the same text differently
    ("anisotropy", "game>launcher", "the game takes 0, the launcher saves it as 1"),
    ("head_tracking_z_sens", "game>launcher", "as head_tracking_yaw_sens"),
    ("head_tracking_z_sens", "launcher>game", "as head_tracking_yaw_sens"),
];

/// A difference found: the case (`defaults`, or the key and the value tried), whether the
/// value is one of [`EDGES`], the direction, and the game's field or the launcher's key.
struct Difference {
    key: String,
    value: String,
    edge: bool,
    direction: &'static str,
    what: String,
}

/// The case's file: one key on top of the current version.
fn file(key: &str, value: &str) -> String {
    format!("version={}\n{key}={value}\n", super::SETTINGS_VERSION)
}

/// The game's settings field by field (`triple.fov_deg`, `seat[1]`), as `Debug` shows them.
fn fields(s: &Settings) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    // (the path to the struct or list being read, and the next index in a list)
    let mut path: Vec<(String, usize)> = Vec::new();
    for line in format!("{s:#?}").lines().skip(1) {
        let line = line.trim().trim_end_matches(',');
        let prefix = path
            .iter()
            .map(|(p, _)| p.as_str())
            .collect::<Vec<_>>()
            .join(".");
        let name = |n: &str| {
            if prefix.is_empty() {
                n.to_string()
            } else {
                format!("{prefix}.{n}")
            }
        };
        if line == "}" || line == "]" {
            path.pop();
        } else if let Some((n, v)) = line.split_once(": ") {
            if v.ends_with('{') || v.ends_with('[') {
                path.push((name(n), 0));
            } else {
                out.insert(name(n), v.to_string());
            }
        } else if let Some((p, i)) = path.last_mut() {
            out.insert(format!("{p}[{i}]"), line.to_string());
            *i += 1;
        }
    }
    out
}

/// The fields where `b` differs from `a`.
fn changed(a: &BTreeMap<String, String>, b: &BTreeMap<String, String>) -> Vec<String> {
    a.iter()
        .filter(|(k, v)| b.get(*k) != Some(*v))
        .map(|(k, _)| k.clone())
        .collect()
}

/// The same value on both sides: numbers within the precision the game keeps (f32).
fn same(a: &Value, b: &Value) -> bool {
    let num = |x: &Value| {
        x.as_f64()
            .or_else(|| x.as_str().and_then(|s| s.trim().parse::<f64>().ok()))
    };
    if a == b {
        return true;
    }
    match (num(a), num(b)) {
        (Some(x), Some(y)) => (x - y).abs() <= 1e-6 * x.abs().max(1.0),
        _ => a == b,
    }
}

/// The launcher's keys where `b` differs from `a`.
fn changed_keys(a: &Value, b: &Value) -> Vec<String> {
    let empty = serde_json::Map::new();
    a.as_object()
        .unwrap_or(&empty)
        .iter()
        .filter(|(k, v)| !same(v, b.get(k.as_str()).unwrap_or(&Value::Null)))
        .map(|(k, _)| k.clone())
        .collect()
}

/// The game's file read and saved by the launcher (as its page's Save does, with the old
/// file beside it), then read by the game.
fn through_launcher(s: &Settings) -> Settings {
    let text = s.to_text();
    Settings::from_text(&settings_to_text(
        &settings_from_text(Some(&text)),
        Some(&text),
    ))
}

/// The launcher's view of what it wrote, after the game has read it and written it again.
fn through_game(v: &Value) -> Value {
    settings_from_text(Some(
        &Settings::from_text(&settings_to_text(v, None)).to_text(),
    ))
}

/// Every difference found, on the defaults and on each case of [`VARIED`] and [`EDGES`].
fn differences() -> Vec<Difference> {
    let mut out = Vec::new();
    // the defaults: an empty file on one side, as the other reads it
    let game = Settings::default();
    for f in changed(
        &fields(&game),
        &fields(&Settings::from_text(&settings_to_text(
            &settings_from_text(None),
            None,
        ))),
    ) {
        out.push(Difference {
            key: "defaults".into(),
            value: String::new(),
            edge: false,
            direction: "launcher>game",
            what: f,
        });
    }
    for k in changed_keys(
        &settings_from_text(None),
        &settings_from_text(Some(&game.to_text())),
    ) {
        out.push(Difference {
            key: "defaults".into(),
            value: String::new(),
            edge: false,
            direction: "game>launcher",
            what: k,
        });
    }
    let (game, launcher) = (fields(&Settings::default()), settings_from_text(None));
    let edges = VARIED
        .iter()
        .flat_map(|(k, _)| EDGES.iter().map(move |v| (*k, *v, true)));
    for (key, value, edge) in VARIED.iter().map(|(k, v)| (*k, *v, false)).chain(edges) {
        let found = |direction, what| Difference {
            key: key.to_string(),
            value: value.to_string(),
            edge,
            direction,
            what,
        };
        // what the game takes from the line, kept through the launcher's save
        let s = Settings::from_text(&file(key, value));
        let after = fields(&through_launcher(&s));
        let read = fields(&s);
        for f in changed(&game, &read) {
            if read.get(&f) != after.get(&f) {
                out.push(found("game>launcher", f));
            }
        }
        // what the launcher takes from the line, kept through the game
        let v = settings_from_text(Some(&file(key, value)));
        let back = through_game(&v);
        for k in changed_keys(&launcher, &v) {
            if !same(&v[&k], back.get(&k).unwrap_or(&Value::Null)) {
                out.push(found("launcher>game", k));
            }
        }
    }
    out
}

/// The game's own file reads back as written: every field is in it.
#[test]
fn the_game_reads_back_every_field_it_writes() {
    let mut cases = vec![Settings::default()];
    cases.extend(VARIED.iter().map(|(k, v)| Settings::from_text(&file(k, v))));
    for s in cases {
        let back = Settings::from_text(&s.to_text());
        assert_eq!(
            changed(&fields(&s), &fields(&back)),
            Vec::<String>::new(),
            "fields lost in the game's own file"
        );
    }
}

/// Each key the game reads has a value in [`VARIED`] that changes something.
#[test]
fn every_key_is_varied() {
    let game = fields(&Settings::default());
    for (key, value) in VARIED {
        assert!(
            !changed(&game, &fields(&Settings::from_text(&file(key, value)))).is_empty(),
            "{key}={value} changes nothing the game reads"
        );
    }
    // every field is reached by some key
    let mut reached = std::collections::BTreeSet::new();
    for (key, value) in VARIED {
        reached.extend(changed(
            &game,
            &fields(&Settings::from_text(&file(key, value))),
        ));
    }
    let missed: Vec<_> = game.keys().filter(|f| !reached.contains(*f)).collect();
    assert!(
        missed.is_empty(),
        "fields no key of VARIED changes: {missed:?}"
    );
}

/// The game and the launcher agree on the file, but for the differences known today.
#[test]
fn the_game_and_the_launcher_read_each_others_file() {
    let found = differences();
    let known = |d: &Difference| {
        d.edge
            && KNOWN
                .iter()
                .any(|(k, dir, _)| *k == d.key && *dir == d.direction)
    };
    let new: Vec<String> = found
        .iter()
        .filter(|d| !known(d))
        .map(|d| format!("{}={:?} {}: {}", d.key, d.value, d.direction, d.what))
        .collect();
    let gone: Vec<String> = KNOWN
        .iter()
        .filter(|(k, dir, _)| {
            !found
                .iter()
                .any(|d| d.edge && d.key == *k && d.direction == *dir)
        })
        .map(|(k, dir, _)| format!("{k} {dir}"))
        .collect();
    assert!(new.is_empty() && gone.is_empty(), "settings.cfg differs between the game and the launcher.\nnew differences (fix them, or add the key to KNOWN when only an edge value differs):\n{}\nno longer differ (take them out of KNOWN):\n{}", new.join("\n"), gone.join("\n"));
}
