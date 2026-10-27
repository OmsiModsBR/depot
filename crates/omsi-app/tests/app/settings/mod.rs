mod bus_seats;
mod resolution;
mod right_stick_look;
mod round_trip;

/// The Traffic AI counts follow the original's sliders: a version 2 file said 0 for "no
/// limit" and "every space", -1 for "no parked cars", and a count for the rest.
#[test]
fn the_traffic_counts_of_an_old_file_are_taken_over() {
    let old = Settings::from_text("version=2\nai_max_scheduled=0\nai_max_parked=0\n");
    assert_eq!((old.ai_max_scheduled, old.ai_max_parked), (1000, 100));
    let none = Settings::from_text("version=2\nai_max_parked=-1\nai_max_scheduled=25\n");
    assert_eq!((none.ai_max_scheduled, none.ai_max_parked), (25, 0));
    let count = Settings::from_text("version=2\nai_max_parked=250\n");
    assert_eq!(count.ai_max_parked, 100);
    let now = Settings::from_text("version=3\nai_max_scheduled=0\nai_max_parked=51\n");
    assert_eq!((now.ai_max_scheduled, now.ai_max_parked), (0, 51));
}

/// The ranges of the original's sliders.
#[test]
fn the_traffic_and_people_options_keep_to_the_originals_ranges() {
    let s = Settings::from_text(
        "version=3\nai_max_random=5000\nai_priority_scheduled=9\nai_unsched_factor=0\nai_max_humans=0\npax_density=3\n",
    );
    assert_eq!(s.ai_max_random, 1000);
    assert_eq!(s.ai_priority_scheduled, 4);
    assert!((s.ai_unsched_factor - 0.01).abs() < 1e-6);
    assert_eq!(s.ai_max_humans, 0);
    assert_eq!(s.pax_density, 2.0);
    let d = Settings::default();
    assert_eq!(
        (d.ai_max_scheduled, d.ai_max_random, d.ai_priority_scheduled),
        (1000, 1000, 4)
    );
}

/// The launcher writes the accents the game reads: the same names, in the same order, the
/// default first.
#[test]
fn the_launcher_and_the_game_know_the_same_accents() {
    let game: Vec<&str> = omsi_ui::theme::Accent::ALL
        .iter()
        .map(|a| a.name())
        .collect();
    assert_eq!(game, omsi_launcher_lib::ACCENTS);
    assert_eq!(
        omsi_launcher_lib::ACCENTS[0],
        omsi_ui::theme::Accent::default().name()
    );
}

/// Square corners are the launcher's only: the game's menus are rounded again when its
/// settings are applied (on Android the launcher ran in the same process just before).
#[test]
fn the_game_menus_keep_rounded_corners_after_a_square_launcher() {
    omsi_ui::theme::set_square(true);
    openomsi_game::testing::settings::Settings::from_text("ui_corners=square\n").apply_accent();
    assert!(!omsi_ui::theme::square());
}

/// The information bar is as the last session left it (#1164); off at first.
#[test]
fn the_information_bar_is_kept_between_sessions() {
    assert!(!openomsi_game::testing::settings::Settings::default().info_bar);
    assert!(openomsi_game::testing::settings::Settings::from_text("info_bar=1\n").info_bar);
    assert!(!openomsi_game::testing::settings::Settings::from_text("info_bar=0\n").info_bar);
}

/// "DirectX9 (D3D9Ex)" is kept on Windows only; elsewhere the setting reads "auto".
/// The locked values of the "DirectX9 (D3D9Ex)" table (`api_limits`) are what `lock_vanilla`
/// sets, option by option; a new table entry needs its check here.
#[test]
fn the_d3d9_lock_sets_every_option_of_the_table() {
    let mut s = Settings::default();
    s.msaa = 8;
    s.anisotropy = 16;
    s.render_scale = 0.5;
    s.mirror_size = 64;
    s.mirror_refresh = "eco".into();
    s.mirror_hud = 3;
    s.windy_trees = true;
    s.night_brightness = 1.0;
    s.clouds = false;
    s.lock_vanilla();
    for l in openomsi_game::testing::api_limits::D3D9 {
        let ok = match l.key {
            "graphics" => s.graphics == "vanilla" && s.classic(),
            "render_scale" => s.render_scale == 0.0,
            "mirror_size" => {
                s.mirror_size == 128 && l.allows(&s.mirror_size.to_string()) && s.mirror_hud == 0
            }
            "mirror_refresh" => s.mirror_refresh == "full" && l.allows(&s.mirror_refresh),
            "windy_trees" => !s.windy_trees,
            "clouds" => s.clouds,
            "shadows" => !s.shadows,
            "ssao" => !s.ssao,
            "detail_textures" => !s.detail_textures,
            "night_brightness" => s.night_brightness == 0.0,
            // (hidden, and with nothing of their own to switch off under Vanilla)
            "preset" | "shadow_size" | "shadow_casters" | "led_glow" | "led_mips" => true,
            other => panic!("{other} is in the table but `lock_vanilla` is not checked for it"),
        };
        assert!(ok, "{} is not set by the lock", l.key);
    }
}

