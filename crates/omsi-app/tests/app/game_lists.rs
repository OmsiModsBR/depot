use openomsi_game::testing;

/// "DirectX9 (D3D9Ex)": a hidden option has no row, a locked one an info line with its fixed
/// value and the reason, an unlimited one its own row; other APIs leave every row alone.
#[test]
fn the_menu_rows_follow_the_directx9_limit_table() {
    let own = || Some(("own".to_string(), "id".to_string()));
    for l in testing::api_limits::D3D9 {
        let row = testing::game_lists::limit_row(true, l.key, "Name", own);
        match l.shown() {
            None if l.is_row() => assert_eq!(row, own(), "{} is a row", l.key),
            None => assert!(row.is_none(), "{} is hidden", l.key),
            Some(v) => {
                let (text, action) = row.unwrap();
                assert_eq!(action, "noop");
                assert!(text.contains("Name") && text.contains(v), "{text}");
            }
        }
        assert_eq!(
            testing::game_lists::limit_row(false, l.key, "Name", own),
            own()
        );
    }
    assert_eq!(
        testing::game_lists::limit_row(true, "view_distance", "Name", own),
        own()
    );
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
    assert_eq!(testing::game_lists::destination_line(&v), "92E");
    let hof = omsi_vehicle::Hof {
        termini: vec![omsi_vehicle::hof::Terminus {
            code: 211,
            texture_id: "U Rathaus Spandau".into(),
            strings: vec!["RATHAUS SPANDAU".into()],
            ..Default::default()
        }],
        ..Default::default()
    };
    let line = testing::game_lists::destination_line(&v);
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
    assert_eq!(testing::game_lists::destination_line(&v), "92");
    // nothing shown: the IBIS's number
    set_str(&mut v, "Matrix_Nr", "   ");
    set_str(&mut v, "SetLineTo", "");
    assert_eq!(testing::game_lists::destination_line(&v), "92");
    // no matrix line, the line of an earlier pick left in SetLineTo and another typed on
    // the IBIS since: the IBIS's (only a roller blind shows SetLineTo)
    set_str(&mut v, "Matrix_Nr", "");
    set_str(&mut v, "SetLineTo", "5");
    v.set_var("IBIS_LinieKurs", 145.0);
    assert_eq!(testing::game_lists::destination_line(&v), "145");
    let mut blind = testing::script_test_vehicle(
        "{trigger:rollband_sync}\n{end}\n",
        "IBIS_LinieKurs\n",
        "SetLineTo\n",
    );
    set_str(&mut blind, "SetLineTo", "  5");
    blind.set_var("IBIS_LinieKurs", 145.0);
    assert_eq!(testing::game_lists::destination_line(&blind), "5");
}

#[test]
fn escape_fov_updates_the_active_projection_and_can_restore_geometry() {
    let mut settings = testing::settings::Settings::default();
    assert_eq!(
        testing::game_lists::set_camera_fov(&mut settings, 50.0).0,
        "fov"
    );
    settings.triple.enabled = true;
    assert_eq!(
        testing::game_lists::set_camera_fov(&mut settings, 75.0).0,
        "triple_fov_deg"
    );
    assert_eq!(settings.fov, 50.0);
    assert_eq!(settings.triple.fov_deg, 75.0);
    testing::game_lists::set_camera_fov(&mut settings, 0.0);
    assert_eq!(settings.triple.fov_deg, 0.0);
}
/// The game menu offers the 16x anisotropic filtering the launcher does (#669): set
/// there, it showed as a bare "16" here and could not be chosen again.
#[test]
fn anisotropy_zero_shows_as_off() {
    let file = serde_json::json!({ "anisotropy": 0 });
    let (options, at, _) = testing::game_lists::select_state(&file, "anisotropy");
    assert_eq!(at.map(|i| options[i].1), Some("Off"));
}

#[test]
fn sixteen_x_anisotropy_can_be_chosen_in_the_game_menu() {
    let file = serde_json::json!({ "anisotropy": 16 });
    let (options, at, _) = testing::game_lists::select_state(&file, "anisotropy");
    assert_eq!(at.map(|i| options[i]), Some(("16", "16x")));
}

#[test]
fn lines_sort_as_numbers() {
    let mut v = vec!["13N", "5", "137", "N30", "92"];
    v.sort_by(|a, b| testing::game_lists::natural(a, b));
    assert_eq!(v, vec!["5", "13N", "92", "137", "N30"]);
}

#[test]
fn symbols_are_not_forced_through_numeric_ibis_lines() {
    assert!(testing::game_lists::numeric_ibis_line("10"));
    assert!(testing::game_lists::numeric_ibis_line("10E"));
    assert!(testing::game_lists::numeric_ibis_line("X10"));
    assert!(!testing::game_lists::numeric_ibis_line("-10"));
    assert!(!testing::game_lists::numeric_ibis_line("10-"));
    assert!(!testing::game_lists::numeric_ibis_line("EXP"));
}
