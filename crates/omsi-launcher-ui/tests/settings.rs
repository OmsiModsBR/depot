//! The Settings page without a window: every option in one section with its control, the
//! graphics modes' table, the search, the reset and what the page asks of the launcher.

use glam::Vec2;
use omsi_cfg::graphics_options::{self, Applies, GRAPHICS_OPTIONS};
use omsi_cfg::GraphicsMode;
use omsi_content::input::gamectrler::DeviceCfg;
use omsi_launcher_lib as core;
use omsi_launcher_lib::start::Choice;
use omsi_launcher_ui::window::*;
use omsi_ui::widgets::{id_of, Ui};
use omsi_ui::Rect;
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
    "smoke",
    "smoke_user_only",
    "smoke_max",
    "smoke_in_mirrors",
    "rain_reflections",
    "humans_in_rain_reflections",
    "fewer_night_lights",
    "dx9_upscale",
    "dx9_glass_rain",
    "dx9_coronas",
    "dx9_stars",
    "dx9_cloud_picture",
    "dx9_texture_limit",
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
    "developer_mode",
    "ui_scale",
    "ui_scale_window",
    "ui_opacity",
    "tooltips",
    "show_fps",
    "info_bar",
    "info_bar_mode",
    "info_bar_pos",
    "info_bar_fps",
    "info_bar_collapsed",
    "info_bar_labels",
    "info_bar_lowfuel",
    "info_item_time",
    "info_item_speed",
    "info_item_trip",
    "info_item_odometer",
    "info_item_temps",
    "info_item_tank",
    "info_item_pax",
    "info_item_line",
    "info_item_next",
    "info_item_pedals",
    "info_item_grade",
    "info_item_skid",
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
    "physics",
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

/// The game, as the page sees it: one radio station (not the radio.cfg of whoever runs the
/// tests), nothing else.
struct Quiet;

impl omsi_launcher_ui::Host for Quiet {
    fn set_language(&self, _: &str) {}
    fn open_url(&self, _: &str) {}
    fn open_dotnet_download(&self) {}
    fn version(&self) -> (&'static str, &'static str) {
        ("0.0.0", "test")
    }
    fn repo_url(&self) -> &'static str {
        "https://example.org/depot"
    }

    fn releases_url(&self) -> &'static str {
        "https://example.org/depot/releases"
    }
    fn action_text(&self, _: &str, _: &str, action: &str) -> String {
        action.into()
    }
    fn actions(&self, _: &str, _: &str) -> Vec<(String, String)> {
        Vec::new()
    }
    fn key_name(&self, _: i64, _: i64) -> String {
        String::new()
    }
    fn dik_code(&self, _: winit::keyboard::KeyCode) -> Option<i32> {
        None
    }
    fn controllers_file(&self) -> Option<std::path::PathBuf> {
        None
    }
    fn save_controllers(&self, _: &[DeviceCfg]) -> Result<(), String> {
        Ok(())
    }
    fn adapter_texture_mb(&self) -> i64 {
        0
    }
    fn own_stations(&self) -> Vec<(String, String)> {
        vec![("One".into(), "https://example.org/one.mp3".into())]
    }
    fn save_stations(&self, _: &[(String, String)]) -> std::io::Result<()> {
        Ok(())
    }
}

