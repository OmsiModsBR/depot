use glam::Vec2;
use omsi_launcher_lib as core;
use omsi_ui::Rect;
use openomsi_game::testing::launcher::settings::*;
use openomsi_game::testing::launcher::state::Choice;
use openomsi_game::testing::launcher::ui::id_of;
use openomsi_game::testing::launcher::ui::Ui;
use serde_json::json;
use serde_json::Value;

/// Every option the page had before its sections (by its key): nothing may go missing.
const BEFORE: &[&str] = &[
    "preset",
    "graphics",
    "msaa",
    "render_scale",
    "anisotropy",
    "shadow_size",
    "ssao",
    "shadows",
    "shadow_casters",
    "detail_textures",
    "night_brightness",
    "led_glow",
    "led_mips",
    "shadow_blobs",
    "reflections",
    "clouds",
    "sun_glow",
    "windy_trees",
    "fullscreen",
    "resolution",
    "vsync",
    "max_fps",
    "graphics_api",
    "view_distance",
    "max_obj_dist",
    "min_obj_size",
    "mirror_size",
    "mirror_refresh",
    "texture_memory",
    "texture_compression",
    "drive_keys",
    "steering_linear",
    "old_steering",
    "red_steer_spd",
    "mouse_sens",
    "mouse_smooth",
    "mouse_right_off",
    "blinker_cancel",
    "brake_hold",
    "auto_clutch",
    "auto_shift",
    "momentary_gears",
    "wheel_range",
    "wheel_lock",
    "pad_steer_smooth",
    "pad_steer_linear",
    "arrows_switch_cams",
    "pedal_throttle",
    "pedal_brake",
    "ff_enabled",
    "ff_invert",
    "ff_road_vib",
    "ff_engine_vib",
    "ff_fade",
    "seat_y",
    "seat_z",
    "seat_x",
    "seat_pitch_deg",
    "fov",
    "triple_fov_deg",
    "outside_fov",
    "look_sens",
    "right_stick_look",
    "look_smoothing_ms",
    "head_idle",
    "head_idle_pace",
    "steer_look",
    "steer_look_angle",
    "steer_look_response",
    "head_movement",
    "driverview_smooth",
    "hands_in_cab",
    "alt_view",
    "precision_zoom",
    "camera_collision",
    "driver",
    "head_tracking",
    "head_tracking_yaw_sens",
    "head_tracking_pitch_sens",
    "head_tracking_roll_sens",
    "head_tracking_invert_yaw",
    "head_tracking_invert_pitch",
    "head_tracking_invert_roll",
    "head_tracking_x_sens",
    "head_tracking_y_sens",
    "head_tracking_z_sens",
    "head_tracking_invert_x",
    "head_tracking_invert_y",
    "head_tracking_invert_z",
    "triple_screen",
    "triple_span",
    "triple_hud_center",
    "triple_width_mm",
    "triple_distance_mm",
    "triple_bezel_mm",
    "triple_left_angle_deg",
    "triple_right_angle_deg",
    "triple_eye_height_mm",
    "vr",
    "vr_scale",
    "vr_head_smoothing_ms",
    "vr_mirror_rate",
    "vr_desktop_mirror",
    "volume",
    "vol_ai",
    "vol_scenery",
    "doppler",
    "muffle_outside",
    "pax_voices",
    "boarding",
    "exact_fare",
    "pax_density",
    "get_up",
    "ai_unsched_factor",
    "ai_max_scheduled",
    "ai_max_random",
    "ai_priority_scheduled",
    "ai_max_parked",
    "autosave_minutes",
    "ai_max_humans",
    "maintenance",
    "collision_vehicles",
    "collision_objects",
    "collision_pedestrians",
    "use_real_time",
    "use_real_date",
    "time_sync",
    "metar_sync",
    "time_speed",
    "language",
    "launcher_rest",
    "ui_scale",
    "ui_scale_window",
    "ui_opacity",
    "tooltips",
    "show_fps",
    "info_bar",
    "info_bar_mode",
    "info_bar_pos",
    "info_bar_fps",
    "info_item_time",
    "info_item_speed",
    "info_item_trip",
    "info_item_odometer",
    "info_item_temps",
    "info_item_tank",
    "info_item_pax",
    "info_item_line",
    "info_item_next",
    "notes",
    "navigator",
    "nav_arrows",
    "nav_ai",
    "nav_follow_view",
    "navigator_corner",
    // (the Interface section, drawn on its own before)
    "ui_accent",
    "ui_nav",
    "ui_background",
    "ui_dimming",
    "ui_corners",
    // (the Maps page's temporary window: the start's own choices)
    "start.season",
    "start.traffic",
    "start.passengers",
    "start.schedule",
    "start.autostart",
];

