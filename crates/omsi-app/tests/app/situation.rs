use clap::Parser;
use openomsi_game::testing;
use openomsi_game::testing::core::cli::Args;
use openomsi_game::testing::situation::*;
use std::path::PathBuf;

/// A vehicle that fell through the map is neither saved nor spawned at its fallen height.
#[test]
fn a_fallen_height_is_not_a_spawn_height() {
    assert!(plausible_height(0.0) && plausible_height(-400.0) && plausible_height(2500.0));
    for z in [
        -119_841.0,
        -2_470_000.0,
        f64::NAN,
        f64::NEG_INFINITY,
        f64::INFINITY,
    ] {
        assert!(!plausible_height(z), "{z}");
    }
    assert_eq!(spawn_arg(1.0, 2.0, 90.0, 12.5), "1,2,90,12.5");
    assert_eq!(spawn_arg(1.0, 2.0, 90.0, -2_470_000.0), "1,2,90");
    assert_eq!(spawn_arg(1.0, 2.0, 90.0, f64::NAN), "1,2,90");
    assert_eq!(spawn_without_height("1,2,90"), Some((1.0, 2.0)));
    assert_eq!(spawn_without_height("1,2,90,12.5"), None);
}

#[test]
fn saved_odometer_prefers_the_header_and_recovers_legacy_variables() {
    use omsi_content::situation::SituationVehicle;
    let mut v = SituationVehicle {
        odometer_km: 75556.639,
        vars: vec![
            ("KMCOUNTER_KM".into(), 12345.0),
            ("kmcounter_m".into(), 678.0),
        ],
        ..Default::default()
    };
    assert_eq!(saved_odometer_km(&v, false), Some(75556.639));
    for placeholder in [1.0, 2.0, 8.0, 9.0, 10.0] {
        v.odometer_km = placeholder;
        assert_eq!(saved_odometer_km(&v, true), Some(12345.678));
    }
    v.odometer_km = 1.0;
    assert_eq!(saved_odometer_km(&v, false), Some(12345.678));
    v.vars.clear();
    assert_eq!(saved_odometer_km(&v, true), None);
    for actual in [8.0, 8.5, 9.9] {
        v.odometer_km = actual;
        assert_eq!(saved_odometer_km(&v, false), Some(actual));
    }
    v.odometer_km = f64::NAN;
    v.vars = vec![("kmcounter_km".into(), f64::INFINITY)];
    assert_eq!(saved_odometer_km(&v, false), None);
}