/// One frame of section `sec` (or a search for `query`), tall enough that nothing is cut
/// off. Returns whether the settings and the start's choices changed.
fn frame(
    ui: &mut Ui,
    sec: usize,
    query: &str,
    s: &mut Value,
    start: &mut Value,
    out: &mut Outside,
) -> (bool, bool) {
    ui.begin(Vec2::new(1400.0, 6000.0), 1.0, 1.0 / 60.0);
    let mut st = Store {
        s,
        start,
        changed: false,
        start_changed: false,
        omsi2: false,
        adapter_mb: 0,
        ceiling_mb: 0,
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
        &Quiet,
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
            adapter_mb: 0,
            ceiling_mb: 0,
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

/// Every option of the Graphics and Advanced graphics sections, VR and the triple screen has a
/// row in the graphics modes' table (spec 001, FR-020).
#[test]
fn every_graphics_option_has_a_row_in_the_table() {
    for o in OPTS.iter().filter(|o| {
        o.sec == GRAPHICS
            || SECTIONS[o.sec] == "Advanced graphics"
            || o.group == "VR headset"
            || o.group == "Triple screen"
    }) {
        assert!(
            graphics_options::graphics_option(o.key).is_some(),
            "{} has no row in GRAPHICS_OPTIONS",
            o.key
        );
    }
    for row in GRAPHICS_OPTIONS {
        assert!(
            OPTS.iter().any(|o| o.key == row.key),
            "{} is in the table but not on the page",
            row.key
        );
    }
}

/// In each mode the page shows each option of the table as the table says: no row where it is
/// not offered, a fixed line where it is fixed, a control where it is offered; nothing saved is
/// rewritten (spec 001, FR-014, FR-015, FR-016).
#[test]
fn the_page_offers_what_the_table_says_in_each_mode() {
    for (mode, name) in [
        (GraphicsMode::Vanilla, "vanilla"),
        (GraphicsMode::VanillaPlus, "vanilla_plus"),
        (GraphicsMode::Enhanced, "enhanced"),
    ] {
        let (mut s, mut start) = (all_rows(), start());
        s["graphics"] = json!(name);
        // (what the conditional rows wait for)
        s["render_scale"] = json!("0.75");
        s["rain_reflections"] = json!(true);
        s["vr"] = json!(false);
        let before = s.clone();
        let st = Store {
            s: &mut s,
            start: &mut start,
            changed: false,
            start_changed: false,
            omsi2: false,
            adapter_mb: 0,
            ceiling_mb: 0,
        };
        for row in GRAPHICS_OPTIONS {
            let o = OPTS.iter().find(|o| o.key == row.key).unwrap();
            let got = shown(o, &st);
            match row.applies(mode) {
                Applies::NotOffered { .. } => {
                    assert_eq!(got, Shown::Hidden, "{} in {mode:?}", row.key)
                }
                Applies::Fixed { .. } => assert!(
                    matches!(got, Shown::Locked(_)),
                    "{} in {mode:?}: {got:?}",
                    row.key
                ),
                // (the platform's own rules: no VR off Windows, no API choice on a Mac; the VR
                // headset's settings wait for it, which is off here)
                Applies::Offered | Applies::Choices { .. }
                    if row.key == "graphics_api"
                        || row.key == "vr"
                        || row.key.starts_with("vr_")
                        || row.key == "triple_fov_deg" => {}
                Applies::Offered | Applies::Choices { .. } => {
                    assert_eq!(got, Shown::Row, "{} in {mode:?}", row.key)
                }
            }
        }
        assert_eq!(s, before, "{mode:?}");
    }
}

/// The Graphics page leads with the graphics mode, in a card of its own that says what it
/// decides; the graphics API is a row of "Picture" in Vanilla+ and Enhanced and absent in
/// Vanilla (spec 001, FR-020a, FR-020b).
#[test]
fn the_graphics_page_leads_with_the_mode_and_puts_the_api_in_picture() {
    let first = OPTS.iter().find(|o| o.sec == GRAPHICS).unwrap();
    assert_eq!((first.key, first.group), ("graphics", "Graphics mode"));
    let api = OPTS.iter().find(|o| o.key == "graphics_api").unwrap();
    assert_eq!((api.sec, api.group), (GRAPHICS, "Picture"));
    let drawn = |mode: &str, id: &str| {
        let mut s = all_rows();
        s["graphics"] = json!(mode);
        let mut ui = Ui::new();
        frame(
            &mut ui,
            GRAPHICS,
            "",
            &mut s,
            &mut start(),
            &mut Outside::default(),
        );
        ui.drawn.contains_key(&id_of(id))
    };
    for mode in ["vanilla", "vanilla_plus", "enhanced"] {
        assert!(drawn(mode, "opt-graphics"), "the mode in {mode}");
    }
    assert!(!drawn("vanilla", "opt-graphics_api"));
    for mode in ["vanilla_plus", "enhanced"] {
        assert_eq!(
            drawn(mode, "opt-graphics_api"),
            !cfg!(target_os = "macos"),
            "the API in {mode}"
        );
    }
    // (each mode has its line on the card)
    assert_eq!(MODE_LINES.len(), GraphicsMode::ALL.len());
}

/// A search finds only the options the graphics mode offers (spec 001, contracts/graphics-options.md).
#[test]
fn a_search_finds_only_the_options_the_mode_offers() {
    let (mut s, mut start) = (all_rows(), start());
    s["graphics"] = json!("vanilla_plus");
    let st = Store {
        s: &mut s,
        start: &mut start,
        changed: false,
        start_changed: false,
        omsi2: false,
        adapter_mb: 0,
        ceiling_mb: 0,
    };
    let keys = |q: &str| -> Vec<&str> {
        listed(GRAPHICS, q, false, &st)
            .iter()
            .map(|o| o.key)
            .collect()
    };
    // (LED glow is Enhanced's; raindrops on glass Vanilla's)
    assert!(!keys("LED glow").contains(&"led_glow"));
    assert!(!keys("Raindrops").contains(&"dx9_glass_rain"));
    assert!(keys("Anti-aliasing").contains(&"msaa"));
}

/// Vanilla opens without VR and the triple screen: the page does not offer them there, and
/// keeps what was saved for the other modes (spec 001, FR-018a).
#[test]
fn vanilla_offers_no_vr_and_no_triple_screen() {
    let mut s = all_rows();
    s["vr"] = json!(false);
    let mut start = start();
    let shown_in = |s: &mut Value, start: &mut Value, key: &str| {
        let st = Store {
            s,
            start,
            changed: false,
            start_changed: false,
            omsi2: false,
            adapter_mb: 0,
            ceiling_mb: 0,
        };
        shown(OPTS.iter().find(|o| o.key == key).unwrap(), &st)
    };
    assert_eq!(shown_in(&mut s, &mut start, "triple_screen"), Shown::Row);
    assert_eq!(shown_in(&mut s, &mut start, "triple_width_mm"), Shown::Row);
    s["graphics"] = json!("vanilla");
    for key in ["triple_screen", "triple_width_mm", "vr", "vr_scale"] {
        assert_eq!(shown_in(&mut s, &mut start, key), Shown::Hidden, "{key}");
    }
    assert_eq!(s["triple_screen"], json!(true));
}

/// An option Vanilla leaves some choices out of offers only the rest, and a saved value
/// outside them shows as the choice it takes.
#[test]
fn vanilla_offers_only_the_choices_it_allows() {
    let (mut s, mut start) = (all_rows(), start());
    s["graphics"] = json!("vanilla");
    s["mirror_size"] = json!(64);
    s["mirror_refresh"] = json!("eco");
    let st = Store {
        s: &mut s,
        start: &mut start,
        changed: false,
        start_changed: false,
        omsi2: false,
        adapter_mb: 0,
        ceiling_mb: 0,
    };
    for (key, want, now) in [
        ("mirror_size", vec!["0", "128", "256", "512"], "128"),
        ("mirror_refresh", vec!["off", "full"], "full"),
    ] {
        let o = OPTS.iter().find(|o| o.key == key).unwrap();
        let mut options = options_of(o, &st);
        let at = chosen(o, &st, &mut options);
        let values: Vec<_> = options.iter().map(|x| x.0.as_str()).collect();
        assert_eq!(values, want, "{key}");
        assert_eq!(options[at].0, now, "{key}");
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
        adapter_mb: 0,
        ceiling_mb: 0,
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
        adapter_mb: 0,
        ceiling_mb: 0,
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
        adapter_mb: 0,
        ceiling_mb: 0,
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

/// Developer mode is off until the player switches it on, the switch is kept in the file, and
/// the game's own settings parser neither needs nor changes anything for it (#500, the game's
/// side in omsi-app's tests).
#[test]
fn developer_mode_is_off_by_default_and_the_switch_is_saved() {
    let (mut s, mut start) = (all_rows(), start());
    assert_eq!(s["developer_mode"], json!(false));
    assert_eq!(default_of("developer_mode"), Some(json!(false)));
    let off = core::settings_to_text(&s, None);
    assert!(off.contains("developer_mode=0\n"), "{off}");
    click(DEVELOPER, "opt-developer_mode", &mut s, &mut start);
    assert_eq!(s["developer_mode"], json!(true));
    let on = core::settings_to_text(&s, None);
    assert!(on.contains("developer_mode=1\n"), "{on}");
    let back = core::settings_from_text(Some(&on));
    assert_eq!(back["developer_mode"], json!(true));
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
        adapter_mb: 0,
        ceiling_mb: 0,
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

/// The limits of the Depot's Vanilla do not reach the options of the original OMSI 2.
#[test]
fn the_depot_vanilla_does_not_limit_the_original_omsi2() {
    let (mut s, mut start) = (all_rows(), start());
    s["graphics"] = json!("vanilla");
    s["mirror_refresh"] = json!("eco");
    let st = Store {
        s: &mut s,
        start: &mut start,
        changed: false,
        start_changed: false,
        omsi2: true,
        adapter_mb: 0,
        ceiling_mb: 0,
    };
    let o = OPTS.iter().find(|o| o.key == "mirror_refresh").unwrap();
    assert_eq!(shown(o, &st), Shown::Row);
    let mut options = options_of(o, &st);
    let values: Vec<_> = options.iter().map(|x| x.0.as_str()).collect();
    assert_eq!(values, ["eco", "full"]);
    let at = chosen(o, &st, &mut options);
    assert_eq!(options[at].0, "eco");
}

/// The Performance section gathers existing options (each key is one option of another
/// section) and is offered in Vanilla on Windows only.
#[test]
fn the_performance_section_is_vanillas_and_owns_no_option() {
    let mut seen = std::collections::HashSet::new();
    for (key, _) in PERFORMANCE_OPTS {
        assert!(seen.insert(*key), "{key} is listed twice");
        assert!(OPTS.iter().any(|o| o.key == *key), "{key} is not an option");
    }
    assert!(OPTS.iter().all(|o| o.sec != PERFORMANCE));
    let drawn = |mode: &str, id: &str| {
        let mut s = all_rows();
        s["graphics"] = json!(mode);
        let mut ui = Ui::new();
        frame(
            &mut ui,
            PERFORMANCE,
            "",
            &mut s,
            &mut start(),
            &mut Outside::default(),
        );
        ui.drawn.contains_key(&id_of(id))
    };
    // (the page offers the section only in the Depot's Vanilla)
    let offered = |mode: &str, omsi2: bool| {
        let mut s = all_rows();
        s["graphics"] = json!(mode);
        performance_offered(&s, omsi2)
    };
    assert!(!offered("enhanced", false));
    assert_eq!(offered("vanilla", false), cfg!(windows));
    assert!(!offered("vanilla", true));
    // (the section's body lists its rows even where the page does not offer it; here only the
    // page's gate, above, decides, and the rows draw on Windows)
    if cfg!(windows) {
        for key in [
            "mirror_size",
            "max_obj_dist",
            "ai_max_random",
            "max_fps",
            "vsync",
        ] {
            assert!(drawn("vanilla", &format!("opt-{key}")), "{key} in Vanilla");
        }
    }
}

/// The engine is chosen in the Simulation section (spec 002, FR-003): OMSI Classic by default
/// or Project Depot Physics, with the one-line notice under the list; the choice is saved as
/// `physics=classic|depot` and read back. The original OMSI 2 has no such setting.
#[test]
fn the_physics_list_is_saved_and_read_back_with_its_notice() {
    let o = OPTS.iter().find(|o| o.key == "physics").unwrap();
    assert_eq!(o.sec, SIMULATION);
    assert_eq!(o.hint, PHYSICS_NOTICE);
    assert!(PHYSICS_NOTICE.contains("[depot_physics]") && PHYSICS_NOTICE.contains("OMSI 2"));
    let (mut s, mut start) = (all_rows(), start());
    assert_eq!(default_of("physics"), Some(json!("classic")));
    assert_eq!(s["physics"], json!("classic"));
    let st = Store {
        s: &mut s,
        start: &mut start,
        changed: false,
        start_changed: false,
        omsi2: false,
        adapter_mb: 0,
        ceiling_mb: 0,
    };
    let choices: Vec<String> = options_of(o, &st).into_iter().map(|c| c.0).collect();
    assert_eq!(choices, ["classic", "depot"]);
    assert_eq!(shown(o, &st), Shown::Row);
    let omsi2 = Store { omsi2: true, ..st };
    assert_eq!(shown(o, &omsi2), Shown::Hidden);
    for mode in ["depot", "classic"] {
        s["physics"] = json!(mode);
        let text = core::settings_to_text(&s, None);
        assert!(text.contains(&format!("physics={mode}\n")), "{text}");
        assert_eq!(
            core::settings_from_text(Some(&text))["physics"],
            json!(mode)
        );
    }
}