#[test]
fn d3d9_has_no_graphics_improvements_to_choose() {
    for g in ["vanilla_plus", "enhanced", "enhanced_plus"] {
        let text = format!("graphics={g}\ngraphics_api=d3d9\n");
        let chosen = Settings::from_text(&text);
        assert_eq!(chosen.graphics, g);
        // (the game loads it with `locked_for_api`; "DirectX9 (D3D9Ex)" is Windows only)
        let loaded = chosen.clone().locked_for_api();
        if cfg!(windows) {
            assert_eq!(loaded.graphics, "vanilla");
            assert!(loaded.classic() && !loaded.enhanced && !loaded.ray_tracing());
            // as a file that says graphics=vanilla is loaded
            let file = Settings::from_text("graphics=vanilla\ngraphics_api=d3d9\n");
            assert_eq!(
                (loaded.shadows, loaded.ssao, loaded.detail_textures),
                (file.shadows, file.ssao, file.detail_textures)
            );
            assert!(!loaded.shadows && !loaded.ssao && !loaded.detail_textures);
            // what DirectX9 does not draw yet is off, whatever was saved (`api_limits`);
            // anti-aliasing and anisotropic filtering are drawn: kept as saved
            let saved = Settings::from_text(&format!(
                "graphics={g}\ngraphics_api=d3d9\nmsaa=8\nanisotropy=16\nrender_scale=0.5\nmirror_size=1024\nmirror_refresh=eco\nmirror_hud=3\nwindy_trees=1\npost_aa=fxaa\nclouds=0\n"
            ))
            .locked_for_api();
            assert_eq!(
                (saved.msaa, saved.anisotropy, saved.render_scale),
                (8, 16, 0.0)
            );
            // (mirrors are drawn: a saved value DirectX9 does not offer is fitted)
            assert_eq!(
                (
                    saved.mirror_size,
                    saved.mirror_refresh.as_str(),
                    saved.mirror_hud
                ),
                (512, "full", 0)
            );
            // (what it does draw stays as saved: env maps and shadow meshes)
            assert!(saved.reflections && saved.shadow_blobs);
            assert_eq!((saved.windy_trees, saved.post_aa.as_str()), (false, "off"));
            // (clouds are on, as OMSI 2 has the weather's own: a saved "off" is not taken)
            assert!(saved.clouds);
            // VR takes DirectX 12 instead, with the graphics as chosen
            let vr = Settings::from_text(&format!("graphics={g}\ngraphics_api=d3d9\nvr=1\n"))
                .locked_for_api();
            assert_eq!(vr.graphics, g);
        } else {
            assert_eq!(loaded, chosen);
        }
        // another API keeps the choice, which comes back when it is picked again
        let other =
            Settings::from_text(&format!("graphics={g}\ngraphics_api=vulkan\n")).locked_for_api();
        assert_eq!(other.graphics, g);
    }
    // --enhanced / --enhanced-plus (and their variables) do not apply to DirectX9 (D3D9Ex)
    let d3d9 = Settings::from_text("graphics_api=d3d9\n");
    assert_eq!(
        openomsi_game::testing::startup::enhanced_wanted(&d3d9, true, true),
        if cfg!(windows) {
            (false, false)
        } else {
            (true, true)
        }
    );
    let vulkan = Settings::from_text("graphics_api=vulkan\n");
    assert_eq!(
        openomsi_game::testing::startup::enhanced_wanted(&vulkan, true, true),
        (true, true)
    );
    let mut s = Settings::from_text("graphics=enhanced_plus\n");
    s.lock_vanilla();
    assert!(s.classic() && !s.enhanced && !s.ray_tracing());
}