#[test]
fn legacy_odometer_ids_fall_back_for_player_and_placed_buses() {
    use omsi_content::situation::{Situation, SituationVehicle};
    let mut sit = Situation {
        description: "Saved by the openOMSI at 08:45".into(),
        vehicles: (1..=10)
            .map(|id| SituationVehicle {
                odometer_km: id as f64,
                is_my_vehicle: id == 1,
                vars: vec![
                    ("kmcounter_km".into(), 12000.0 + id as f64),
                    ("kmcounter_m".into(), 125.0),
                ],
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };
    let mut args = Args::parse_from(["depot"]);
    apply_situation_parsed(&sit, &mut args);
    assert_eq!(args.situation_odometer_km, Some(12001.125));
    assert_eq!(args.situation_others[0].odometer_km, Some(12002.125));
    assert_eq!(args.situation_others[8].odometer_km, Some(12010.125));
    sit.vehicles[0].vars.clear();
    apply_situation_parsed(&sit, &mut args);
    assert_eq!(args.situation_odometer_km, None);
}

#[test]
fn situation_others_preserve_their_colorscheme() {
    let sit = omsi_content::situation::Situation {
        map: "maps/Berlin/global.cfg".into(),
        vehicles: vec![
            omsi_content::situation::SituationVehicle {
                file: "Vehicles/MAN_SD200/MAN_SD77.bus".into(),
                is_my_vehicle: true,
                odometer_km: 10001.125,
                vars: vec![("Colorscheme".into(), 1.0)],
                ..Default::default()
            },
            omsi_content::situation::SituationVehicle {
                file: "Vehicles/MAN_NL_NG/MAN_EN92.bus".into(),
                is_my_vehicle: false,
                odometer_km: 20002.375,
                vars: vec![("Colorscheme".into(), 4.0)],
                string_vars: vec![("destination".into(), "  Manual destination  ".into())],
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let mut args = testing::core::cli::Args::parse_from(["depot"]);
    apply_situation_parsed(&sit, &mut args);
    assert_eq!(args.paint.as_deref(), Some("1"));
    assert_eq!(args.situation_odometer_km, Some(10001.125));
    assert_eq!(args.situation_others.len(), 1);
    assert_eq!(args.situation_others[0].paint.as_deref(), Some("4"));
    assert_eq!(args.situation_others[0].odometer_km, Some(20002.375));
    assert_eq!(
        args.situation_others[0].strvars,
        vec![("destination".into(), "  Manual destination  ".into())]
    );
}

/// #653: a saved duty goes on at the trip of the tour it was saved on, with the rest of
/// the tour after it.
#[test]
fn a_saved_duty_goes_on_at_its_trip() {
    let sit = omsi_content::situation::Situation {
        map: "maps/Berlin/global.cfg".into(),
        vehicles: vec![omsi_content::situation::SituationVehicle {
            file: "Vehicles/MAN_SD200/MAN_SD77.bus".into(),
            is_my_vehicle: true,
            timetable: ["137", "4", "3", "2", "0", "0"].map(String::from).to_vec(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut args = testing::core::cli::Args::parse_from(["depot"]);
    apply_situation_parsed(&sit, &mut args);
    assert_eq!(
        (
            args.line.as_deref(),
            args.tour.as_deref(),
            args.trip.as_deref()
        ),
        (Some("137"), Some("4"), Some("4"))
    );
    assert!(args.whole_tour);
    assert_eq!(args.situation_next_stop, Some(2));
}

#[test]
fn legacy_and_invalid_saved_stop_fields_keep_legacy_selection() {
    let mut args = testing::core::cli::Args::parse_from(["depot"]);
    for fields in [
        vec!["109", "65104", "16"],
        vec!["109", "65104", "16", "-1"],
        vec!["109", "65104", "16", "invalid"],
        vec![],
    ] {
        args.situation_next_stop = Some(9);
        let sit = omsi_content::situation::Situation {
            vehicles: vec![omsi_content::situation::SituationVehicle {
                is_my_vehicle: true,
                timetable: fields.into_iter().map(String::from).collect(),
                ..Default::default()
            }],
            ..Default::default()
        };
        apply_situation_parsed(&sit, &mut args);
        assert_eq!(args.situation_next_stop, None);
    }
}

#[test]
fn a_string_only_snapshot_is_a_resume() {
    let mut args = testing::core::cli::Args::parse_from(["depot"]);
    assert!(!args.is_resuming());
    args.situation_strvars
        .push(("destination".into(), "Manual".into()));
    assert!(args.is_resuming());
}

#[test]
fn an_odometer_only_snapshot_is_a_resume() {
    let mut args = Args::parse_from(["depot"]);
    args.situation_odometer_km = Some(75556.639);
    assert!(args.is_resuming());
}

/// A vehicle with `[scripttexture]`s of these sizes and no scripts.
fn with_script_textures(sizes: &[(i32, i32)]) -> omsi_sim::VehicleInstance {
    let mut model = omsi_model::Model::default();
    model.script_textures = sizes.to_vec();
    let ty = std::sync::Arc::new(omsi_sim::VehicleType {
        def: Default::default(),
        model,
        model_dir: PathBuf::new(),
        program: Default::default(),
        meshes: Vec::new(),
        paint_schemes: Vec::new(),
        texchanges: Vec::new(),
        wheel_meshes: Vec::new(),
        suspension_axles: Vec::new(),
        missing_packs: Vec::new(),
        mesh_bounds: Vec::new(),
        mesh_boxes: Vec::new(),
    });
    omsi_sim::VehicleInstance::new(ty, omsi_sim::VehicleHost::new(Default::default()))
}

/// #34: what the scripts drew (an IBIS, a destination sign) comes back with the
/// situation, in OMSI's files next to it, for each vehicle saved; a missing or damaged
/// file leaves its texture empty.
#[test]
fn script_textures_come_back_with_the_situation() {
    let dir = std::env::temp_dir().join(format!("depot_sit_scripttex_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let osn = dir.join("Slot 1.osn");
    let mut player = with_script_textures(&[(4, 2), (8, 8)]);
    let mut placed = with_script_textures(&[(2, 2)]);
    player.host.script_textures[0].color = [200, 100, 50, 255];
    player.host.script_textures[0].rect(1, 0, 2, 1);
    placed.host.script_textures[0].color = [1, 2, 3, 4];
    placed.host.script_textures[0].rect(0, 0, 1, 1);
    let drawn = player.host.script_textures[0].rgba.clone();
    // a file of an earlier save under the same name, for a texture this save has not
    std::fs::write(dir.join("Slot 1.osn_0_7.dds"), b"old").unwrap();
    assert_eq!(save_script_textures(&osn, &[&player, &placed]), 3);
    assert!(dir.join("Slot 1.osn_0_0.dds").is_file());
    assert!(dir.join("Slot 1.osn_1_0.dds").is_file());
    assert!(!dir.join("Slot 1.osn_0_7.dds").exists());

    // the situation as read back: each vehicle finds its own files
    let mut args = Args::parse_from(["depot"]);
    let sit = omsi_content::situation::Situation {
        path: osn.clone(),
        vehicles: vec![
            omsi_content::situation::SituationVehicle {
                is_my_vehicle: true,
                ..Default::default()
            },
            omsi_content::situation::SituationVehicle::default(),
        ],
        ..Default::default()
    };
    apply_situation_parsed(&sit, &mut args);
    let mine = args.situation_script_textures.clone().unwrap();
    let other = args.situation_others[0].script_textures.clone().unwrap();

    let mut back = with_script_textures(&[(4, 2), (8, 8)]);
    assert_eq!(restore_script_textures(&mut back, &mine), 2);
    assert_eq!(back.host.script_textures[0].rgba, drawn);
    assert!(
        back.host.script_textures[0].take_upload().is_some(),
        "goes up to the GPU"
    );
    let mut back_placed = with_script_textures(&[(2, 2)]);
    assert_eq!(restore_script_textures(&mut back_placed, &other), 1);
    assert_eq!(back_placed.host.script_textures[0].get(1, 1), [1, 2, 3, 4]);

    // a damaged file: an empty texture and a line in the log, no panic
    std::fs::write(dir.join("Slot 1.osn_0_0.dds"), b"DDS broken").unwrap();
    std::fs::remove_file(dir.join("Slot 1.osn_0_1.dds")).unwrap();
    let mut damaged = with_script_textures(&[(4, 2), (8, 8)]);
    damaged.host.script_textures[0].color = [9, 9, 9, 9];
    damaged.host.script_textures[0].rect(0, 0, 3, 1);
    assert_eq!(restore_script_textures(&mut damaged, &mine), 0);
    assert!(damaged.host.script_textures[0].rgba.iter().all(|b| *b == 0));
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A buffer the script keeps locked saves the image it last released, not its
/// half-drawn next one.
#[test]
fn a_locked_texture_saves_what_it_shows() {
    let mut st = omsi_sim::scripttex::ScriptTexture::new(2, 1);
    st.put(0, 0, [1, 1, 1, 1]);
    st.unlock();
    st.locked = true;
    st.put(1, 0, [2, 2, 2, 2]);
    assert_eq!(st.shown(), &[1, 1, 1, 1, 0, 0, 0, 0]);
}