/// Settings that show every row: Enhanced (Vanilla hides the shadows and effects), triple
/// screen and head tracking on.
fn all_rows() -> Value {
    let mut s = core::settings_from_text(None);
    s["triple_screen"] = json!(true);
    s["head_tracking"] = json!(true);
    s["info_bar_mode"] = json!("custom");
    s["graphics"] = json!("enhanced");
    s
}

fn start() -> Value {
    start_values(&Choice::default())
}

/// One frame of section `sec` (or a search for `query`), tall enough that nothing is cut
/// off. The Sound section lists one radio station (not the radio.cfg of whoever runs the
/// tests). Returns whether the settings and the start's choices changed.
fn frame(
    ui: &mut Ui,
    sec: usize,
    query: &str,
    s: &mut Value,
    start: &mut Value,
    out: &mut Outside,
) -> (bool, bool) {
    RADIO.with(|r| {
        r.borrow_mut()
            .get_or_insert_with(|| vec![("One".into(), "https://example.org/one.mp3".into())]);
    });
    ui.begin(Vec2::new(1400.0, 6000.0), 1.0, 1.0 / 60.0);
    let mut st = Store {
        s,
        start,
        changed: false,
        start_changed: false,
        omsi2: false,
    };
    let opts = listed(sec, query, false, &st);
    section_body(
        ui,
        Rect::new(0.0, 0.0, 1400.0, 6000.0),
        sec,
        !query.is_empty(),
        false,
        &opts,
        &mut st,
        out,
        std::path::Path::new("/nowhere"),
    );
    (st.changed, st.start_changed)
}

/// Click the widget `name` in section `sec`: the mouse goes down over it and comes up again.
fn click(sec: usize, name: &str, s: &mut Value, start: &mut Value) -> (Outside, bool) {
    let mut ui = Ui::new();
    let mut out = Outside::default();
    // (twice: the page's own cards take their height from the frame before)
    frame(&mut ui, sec, "", s, start, &mut out);
    frame(&mut ui, sec, "", s, start, &mut out);
    let r = *ui
        .drawn
        .get(&id_of(name))
        .unwrap_or_else(|| panic!("{name} is not in {}", SECTIONS[sec]));
    ui.input.mouse = r.center();
    ui.input.pressed = true;
    ui.input.down = true;
    let a = frame(&mut ui, sec, "", s, start, &mut out);
    ui.input.pressed = false;
    ui.input.down = false;
    ui.input.released = true;
    let b = frame(&mut ui, sec, "", s, start, &mut out);
    (out, a.0 || a.1 || b.0 || b.1)
}

#[test]
fn every_option_is_in_exactly_one_section() {
    let keys: Vec<&str> = OPTS.iter().map(|o| o.key).collect();
    for k in BEFORE {
        assert_eq!(keys.iter().filter(|x| *x == k).count(), 1, "{k}");
    }
    assert_eq!(
        keys.len(),
        BEFORE.len(),
        "an option the list above does not name"
    );
    // (each has a default to go back to, but the preset, which is not one setting)
    for o in OPTS.iter().filter(|o| o.key != "preset") {
        assert!(default_of(o.key).is_some(), "{} has no default", o.key);
    }
}