#[test]
fn d3d9_is_a_windows_graphics_api() {
    let want = if cfg!(windows) { "d3d9" } else { "auto" };
    for v in ["d3d9", "D3D9Ex", "dx9"] {
        let s =
            openomsi_game::testing::settings::Settings::from_text(&format!("graphics_api={v}\n"));
        assert_eq!(s.graphics_api, want);
        assert_eq!(
            openomsi_game::testing::settings::Settings::from_text(&s.to_text()).graphics_api,
            want
        );
    }
    assert_eq!(
        openomsi_game::testing::settings::Settings::default().graphics_api,
        "auto"
    );
}

#[test]
fn triple_hud_defaults_to_the_centre_panel_and_can_span() {
    let mut s = openomsi_game::testing::settings::Settings::from_text("triple_screen=1\n");
    assert_eq!(s.hud_viewport((5760, 1080)), [1920.0, 0.0, 1920.0, 1080.0]);
    assert_eq!(s.hud_viewport((5761, 1080)), [1921.0, 0.0, 1921.0, 1080.0]);
    s.triple_hud_center = false;
    assert_eq!(s.hud_viewport((5761, 1080)), [0.0, 0.0, 5761.0, 1080.0]);
    let s = openomsi_game::testing::settings::Settings::from_text("triple_screen=0\n");
    assert_eq!(s.hud_viewport((1920, 1080)), [0.0, 0.0, 1920.0, 1080.0]);
}

#[test]
fn triple_fov_and_hud_preferences_round_trip() {
    let s = openomsi_game::testing::settings::Settings::from_text(
        "triple_screen=1\ntriple_hud_center=0\ntriple_fov_deg=75\nfov=50\n",
    );
    assert_eq!(
        openomsi_game::testing::settings::Settings::from_text(&s.to_text()),
        s
    );
    assert_eq!(s.triple.fov_deg, 75.0);
    assert_eq!(s.fov, 50.0);
    assert_eq!(
        openomsi_game::testing::settings::Settings::from_text("triple_fov_deg=NaN\n")
            .triple
            .fov_deg,
        0.0
    );
}
#[test]
fn outside_fov_round_trips_and_rejects_invalid_values() {
    let s = openomsi_game::testing::settings::Settings::from_text("outside_fov=48\nfov=72\n");
    assert_eq!(s.outside_fov, 48.0);
    assert_eq!(s.fov, 72.0);
    assert_eq!(
        openomsi_game::testing::settings::Settings::from_text(&s.to_text()),
        s
    );
    for invalid in ["NaN", "inf", "-1", "15", "garbage"] {
        assert_eq!(
            openomsi_game::testing::settings::Settings::from_text(&format!(
                "outside_fov={invalid}\n"
            ))
            .outside_fov,
            0.0
        );
    }
    assert_eq!(
        openomsi_game::testing::settings::Settings::from_text("outside_fov=200\n").outside_fov,
        120.0
    );
}
#[test]
fn triple_settings_round_trip_and_reject_nonfinite_geometry() {
    let s = openomsi_game::testing::settings::Settings::from_text("triple_screen=1\ntriple_span=0\ntriple_width_mm=620\ntriple_distance_mm=700\ntriple_bezel_mm=18\ntriple_left_angle_deg=50\ntriple_right_angle_deg=40\ntriple_eye_height_mm=60\n");
    assert_eq!(
        openomsi_game::testing::settings::Settings::from_text(&s.to_text()),
        s
    );
    assert!(s.triple.enabled);
    assert!(!s.triple_span);
    let invalid = openomsi_game::testing::settings::Settings::from_text("triple_width_mm=NaN\ntriple_distance_mm=inf\ntriple_left_angle_deg=-30\ntriple_bezel_mm=-20\n");
    assert_eq!(invalid.triple.width_mm, 600.0);
    assert_eq!(invalid.triple.distance_mm, 650.0);
    assert_eq!(invalid.triple.left_angle_deg, 0.0);
    assert_eq!(invalid.triple.bezel_mm, 0.0);
}
use openomsi_game::testing::settings::*;

