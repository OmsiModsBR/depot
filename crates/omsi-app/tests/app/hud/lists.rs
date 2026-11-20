use openomsi_game::testing;

/// Each graphics mode: an option it does not offer has no row, a fixed one an info line with
/// its fixed value and the reason, an offered one its own row (`omsi_cfg::graphics_options`).
#[test]
fn the_menu_rows_follow_the_graphics_option_table() {
    use omsi_cfg::graphics_options::{Applies, GRAPHICS_OPTIONS};
    let own = || Some(("own".to_string(), "id".to_string()));
    for mode in omsi_cfg::GraphicsMode::ALL {
        for o in GRAPHICS_OPTIONS {
            let row = testing::hud::lists::limit_row(mode, o.key, "Name", own);
            match o.applies(mode) {
                Applies::Offered | Applies::Choices { .. } => {
                    assert_eq!(row, own(), "{} in {mode:?}", o.key)
                }
                Applies::NotOffered { .. } => assert!(row.is_none(), "{} in {mode:?}", o.key),
                Applies::Fixed { shown, .. } => {
                    let (text, action) = row.unwrap();
                    assert_eq!(action, "noop");
                    assert!(text.contains("Name") && text.contains(shown), "{text}");
                }
            }
        }
        // (an option with no row in the table is the mode's in every mode)
        assert_eq!(
            testing::hud::lists::limit_row(mode, "volume", "Name", own),
            own()
        );
    }
}

/// A destination picked from the list keeps the route number the bus shows, its letter
/// too (92E, IBIS 92 and suffix 10).
#[test]
fn a_destination_picked_from_the_list_keeps_the_lines_letter() {
    let mut v = testing::script_test_vehicle(
        "{frame}\n{end}\n",
        "IBIS_LinieKurs\nIBIS_Linie_Suffix\nIBIS_Linie_Complex\nIBIS_TerminusCode\n",
        "Matrix_Nr\nSetLineTo\n",
    );
    let set_str = |v: &mut omsi_sim::VehicleInstance, name: &str, s: &str| {
        let i = v.ty.program.str_var(name).unwrap();
        v.state.str_vars[i as usize] = s.to_string();
    };
    v.set_var("IBIS_LinieKurs", 92.0);
    v.set_var("IBIS_Linie_Suffix", 10.0);
    v.set_var("IBIS_Linie_Complex", 9210.0);
    set_str(&mut v, "Matrix_Nr", "92E");
    assert_eq!(testing::hud::lists::destination_line(&v), "92E");
    let hof = omsi_vehicle::Hof {
        termini: vec![omsi_vehicle::hof::Terminus {
            code: 211,
            texture_id: "U Rathaus Spandau".into(),
            strings: vec!["RATHAUS SPANDAU".into()],
            ..Default::default()
        }],
        ..Default::default()
    };
    let line = testing::hud::lists::destination_line(&v);
    testing::schedule::set_player_destination_at(&mut v, &hof, &line, 0, &[]);
    assert_eq!(
        (
            v.var("IBIS_LinieKurs"),
            v.var("IBIS_Linie_Suffix"),
            v.var("IBIS_Linie_Complex"),
            v.var("IBIS_TerminusCode")
        ),
        (Some(92.0), Some(10.0), Some(9210.0), Some(211.0))
    );
    // a route number the IBIS cannot take is left to the display: the IBIS keeps its own
    set_str(&mut v, "Matrix_Nr", "-10");
    assert_eq!(testing::hud::lists::destination_line(&v), "92");
    // nothing shown: the IBIS's number
    set_str(&mut v, "Matrix_Nr", "   ");
    set_str(&mut v, "SetLineTo", "");
    assert_eq!(testing::hud::lists::destination_line(&v), "92");
    // no matrix line, the line of an earlier pick left in SetLineTo and another typed on
    // the IBIS since: the IBIS's (only a roller blind shows SetLineTo)
    set_str(&mut v, "Matrix_Nr", "");
    set_str(&mut v, "SetLineTo", "5");
    v.set_var("IBIS_LinieKurs", 145.0);
    assert_eq!(testing::hud::lists::destination_line(&v), "145");
    let mut blind = testing::script_test_vehicle(
        "{trigger:rollband_sync}\n{end}\n",
        "IBIS_LinieKurs\n",
        "SetLineTo\n",
    );
    set_str(&mut blind, "SetLineTo", "  5");
    blind.set_var("IBIS_LinieKurs", 145.0);
    assert_eq!(testing::hud::lists::destination_line(&blind), "5");
}