/// Each section draws a control for every option it shows.
#[test]
fn every_shown_option_has_its_control() {
    let (mut s, mut start) = (all_rows(), start());
    for sec in 0..SECTIONS.len() {
        let mut ui = Ui::new();
        frame(
            &mut ui,
            sec,
            "",
            &mut s,
            &mut start,
            &mut Outside::default(),
        );
        let st = Store {
            s: &mut s,
            start: &mut start,
            changed: false,
            start_changed: false,
            omsi2: false,
        };
        for o in OPTS
            .iter()
            .filter(|o| o.sec == sec && shown(o, &st) == Shown::Row)
        {
            // (the accent colour as swatches in its section)
            let id = if o.key == "ui_accent" {
                "accent-amber".to_string()
            } else {
                format!("opt-{}", o.key)
            };
            assert!(
                ui.drawn.contains_key(&id_of(&id)),
                "{} is not drawn in {}",
                o.key,
                SECTIONS[sec]
            );
        }
    }
}

/// Every option of the "DirectX9 (D3D9Ex)" table (`api_limits`) has a control under another
/// API, and none under DirectX9 (it is hidden or shown locked); nothing saved is rewritten.
#[test]
fn d3d9_limits_each_option_of_the_table() {
    let section_of = |key: &str| {
        OPTS.iter()
            .find(|o| o.key == key)
            .unwrap_or_else(|| panic!("{key} is not on the page"))
            .sec
    };
    let drawn = |s: &mut Value, key: &str| {
        let mut ui = Ui::new();
        frame(
            &mut ui,
            section_of(key),
            "",
            s,
            &mut start(),
            &mut Outside::default(),
        );
        ui.drawn.contains_key(&id_of(&format!("opt-{key}")))
    };
    let mut s = all_rows();
    s["graphics_api"] = json!("vulkan");
    for l in openomsi_game::testing::api_limits::D3D9 {
        assert!(
            drawn(&mut s, l.key),
            "{} has no control under Vulkan",
            l.key
        );
    }
    s["graphics_api"] = json!("d3d9");
    let before = s.clone();
    for l in openomsi_game::testing::api_limits::D3D9 {
        assert_eq!(
            drawn(&mut s, l.key),
            !cfg!(windows) || l.is_row(),
            "{} under DirectX9",
            l.key
        );
    }
    assert_eq!(s, before);
    // (VR takes DirectX 12 instead: the choice is offered)
    s["vr"] = json!(true);
    assert!(drawn(&mut s, "graphics"));
}

/// An option DirectX9 leaves some choices out of offers only the rest, and a saved value
/// outside them shows as the choice it takes.
#[test]
fn directx9_offers_only_the_choices_it_allows() {
    let (mut s, mut start) = (all_rows(), start());
    s["graphics_api"] = json!("d3d9");
    s["mirror_size"] = json!(64);
    s["mirror_refresh"] = json!("eco");
    let st = Store {
        s: &mut s,
        start: &mut start,
        changed: false,
        start_changed: false,
        omsi2: false,
    };
    for (key, want, now) in [
        ("mirror_size", vec!["0", "128", "256", "512"], "128"),
        ("mirror_refresh", vec!["off", "full"], "full"),
    ] {
        let o = OPTS.iter().find(|o| o.key == key).unwrap();
        let mut options = options_of(o, &st);
        let at = chosen(o, &st, &mut options);
        let values: Vec<_> = options.iter().map(|x| x.0.as_str()).collect();
        if cfg!(windows) {
            assert_eq!(values, want, "{key}");
            assert_eq!(options[at].0, now, "{key}");
        }
    }
}

/// A changed option is marked and goes back to its default with its own button.
#[test]
fn a_changed_option_goes_back_to_its_default() {
    let (mut s, mut start) = (all_rows(), start());
    s["anisotropy"] = json!(4);
    let (_, changed) = click(GRAPHICS, "opt-reset-anisotropy", &mut s, &mut start);
    assert!(changed);
    assert_eq!(s["anisotropy"], json!(8));
    let mut ui = Ui::new();
    frame(
        &mut ui,
        GRAPHICS,
        "",
        &mut s,
        &mut start,
        &mut Outside::default(),
    );
    assert!(!ui.drawn.contains_key(&id_of("opt-reset-anisotropy")));
}