#[test]
fn old_files_board_automatically() {
    // the launcher's old default, written without a version
    assert_eq!(
        Settings::from_text("msaa=1\nboarding=pay\n").boarding,
        "auto"
    );
    // chosen again in a current file, it stays
    assert_eq!(
        Settings::from_text("version=2\nboarding=pay\n").boarding,
        "pay"
    );
    assert_eq!(Settings::from_text("boarding=walk\n").boarding, "walk");
    // what we write reads back the same
    let s = Settings {
        boarding: "pay".into(),
        ..Default::default()
    };
    assert_eq!(Settings::from_text(&s.to_text()), s);
    let s = Settings {
        texture_compression: false,
        texture_memory: 1500,
        ..Default::default()
    };
    assert_eq!(Settings::from_text(&s.to_text()), s);
    assert_eq!(
        Settings::from_text("texmemlimit=401.0\n").texture_memory,
        401
    );
}

#[test]
fn graphics_modes() {
    // an old file: its vanilla renderer is Vanilla+ now, enhanced stays enhanced
    assert_eq!(Settings::from_text("enhanced=0\n").graphics, "vanilla_plus");
    assert_eq!(Settings::from_text("enhanced=1\n").graphics, "enhanced");
    let v = Settings::from_text("graphics=vanilla\nshadows=1\nssao=1\n");
    assert!(v.classic() && !v.shadows && !v.ssao && !v.detail_textures && !v.enhanced);
    assert!(Settings::from_text("graphics=enhanced\nenhanced=0\n").enhanced);
    assert_eq!(graphics_mode("Vanilla+"), "vanilla_plus");
    assert_eq!(graphics_mode("OMSI 2"), "vanilla");
    assert_eq!(graphics_mode("Enhanced+"), "enhanced_plus");
    let rt = Settings::from_text("graphics=enhanced_plus\nshadows=0\nssao=0\nreflections=0\n");
    assert!(rt.enhanced && rt.ray_tracing() && rt.shadows && rt.ssao && rt.reflections);
    assert!(rt.render_options().ray_tracing && !rt.render_options().no_enhanced);
    let s = Settings {
        graphics: "enhanced".into(),
        enhanced: true,
        ..Default::default()
    };
    assert_eq!(Settings::from_text(&s.to_text()), s);
}

#[test]
fn mirror_size_zero_disables_mirror_rendering() {
    assert_eq!(Settings::from_text("mirror_size=0\n").mirror_size, 0);
    assert_eq!(Settings::from_text("mirror_size=32\n").mirror_size, 64);
    let off = Settings {
        mirror_size: 0,
        ..Default::default()
    };
    assert_eq!(Settings::from_text(&off.to_text()).mirror_size, 0);
}

#[test]
fn mirror_refresh_reads_omsi_realreflexions_and_round_trips() {
    assert_eq!(Settings::default().mirror_refresh, "full");
    assert_eq!(
        Settings::from_text("performance_realreflexions=none\n").mirror_refresh,
        "off"
    );
    assert_eq!(
        Settings::from_text("performance_realreflexions=economy\n").mirror_refresh,
        "eco"
    );
    assert_eq!(
        Settings::from_text("performance_realreflexions=full\n").mirror_refresh,
        "full"
    );
    assert_eq!(
        Settings::from_text("mirror_refresh=nonsense\n").mirror_refresh,
        "full"
    );
    for mode in ["off", "eco", "full"] {
        let s = Settings {
            mirror_refresh: mode.into(),
            ..Default::default()
        };
        assert_eq!(Settings::from_text(&s.to_text()).mirror_refresh, mode);
    }
}

#[test]
fn steering_view_settings_round_trip_and_reject_invalid_values() {
    let settings = Settings {
        steer_look: true,
        steer_look_angle: 45.0,
        steer_look_response: 0.5,
        ..Default::default()
    };
    assert_eq!(Settings::from_text(&settings.to_text()), settings);
    let invalid = Settings::from_text("steer_look_angle=NaN\nsteer_look_response=NaN\n");
    assert_eq!(invalid.steer_look_angle, 30.0);
    assert_eq!(invalid.steer_look_response, 0.25);
    let clamped = Settings::from_text("steer_look_angle=999\nsteer_look_response=-1\n");
    assert_eq!(clamped.steer_look_angle, 60.0);
    assert_eq!(clamped.steer_look_response, 0.05);
}

#[test]
fn seat_pitch_round_trips_and_is_clamped() {
    let settings = Settings {
        seat_pitch_deg: -12.0,
        ..Default::default()
    };
    assert_eq!(Settings::from_text(&settings.to_text()), settings);
    assert_eq!(
        Settings::from_text("seat_pitch_deg=NaN\n").seat_pitch_deg,
        0.0
    );
    assert_eq!(
        Settings::from_text("seat_pitch_deg=90\n").seat_pitch_deg,
        45.0
    );
}