#[test]
fn escape_fov_updates_the_active_projection_and_can_restore_geometry() {
    let mut settings = testing::settings::Settings::default();
    assert_eq!(
        testing::hud::lists::set_camera_fov(&mut settings, 50.0).0,
        "fov"
    );
    settings.triple.enabled = true;
    assert_eq!(
        testing::hud::lists::set_camera_fov(&mut settings, 75.0).0,
        "triple_fov_deg"
    );
    assert_eq!(settings.fov, 50.0);
    assert_eq!(settings.triple.fov_deg, 75.0);
    testing::hud::lists::set_camera_fov(&mut settings, 0.0);
    assert_eq!(settings.triple.fov_deg, 0.0);
}
/// The game menu's graphics list is the launcher's, less what the running mode does not offer
/// or what only takes effect at the next start (spec 001, FR-020c).
#[test]
fn the_ingame_graphics_list_offers_only_the_modes_live_options() {
    use omsi_cfg::graphics_options::offered_in_game;
    use omsi_launcher_ui::window::{OPTS, SECTIONS};
    for mode in omsi_cfg::GraphicsMode::ALL {
        let mut settings = testing::settings::Settings::default();
        settings.graphics = mode;
        settings.render_scale = 0.5;
        settings.rain_reflections = true;
        let keys: Vec<&str> = testing::hud::lists::ingame_graphics_options(&settings)
            .iter()
            .map(|o| o.key)
            .collect();
        let want: Vec<&str> = OPTS
            .iter()
            .filter(|o| matches!(SECTIONS[o.sec], "Graphics" | "Advanced graphics"))
            .filter(|o| offered_in_game(o.key, mode))
            .map(|o| o.key)
            .collect();
        assert_eq!(keys, want, "{mode:?}");
        assert!(!keys.is_empty(), "{mode:?}");
        for key in [
            "graphics",
            "graphics_api",
            "preset",
            "msaa",
            "anisotropy",
            "vr",
        ] {
            assert!(!keys.contains(&key), "{key} in {mode:?}");
        }
    }
}

/// Ambient occlusion is changed at once in the modes that draw it (`Renderer::set_ssao`).
#[test]
fn ambient_occlusion_is_a_live_row_where_the_mode_draws_it() {
    use omsi_cfg::GraphicsMode;
    let has_ssao = |mode| {
        let mut settings = testing::settings::Settings::default();
        settings.graphics = mode;
        testing::hud::lists::ingame_graphics_options(&settings)
            .iter()
            .any(|o| o.key == "ssao")
    };
    assert!(!has_ssao(GraphicsMode::Vanilla));
    assert!(has_ssao(GraphicsMode::VanillaPlus));
    assert!(has_ssao(GraphicsMode::Enhanced));
}

/// The list follows the mode the game runs in: a mode saved since (for the next start) does
/// not change it.
#[test]
fn the_ingame_graphics_list_follows_the_running_mode() {
    use omsi_cfg::GraphicsMode;
    let keys = |mode| {
        let mut settings = testing::settings::Settings::default();
        settings.graphics = mode;
        testing::hud::lists::ingame_graphics_options(&settings)
            .iter()
            .map(|o| o.key)
            .collect::<Vec<_>>()
    };
    assert!(keys(GraphicsMode::Vanilla).contains(&"dx9_coronas"));
    assert!(!keys(GraphicsMode::Enhanced).contains(&"dx9_coronas"));
    assert!(keys(GraphicsMode::Enhanced).contains(&"led_glow"));
    assert!(!keys(GraphicsMode::Vanilla).contains(&"led_glow"));
}

/// A row that waits for another: the upscaling only below 100 % scale, the people in the
/// rain reflections only with the reflections on.
#[test]
fn ingame_rows_that_wait_for_another_follow_it() {
    let mut settings = testing::settings::Settings::default();
    settings.graphics = omsi_cfg::GraphicsMode::Vanilla;
    let has = |s: &testing::settings::Settings, key: &str| {
        testing::hud::lists::ingame_graphics_options(s)
            .iter()
            .any(|o| o.key == key)
    };
    settings.render_scale = 0.0;
    settings.rain_reflections = false;
    assert!(!has(&settings, "dx9_upscale"));
    assert!(!has(&settings, "humans_in_rain_reflections"));
    settings.render_scale = 0.75;
    settings.rain_reflections = true;
    assert!(has(&settings, "dx9_upscale"));
    assert!(has(&settings, "humans_in_rain_reflections"));
}

/// A drop-down takes the launcher's choices, less what the mode leaves out.
#[test]
fn ingame_choices_are_the_launchers_within_the_mode() {
    let file = serde_json::json!({ "graphics": "vanilla", "mirror_refresh": "full" });
    let (options, at, _) = testing::hud::lists::select_state(&file, "mirror_refresh");
    let values: Vec<&str> = options.iter().map(|o| o.0).collect();
    assert_eq!(values, ["off", "full"]);
    assert_eq!(at, Some(1));
}

#[test]
fn lines_sort_as_numbers() {
    let mut v = vec!["13N", "5", "137", "N30", "92"];
    v.sort_by(|a, b| testing::hud::lists::natural(a, b));
    assert_eq!(v, vec!["5", "13N", "92", "137", "N30"]);
}

#[test]
fn symbols_are_not_forced_through_numeric_ibis_lines() {
    assert!(testing::hud::lists::numeric_ibis_line("10"));
    assert!(testing::hud::lists::numeric_ibis_line("10E"));
    assert!(testing::hud::lists::numeric_ibis_line("X10"));
    assert!(!testing::hud::lists::numeric_ibis_line("-10"));
    assert!(!testing::hud::lists::numeric_ibis_line("10-"));
    assert!(!testing::hud::lists::numeric_ibis_line("EXP"));
}