/// A search looks through every section; "changed only" lists the changed options.
#[test]
fn a_search_finds_options_in_any_section() {
    let (mut s, mut start) = (all_rows(), start());
    s["doppler"] = json!(false);
    let st = Store {
        s: &mut s,
        start: &mut start,
        changed: false,
        start_changed: false,
        omsi2: false,
    };
    let found: Vec<&str> = listed(GENERAL, "doppler", false, &st)
        .iter()
        .map(|o| o.key)
        .collect();
    assert_eq!(found, ["doppler"]);
    let changed: Vec<&str> = listed(GENERAL, "", true, &st)
        .iter()
        .map(|o| o.key)
        .collect();
    assert!(changed.contains(&"doppler"), "{changed:?}");
    assert!(changed.contains(&"graphics") && changed.contains(&"triple_screen"));
    assert!(!changed.contains(&"volume"));
    let mut ui = Ui::new();
    frame(
        &mut ui,
        GENERAL,
        "doppler",
        &mut s,
        &mut start,
        &mut Outside::default(),
    );
    assert!(ui.drawn.contains_key(&id_of("opt-doppler")));
}

/// The Maps page's former window: its choices are the start's, changed here.
#[test]
fn the_starts_choices_are_set_here() {
    let (mut s, mut start) = (all_rows(), start());
    let saved = s.clone();
    click(TRAFFIC, "opt-start.passengers", &mut s, &mut start);
    assert_eq!(start["passengers"], json!(false));
    assert_eq!(
        s, saved,
        "the settings file is not where the start's choices go"
    );
    let mut c = Choice::default();
    apply_start(&mut c, &start);
    assert!(!c.passengers && c.schedule);
    assert_eq!(start_values(&c), start);
}

/// A colour clicked in the Interface section is the accent and is saved by its name; "Back
/// to default" is amber again.
#[test]
fn the_accent_is_chosen_in_the_interface_section_and_saved() {
    let (mut s, mut start) = (all_rows(), start());
    assert_eq!(s["ui_accent"], json!("Amber"));
    click(INTERFACE, "accent-blue", &mut s, &mut start);
    assert_eq!(s["ui_accent"], json!("Blue"));
    let saved = core::settings_to_text(&s, None);
    assert_eq!(
        core::settings_from_text(Some(&saved))["ui_accent"],
        json!("Blue")
    );
    click(INTERFACE, "accent-default", &mut s, &mut start);
    assert_eq!(s["ui_accent"], json!("Amber"));
    omsi_ui::theme::set_accent(omsi_ui::theme::Accent::default());
}

/// The Interface options are found by a search, counted as changed and taken back row by row.
#[test]
fn the_interface_options_are_searched_counted_and_reset() {
    let (mut s, mut start) = (all_rows(), start());
    let st = Store {
        s: &mut s,
        start: &mut start,
        changed: false,
        start_changed: false,
        omsi2: false,
    };
    let keys = |q: &str, st: &Store| {
        listed(0, q, false, st)
            .iter()
            .map(|o| o.key)
            .collect::<Vec<_>>()
    };
    assert!(keys("accent", &st).contains(&"ui_accent"));
    assert!(keys("corners", &st).contains(&"ui_corners"));
    drop(st);
    s["ui_corners"] = json!("square");
    s["ui_nav"] = json!("sidebar");
    s["ui_dimming"] = json!(40);
    let mut st = Store {
        s: &mut s,
        start: &mut start,
        changed: false,
        start_changed: false,
        omsi2: false,
    };
    let changed_keys: Vec<&str> = listed(0, "", true, &st).iter().map(|o| o.key).collect();
    assert!(
        ["ui_corners", "ui_nav", "ui_dimming"]
            .iter()
            .all(|k| changed_keys.contains(k)),
        "{changed_keys:?}"
    );
    let corners = OPTS.iter().find(|o| o.key == "ui_corners").unwrap();
    assert_eq!(corners.when, When::Now);
    reset(corners, &mut st);
    assert_eq!(st.get("ui_corners"), &json!("rounded"));
    // (the dimming only with the scene behind the pages)
    st.set("ui_background", json!("plain"));
    let dimming = OPTS.iter().find(|o| o.key == "ui_dimming").unwrap();
    assert_eq!(shown(dimming, &st), Shown::Hidden);
    assert!(!listed(0, "", true, &st)
        .iter()
        .any(|o| o.key == "ui_dimming"));
}