#[test]
fn vr_mirror_rates_round_trip_including_every_frame() {
    for rate in [-1.0, 0.0, 16.0, 60.0, 120.0, 240.0, 360.0] {
        let s = Settings {
            vr_mirror_rate: rate,
            ..Default::default()
        };
        assert_eq!(Settings::from_text(&s.to_text()).vr_mirror_rate, rate);
    }
    assert_eq!(
        Settings::from_text("vr_mirror_rate=NaN\n").vr_mirror_rate,
        16.0
    );
    assert_eq!(
        Settings::from_text("vr_mirror_rate=999\n").vr_mirror_rate,
        360.0
    );
}

#[test]
fn interface_size_is_read_within_its_range() {
    assert_eq!(Settings::default().ui_scale, 1.0);
    assert_eq!(Settings::from_text("ui_scale=1.25\n").ui_scale, 1.25);
    assert_eq!(Settings::from_text("ui_scale=5\n").ui_scale, 2.0);
    assert_eq!(Settings::from_text("ui_scale=0\n").ui_scale, 0.5);
    assert_eq!(Settings::from_text("ui_scale=NaN\n").ui_scale, 1.0);
    let s = Settings {
        ui_scale: 1.5,
        ui_scale_window: false,
        ui_opacity: 0.4,
        notes: false,
        ..Default::default()
    };
    assert_eq!(Settings::from_text(&s.to_text()), s);
    // (the opacity under its old name, the navigator's)
    assert_eq!(
        Settings::from_text("navigator_opacity=0.5\n").ui_opacity,
        0.5
    );
    assert_eq!(Settings::from_text("ui_opacity=7\n").ui_opacity, 1.0);
}

#[test]
fn old_discord_lines_are_ignored_and_not_written() {
    // Discord support is disabled (docs/removed/DISCORD_DISABLED.md)
    let s = Settings::from_text("discord_status=0\ndiscord_app_id=123456\n");
    assert_eq!(s, Settings::default());
    let text = Settings::default().to_text();
    assert!(!text.contains("discord"), "{text}");
}

#[test]
fn voice_chat_round_trips_and_is_on_by_default() {
    assert!(Settings::default().voice_chat);
    let s = Settings {
        voice_chat: false,
        ..Default::default()
    };
    assert_eq!(Settings::from_text(&s.to_text()), s);
}

#[test]
fn post_aa_is_read_and_written() {
    assert_eq!(Settings::from_text("enhanced=1\n").post_aa, "fxaa");
    let off = Settings::from_text("enhanced=1\npost_aa=off\n");
    assert_eq!(off.post_aa, "off");
    assert!(!off.render_options().fxaa);
    assert!(Settings::from_text("post_aa=FXAA").render_options().fxaa);
    assert_eq!(Settings::from_text(&off.to_text()), off);
}

#[test]
fn the_wheels_tremble_and_its_fade_out_survive_a_save() {
    let d = Settings::default();
    assert_eq!(
        (d.ff_road_vib, d.ff_engine_vib, d.ff_fade),
        (1.0, 1.0, 0.28)
    );
    let s = Settings {
        ff_road_vib: 1.5,
        ff_engine_vib: 0.0,
        ff_fade: 0.5,
        ..Default::default()
    };
    assert_eq!(Settings::from_text(&s.to_text()), s);
    // what the file may hold is held between none and twice as much, and a fade of
    // no more than a second - a number outside is brought back in rather than lost
    let wild = Settings::from_text("ff_road_vib=9\nff_engine_vib=-3\nff_fade=12\n");
    assert_eq!(
        (wild.ff_road_vib, wild.ff_engine_vib, wild.ff_fade),
        (4.0, 0.0, 1.5)
    );
    // a file that says nothing keeps the wheel feeling as it did out of the box
    let old = Settings::from_text("msaa=1\n");
    assert_eq!(
        (old.ff_road_vib, old.ff_engine_vib, old.ff_fade),
        (1.0, 1.0, 0.28)
    );
    // and one that says nonsense is left as it was
    let bad = Settings::from_text("ff_road_vib=loud\nff_fade=\n");
    assert_eq!((bad.ff_road_vib, bad.ff_fade), (1.0, 0.28));
}