/// The mouse steering's smoothing is on unless switched off, and the switch is kept (#1092).
#[test]
fn smooth_mouse_steering_switches_off_and_is_saved() {
    let (mut s, mut start) = (all_rows(), start());
    assert_eq!(s["mouse_smooth"], json!(true));
    click(DEVICES, "opt-mouse_smooth", &mut s, &mut start);
    assert_eq!(s["mouse_smooth"], json!(false));
    let saved = core::settings_to_text(&s, None);
    assert!(saved.contains("mouse_smooth=0\n"), "{saved}");
    assert!(!openomsi_game::testing::settings::Settings::from_text(&saved).mouse_smooth);
}

#[test]
fn the_devices_lead_to_the_keys_and_the_controllers_and_reset_asks_first() {
    let (mut s, mut start) = (all_rows(), start());
    assert_eq!(
        click(DEVICES, "s-go-keys", &mut s, &mut start).0.controls,
        Some(0)
    );
    assert_eq!(
        click(DEVICES, "s-go-pads", &mut s, &mut start).0.controls,
        Some(1)
    );
    let before = s.clone();
    assert!(click(PRESETS, "s-reset", &mut s, &mut start).0.reset);
    assert_eq!(s, before);
}

/// With the original OMSI 2 as the engine only what it has is listed; the Depot's own options
/// stay out of sight and come back with the Depot engine.
#[test]
fn the_original_omsi2_lists_only_what_it_has() {
    let (mut s, mut start) = (all_rows(), start());
    let mut st = Store {
        s: &mut s,
        start: &mut start,
        changed: false,
        start_changed: false,
        omsi2: true,
    };
    for (key, want) in [
        ("max_fps", Shown::Row),
        ("collision_vehicles", Shown::Row),
        ("auto_clutch", Shown::Row),
        ("language", Shown::Row),
        ("ui_accent", Shown::Row),
        ("vr", Shown::Hidden),
        ("msaa", Shown::Hidden),
        ("head_tracking", Shown::Hidden),
        ("show_fps", Shown::Hidden),
    ] {
        let o = OPTS.iter().find(|o| o.key == key).unwrap();
        assert_eq!(shown(o, &st), want, "{key}");
    }
    st.omsi2 = false;
    let o = OPTS.iter().find(|o| o.key == "show_fps").unwrap();
    assert_eq!(shown(o, &st), Shown::Row);
}

/// The Depot's DirectX9 limits do not reach the options of the original OMSI 2.
#[test]
fn the_depot_graphics_api_does_not_limit_the_original_omsi2() {
    let (mut s, mut start) = (all_rows(), start());
    s["graphics_api"] = json!("d3d9");
    s["mirror_refresh"] = json!("eco");
    let st = Store {
        s: &mut s,
        start: &mut start,
        changed: false,
        start_changed: false,
        omsi2: true,
    };
    let o = OPTS.iter().find(|o| o.key == "mirror_refresh").unwrap();
    assert_eq!(shown(o, &st), Shown::Row);
    let mut options = options_of(o, &st);
    let values: Vec<_> = options.iter().map(|x| x.0.as_str()).collect();
    assert_eq!(values, ["eco", "full"]);
    let at = chosen(o, &st, &mut options);
    assert_eq!(options[at].0, "eco");
}
