mod authored_station;

use hashbrown::HashMap;
use omsi_sim::traffic::LaneKey;
use openomsi_game::testing::script_test_vehicle;

#[test]
fn off_centre_bounding_boxes_align_the_physical_flank_on_either_side() {
    let mut bus = script_test_vehicle("{frame}\n{end}\n", "", "");
    let ty = std::sync::Arc::get_mut(&mut bus.ty).unwrap();
    ty.def.bounding_box = Some([2.5, 12.0, 3.0, 0.6, 0.0, 1.5]);
    let right = bay_for(4.0, ty, false, false, 0.0);
    assert!((right + 0.6 + 1.25 - 4.3).abs() < 1e-5);
    let left = bay_for(-4.0, ty, false, false, 1.0);
    assert!((left + 0.6 - 1.25 + 4.3).abs() < 1e-5);
    assert_eq!(bay_for(4.0, ty, true, false, 0.0), 0.0);
}

#[test]
fn bay_alignment_follows_the_platform_side_without_moving_rail_vehicles() {
    let bus = script_test_vehicle("{frame}\n{end}\n", "", "");
    let ty = &bus.ty;
    assert!((bay_for(4.0, ty, false, false, 0.0) - 3.05).abs() < 1e-5);
    assert!((bay_for(-4.0, ty, false, false, 1.0) + 3.05).abs() < 1e-5);
    assert!((bay_for(-4.0, ty, false, true, 0.0) + 3.05).abs() < 1e-5);
    assert!((bay_for(4.0, ty, false, true, 1.0) - 3.05).abs() < 1e-5);
    for hand in [false, true] {
        assert!((bay_for(-4.0, ty, false, hand, 2.0) + 3.05).abs() < 1e-5);
        assert!((bay_for(4.0, ty, false, hand, 2.0) - 3.05).abs() < 1e-5);
        for side in [0.0, 1.0, 2.0] {
            assert_eq!(bay_for(-4.0, ty, true, hand, side), 0.0);
            assert_eq!(bay_for(f32::NAN, ty, false, hand, side), 0.0);
        }
    }
}

use openomsi_game::testing;
use openomsi_game::testing::schedule::*;

/// The row OMSI's AI bus is given: the first whose ident is the destination, whatever
/// the codes' order; of equally loose matches the first as well.
#[test]
fn a_tour_bus_takes_a_trip_on_only_at_its_start() {
    let route = [10, 11, 12, 13, 14, 15, 16];
    assert_eq!(tour_entry(&route, 10), Some(0));
    assert_eq!(tour_entry(&route, 13), Some(3));
    // the same trip again: the bus stands on its last lane
    assert_eq!(tour_entry(&route, 16), None);
    assert_eq!(tour_entry(&route, 99), None);
}

#[test]
fn a_stop_is_no_target_of_itself() {
    // a circular line: from A round to A; B has two platforms of one name
    let names = |id: i64| match id {
        1 => "A".to_string(),
        2 | 3 => "B".to_string(),
        _ => "C".to_string(),
    };
    let t = station_targets([(vec![1, 2, 4, 3, 1], "A".to_string())].into_iter(), names);
    let of = |id: i64| t[&id].iter().map(|x| x.0.as_str()).collect::<Vec<_>>();
    assert_eq!(of(1), ["B", "C"]);
    assert_eq!(of(2), ["C", "A"]);
    assert_eq!(of(4), ["B", "A"]);
    assert_eq!(of(3), ["A"]);
    assert!(t[&1].iter().all(|x| x.1.contains("A")));
}

#[test]
fn a_terminus_is_the_first_row_of_its_name() {
    let t = |code: i32, id: &str, s: &[&str]| omsi_vehicle::hof::Terminus {
        code,
        texture_id: id.into(),
        terminus_stop: Some(id.into()),
        all_exit: false,
        strings: s.iter().map(|x| x.to_string()).collect(),
    };
    let hof = omsi_vehicle::Hof {
        termini: vec![
            t(0, "Depot", &[]),
            t(3, "61-Other", &["61-MaoFangChang"]),
            t(4, "61-Third", &[]),
            t(1, "61-MaoFangChang", &["61-MaoFangChang"]),
            t(2, "Wickenberg Nord", &[]),
        ],
        ..Default::default()
    };
    assert_eq!(
        testing::schedule::find_terminus(&hof, "61-MaoFangChang"),
        Some(3)
    );
    assert_eq!(
        testing::schedule::find_terminus(&hof, "  61-MaoFangChang "),
        Some(3)
    );
    let hof = omsi_vehicle::Hof {
        termini: vec![t(0, "A", &["Wickenberg"]), t(1, "B", &["Wickenberg"])],
        ..Default::default()
    };
    assert_eq!(
        testing::schedule::find_terminus(&hof, "wickenberg"),
        Some(0)
    );
}

/// By its sign text a row is the one whose sign reads so, not an earlier one whose
/// second line does (Spandau's Machandelweg, district RUHLEBEN, before U Ruhleben).
#[test]
fn a_terminus_is_found_by_its_own_sign_before_another_signs_second_line() {
    let t = |code: i32, id: &str, s: &[&str]| omsi_vehicle::hof::Terminus {
        code,
        texture_id: id.into(),
        terminus_stop: Some(id.into()),
        strings: s.iter().map(|x| x.to_string()).collect(),
        ..Default::default()
    };
    let hof = omsi_vehicle::Hof {
        termini: vec![
            t(0, "Empty", &[""]),
            t(
                194,
                "Machandelweg",
                &["MACHANDELWEG", "RUHLEBEN", "MACHANDELWEG"],
            ),
            t(282, "U Ruhleben", &["RUHLEBEN", "U-BAHNHOF"]),
            t(5, "Boerse", &["BOERSE", "RATHAUSMARKT"]),
            t(555, "RMarkt", &["", "RATHAUSMARKT"]),
        ],
        ..Default::default()
    };
    assert_eq!(testing::schedule::find_terminus(&hof, "RUHLEBEN"), Some(2));
    assert_eq!(testing::schedule::find_terminus(&hof, "Ruhleben"), Some(2));
    // (a sign whose first line is blank goes by the next)
    assert_eq!(
        testing::schedule::find_terminus(&hof, "RATHAUSMARKT"),
        Some(4)
    );
    // the ident first, as before
    assert_eq!(
        testing::schedule::find_terminus(&hof, "Machandelweg"),
        Some(1)
    );
    // another bus showing it (a LAN player's: its sign text): the same row
    let mut v = ibis_test_vehicle();
    set_ai_destination(&mut v, Some(&hof), "5", "RUHLEBEN", &[]);
    assert_eq!(v.var("IBIS_TerminusCode"), Some(282.0));
    // every row of the stock Spandau and Grundorf depot files by its sign text
    for file in ["Spandau 86.hof", "Grundorf.hof"] {
        let path = std::path::Path::new("../../../OMSI 2 Original/Vehicles/MAN_SD200").join(file);
        let Ok(hof) = omsi_vehicle::Hof::load(&path) else {
            continue;
        };
        for (i, row) in hof.termini.iter().enumerate() {
            let Some(sign) = row.strings.iter().find(|s| !s.trim().is_empty()) else {
                continue;
            };
            let found = testing::schedule::find_terminus(&hof, sign).map(|k| hof.termini[k].code);
            assert_eq!(
                found,
                Some(row.code),
                "{file} row {i} {} '{}'",
                row.code,
                sign.trim()
            );
        }
    }
}

#[test]
fn complex_line_keeps_letter_suffix() {
    assert_eq!(complex_line_text("5", 5.0), "005  ");
    assert_eq!(complex_line_text("5E", 5.0), "   5E");
    assert_eq!(line_suffix_from_text("5E"), 10);
    assert_eq!(line_suffix_from_text("5N"), 4);
    assert_eq!(line_suffix_from_text("5S"), 23);
    assert_eq!(line_code_from_text("5E", Some(505)), Some(510));
    assert_eq!(line_code_from_text("5", Some(505)), Some(500));
}

/// #459: a four-digit line keeps its number and gets no suffix from its route code.
#[test]
fn four_digit_line_keeps_its_number() {
    assert_eq!(line_code_from_text("7110", Some(711001)), Some(711000));
    assert_eq!(line_code_from_text("7110", None), Some(711000));
    assert_eq!(line_code_from_text("7110-10", Some(711010)), Some(711000));
    assert_eq!(line_code_from_text("1234E", None), Some(123410));
    assert_eq!(complex_line_text("7110", 7110.0), "7110  ");
}

/// #546: a letter-first line had no number, and the DL05's matrix blanks line 0.
#[test]
fn line_with_letter_prefix_keeps_its_number() {
    assert_eq!(line_code_from_text("X10", None), Some(1036));
    assert_eq!(line_code_from_text("X10", Some(51001)), Some(51036));
    assert_eq!(line_code_from_text("M41", Some(4101)), Some(4128));
    assert_eq!(line_code_from_text("N9", None), Some(935));
    assert_eq!(line_code_from_text("TML", Some(7601)), Some(7601));
    assert_eq!(line_suffix_from_text("X10"), 36);
    assert_eq!(line_number_digits("X10"), "10");
    assert_eq!(line_number_digits("5E"), "5");
}

#[test]
fn berlin_5e_uses_its_real_terminus_when_no_hof_route_exists() {
    let path = std::path::Path::new("../../../OMSI 2 Original/Vehicles/MAN_SD202/Berlin.hof");
    let Ok(hof) = omsi_vehicle::Hof::load(path) else {
        return;
    };
    let target = ibis_target(&hof, "5E", "Fernbahnhof Spandau", &[], None).expect("5E target");
    assert_eq!(target.terminus_code, Some(232));
    assert_eq!(
        target.terminus_index,
        hof.termini.iter().position(|t| t.code == 232).unwrap() as i32
    );
    let target = ibis_target(&hof, "5E", "Spektefeld Schulzentrum", &[], None)
        .expect("5E shortened HOF target");
    assert_eq!(target.terminus_code, Some(233));
}

#[test]
fn berlin_5e_does_not_turn_hof_route_505_into_s5() {
    let path =
        std::path::Path::new("../../../OMSI 2 Original/Vehicles/MAN_NL_NG/Spandau 89-11.hof");
    let Ok(hof) = omsi_vehicle::Hof::load(path) else {
        return;
    };
    let target = ibis_target(&hof, "5E", "Nervenklinik", &["U Rathaus Spandau"], None)
        .expect("5E route target");
    assert_eq!(target.route, Some(3));
    assert_eq!(target.suffix, 10);
}

#[test]
fn stop_names_meet_in_any_order_and_spelling() {
    assert_eq!(stop_words("Nordstadt Bhf"), stop_words("Bhf. Nordstadt"));
    assert_eq!(stop_words("F_Kirchweg"), stop_words("Kirchweg"));
    assert_ne!(stop_words("Bhf Nordstadt"), stop_words("Nordstadt"));
    assert!(stop_words("").is_empty());
}

/// A made-up line 7 with six routes to Hafen, each telling one rule of `pick_route`.
fn hafen_depot() -> omsi_vehicle::Hof {
    let mut hof = omsi_vehicle::Hof {
        termini: vec![omsi_vehicle::hof::Terminus {
            code: 100,
            strings: vec!["Hafen".into()],
            ..Default::default()
        }],
        ..Default::default()
    };
    let routes: [(&str, &[&str]); 6] = [
        ("707", &["Markt", "Schule", "Park", "Ufer", "Hafen"]),
        ("701", &["Markt", "Schule", "Park", "Hafen"]),
        (
            "702",
            &[
                "Bhf Nordstadt",
                "F_Kirchweg",
                "Markt",
                "Schule",
                "Park",
                "Hafen",
            ],
        ),
        ("703", &["Schule", "Park", "Hafen"]),
        (
            "705",
            &[
                "Bhf Nordstadt",
                "Markt",
                "Rathaus",
                "Schule",
                "Park",
                "Hafen",
            ],
        ),
        (
            "706",
            &["Am Wald", "Kirchweg", "Markt", "Schule", "Park", "Hafen"],
        ),
    ];
    for (code, stops) in routes {
        hof.info_trips.push(omsi_vehicle::hof::InfoTrip {
            code: code.into(),
            route: "100".into(),
            line: "7".into(),
            ..Default::default()
        });
        hof.info_busstop_lists
            .push(stops.iter().map(|s| s.to_string()).collect());
    }
    hof
}

#[test]
fn the_route_follows_the_trips_stops() {
    let hof = hafen_depot();
    let route = |stops: &[&str]| {
        ibis_target(&hof, "7", "Hafen", stops, None)
            .expect("line 7 target")
            .route
    };
    // the depot file spells the first stop another way, and with a one-letter prefix
    assert_eq!(
        route(&[
            "Nordstadt Bhf",
            "Kirchweg",
            "Markt",
            "Schule",
            "Park",
            "Hafen"
        ]),
        Some(2)
    );
    // a short working gets its own route, not the long ones it is part of
    assert_eq!(route(&["Schule", "Park", "Hafen"]), Some(3));
    // of two routes from the trip's first stop, the one as long as the trip
    assert_eq!(route(&["Markt", "Schule", "Park", "Hafen"]), Some(1));
    // starting at the trip's first stop counts before one stop more of the trip: 705
    // has Rathaus too but begins at Bhf Nordstadt, so a route from Markt (707, as
    // long as the trip) is taken
    assert_eq!(
        route(&["Markt", "Rathaus", "Schule", "Park", "Hafen"]),
        Some(7)
    );
    // nothing known of the trip: the first route, as before
    assert_eq!(route(&[]), Some(7));
}

#[test]
fn the_ibis_stands_at_the_trips_first_stop_on_a_route_that_begins_before_it() {
    let hof = hafen_depot();
    // no route begins at Kirchweg; 702 and 706 have the trip's stops after one more
    let stops = ["Kirchweg", "Markt", "Schule", "Park", "Hafen"];
    let target =
        ibis_target(&hof, "7", "Hafen", &stops, Some((0, stops[0]))).expect("line 7 target");
    assert_eq!(target.route, Some(2));
    assert_eq!(target.stop, 1);
}

#[test]
fn where_a_bus_is_on_a_partly_loaded_route() {
    let key = |id: i64| {
        Some(LaneKey {
            tile: (0, 0),
            id,
            path: 0,
        })
    };
    let legs = [0, 0, 1, 1, 1, 2];
    let steps: Vec<Step> = legs
        .iter()
        .enumerate()
        .map(|(i, &leg)| Step {
            key: key(i as i64),
            leg,
            length: 0.0,
        })
        .collect();
    let slots = [
        Slot::Lane(10),
        Slot::Lane(11),
        Slot::Lane(12),
        Slot::Waiting,
        Slot::Lane(14),
        Slot::Absent,
    ];
    let est = [100.0, 100.0, 50.0, 70.0, 50.0, 0.0];
    // a layover bus stands at the start
    assert_eq!(step_at(&steps, &slots, &est, 0, 0.0), Some((0, 0.0)));
    // 17 m into leg 1: on its first lane, whose part of the route ends at the gap
    let (at, off) = step_at(&steps, &slots, &est, 1, 0.1).unwrap();
    assert!(at == 2 && (off - 17.0).abs() < 1e-9, "{at} {off}");
    assert_eq!(section_around(&slots, 2), (0, 3));
    // half way: on the step still to come - the bus has to wait
    assert_eq!(step_at(&steps, &slots, &est, 1, 0.5), Some((3, 35.0)));
    // near the end of the leg: after the gap
    let (at, off) = step_at(&steps, &slots, &est, 1, 0.9).unwrap();
    assert_eq!(at, 4);
    assert!((off - 33.0).abs() < 1e-9);
    assert_eq!(section_around(&slots, 4), (4, 6));
    // a leg of absent steps only, and a leg without steps: past the end
    assert_eq!(step_at(&steps, &slots, &est, 2, 0.5), None);
    assert_eq!(step_at(&steps, &slots, &est, 3, 0.5), None);
    // a leg without a station link: at the start of the next leg
    let steps2: Vec<Step> = [0, 2, 2]
        .iter()
        .enumerate()
        .map(|(i, &leg)| Step {
            key: key(i as i64),
            leg,
            length: 0.0,
        })
        .collect();
    let slots2 = [Slot::Lane(1), Slot::Absent, Slot::Lane(3)];
    assert_eq!(
        step_at(&steps2, &slots2, &[10.0, 0.0, 10.0], 1, 0.5),
        Some((2, 0.0))
    );
    assert_eq!(section_around(&slots2, 2), (0, 3));
}

fn link(a: i64, b: i64) -> Option<f64> {
    // 100 m between neighbours, 300 m from 3 to 4
    Some(if (a, b) == (3, 4) { 300.0 } else { 100.0 })
}

#[test]
fn trip_times_from_the_profile() {
    let stations = [1, 2, 3, 4, 5];
    // no manual times: the duration split by the link lengths
    let p = omsi_timetable::TripProfile {
        name: "p".into(),
        factor: 10.0,
        ..Default::default()
    };
    let t = TripTimes::new(&stations, Some(&p), &link);
    let arr: Vec<f64> = t.stations.iter().map(|s| s.0).collect();
    assert_eq!(arr, vec![0.0, 100.0, 200.0, 500.0, 600.0]);
    assert_eq!(t.duration, 600.0);
    // manual minutes win, the rest in between by length; a passed station stops nowhere
    let p = omsi_timetable::TripProfile {
        name: "p".into(),
        factor: 10.0,
        man_dep_time: vec![(0, 1.0), (1, 2.0)],
        man_arr_time: vec![(3, 6.0), (4, 9.0)],
        other_stopping: vec![(2, 2)],
    };
    let t = TripTimes::new(&stations, Some(&p), &link);
    assert_eq!(t.stations[0], (60.0, 60.0));
    assert_eq!(t.stations[1], (120.0, 120.0));
    // station 2 lies 100 m of the 400 m between the departure at 2 min and the arrival at 6
    assert!((t.stations[2].0 - 180.0).abs() < 1e-9, "{:?}", t.stations);
    assert_eq!(t.stations[3], (360.0, 360.0));
    assert_eq!(t.duration, 540.0);
    assert_eq!(t.stops, vec![true, true, false, true, true]);
    // a profile without stations keeps its duration (flights on a track)
    assert_eq!(
        TripTimes::new(
            &[],
            Some(&omsi_timetable::TripProfile {
                factor: 25.0,
                ..Default::default()
            }),
            &link
        )
        .duration,
        1500.0
    );
}

fn planned(departure: f64, stops: &[(f64, f64, f64)]) -> PlannedTrip {
    let stops: Vec<PlannedStop> = stops
        .iter()
        .enumerate()
        .map(|(i, &(x, arr, dep))| PlannedStop {
            object_id: i as i64,
            name: format!("s{i}"),
            arr,
            dep,
            position: Some(glam::DVec3::new(x, 0.0, 0.0)),
            dir: StopDir::default(),
            stops: true,
        })
        .collect();
    PlannedTrip {
        name: format!("t{departure}"),
        line: "5".into(),
        terminus: "T".into(),
        departure,
        end: stops.last().unwrap().arr,
        stops,
    }
}

/// A vehicle whose script drops its duty and destination while it has no timetable,
/// and shows what the timetable callbacks tell it.
fn timetable_test_vehicle() -> omsi_sim::VehicleInstance {
    timetable_test_vehicle_with_door("door_0")
}

fn timetable_test_vehicle_with_door(door: &str) -> omsi_sim::VehicleInstance {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "omsi_tt_restore_{}_{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("device.osc");
    let vars = dir.join("vars.txt");
    let strings = dir.join("strings.txt");
    std::fs::write(
        &vars,
        format!("duty\nobserved_stop\nobserved_delay\n{door}\n"),
    )
    .unwrap();
    std::fs::write(&strings, "destination\nobserved_line\n").unwrap();
    std::fs::write(
        &script,
        r#"
{frame}
(L.L.schedule_active) ! (M.V.GetTTBusstopCount) 0 = ||
{if}
0 (S.L.duty)
"" (S.$.destination)
{endif}
(M.V.GetTTLineString) (S.$.observed_line)
(M.V.GetTTBusstopIndex) (S.L.observed_stop)
(M.V.GetTTDelay) (S.L.observed_delay)
{end}
"#,
    )
    .unwrap();
    let program = omsi_script::compile(&omsi_script::CompileInput {
        scripts: vec![script],
        varlists: vec![vars],
        stringvarlists: vec![strings],
        builtin_vars: vec!["schedule_active".into()],
        ..Default::default()
    });
    assert!(program.errors.is_empty(), "{:?}", program.errors);
    let ty = Arc::new(omsi_sim::VehicleType {
        def: Default::default(),
        model: Default::default(),
        model_dir: dir.clone(),
        program: Arc::new(program),
        meshes: Vec::new(),
        paint_schemes: Vec::new(),
        texchanges: Vec::new(),
        wheel_meshes: Vec::new(),
        suspension_axles: Vec::new(),
        missing_packs: Vec::new(),
        mesh_bounds: Vec::new(),
        mesh_boxes: Vec::new(),
    });
    std::fs::remove_dir_all(dir).unwrap();
    omsi_sim::VehicleInstance::new(ty, omsi_sim::VehicleHost::new(Default::default()))
}

/// A vehicle that declares the IBIS variables a destination is written to.
fn ibis_test_vehicle() -> omsi_sim::VehicleInstance {
    script_test_vehicle(
        "{frame}\n{end}\n",
        "IBIS_LinieKurs\nIBIS_TerminusIndex\nIBIS_TerminusCode\n",
        "IBIS_terminus_name\n",
    )
}

/// #738: of four destinations of one name the one picked from the list is set, not
/// the first of that name.
#[test]
fn a_destination_picked_from_the_list_is_that_one_of_its_name() {
    let t = |code: i32, id: &str| omsi_vehicle::hof::Terminus {
        code,
        texture_id: id.into(),
        strings: vec!["ul. Xutorskaya".into()],
        ..Default::default()
    };
    let hof = omsi_vehicle::Hof {
        termini: vec![
            t(92, "Xut_92"),
            t(120, "Xut_120"),
            t(123, "Xut_123"),
            t(124, "Xut_124"),
        ],
        ..Default::default()
    };
    let mut v = ibis_test_vehicle();
    set_player_destination_at(&mut v, &hof, "39", 1, &[]);
    assert_eq!(v.var("IBIS_TerminusCode"), Some(120.0));
    assert_eq!(v.var("IBIS_TerminusIndex"), Some(1.0));
    assert_eq!(v.var("IBIS_LinieKurs"), Some(39.0));
    assert_eq!(v.str_var("IBIS_terminus_name"), "ul. Xutorskaya");
    set_player_destination_at(&mut v, &hof, "39", 3, &[]);
    assert_eq!(v.var("IBIS_TerminusCode"), Some(124.0));
    // (by its name it is the first of them, as the list used to set it)
    set_player_destination_directly(&mut v, Some(&hof), "39", "ul. Xutorskaya", &[]);
    assert_eq!(v.var("IBIS_TerminusCode"), Some(92.0));
}

/// A destination picked from the list (or a route number) turns a hand-cranked roller
/// blind to its row, as OMSI 2's line and destination dialog does: the AI's way, the
/// stock blinds' ai_scheduled_settarget taking AI_target_index for the blind's place.
/// On a bus not switched on yet (#1098: the SD77 is put on the road cold) once the main
/// switch is on and the start-up is done.
#[test]
fn a_destination_picked_from_the_list_turns_the_roller_blind() {
    let osc = "{trigger:rollband_sync}\n{end}\n{trigger:ai_scheduled_settarget}\n(L.L.AI_target_index) (S.L.rlbnd_ziel_target)\n(L.$.SetLineTo) (S.$.rlbnd_line)\n{end}\n";
    let vars = "IBIS_LinieKurs\nIBIS_TerminusIndex\nIBIS_TerminusCode\nAI_target_index\nrlbnd_ziel_target\nelec_busbar_main_sw\n";
    let t = |code: i32, id: &str| omsi_vehicle::hof::Terminus {
        code,
        texture_id: id.into(),
        strings: vec![id.to_uppercase()],
        ..Default::default()
    };
    let hof = omsi_vehicle::Hof {
        termini: vec![
            t(0, "Empty"),
            t(205, "U Ruhleben"),
            t(154, "U Rathaus Spandau"),
        ],
        ..Default::default()
    };
    let mut v = script_test_vehicle(osc, vars, "SetLineTo\nrlbnd_line\nIBIS_terminus_name\n");
    v.set_var("elec_busbar_main_sw", 1.0);
    let mut pick = set_player_destination_at(&mut v, &hof, "145", 2, &[]);
    turn_roller_blind(&mut v, &mut pick, false);
    assert_eq!(pick, None);
    assert_eq!(v.var("rlbnd_ziel_target"), Some(2.0));
    assert_eq!(v.str_var("rlbnd_line"), "145");
    // (and the IBIS as before)
    assert_eq!(v.var("IBIS_TerminusCode"), Some(154.0));
    // with the main switch off the pick waits for it (the stock trigger switches it on
    // itself, and the start-up's toggle then switched it off again), and for the start-up
    v.set_var("elec_busbar_main_sw", 0.0);
    let mut pick = set_player_destination_at(&mut v, &hof, "145", 1, &[]);
    turn_roller_blind(&mut v, &mut pick, false);
    assert_eq!(v.var("rlbnd_ziel_target"), Some(2.0));
    assert_eq!(v.var("IBIS_TerminusCode"), Some(205.0));
    v.set_var("elec_busbar_main_sw", 1.0);
    turn_roller_blind(&mut v, &mut pick, true);
    assert_eq!(v.var("rlbnd_ziel_target"), Some(2.0));
    turn_roller_blind(&mut v, &mut pick, false);
    assert_eq!(v.var("rlbnd_ziel_target"), Some(1.0));
    assert_eq!(pick, None);
    // a bus without a roller blind has nothing to turn
    let mut ibis = ibis_test_vehicle();
    assert_eq!(
        set_player_destination_at(&mut ibis, &hof, "145", 1, &[]),
        None
    );
}

#[test]
fn a_destination_picked_from_the_list_updates_display_target() {
    let osc =
        "{trigger:ai_scheduled_settarget}\n(L.L.AI_target_index) (S.L.display_target)\n{end}\n";
    let vars =
        "IBIS_LinieKurs\nIBIS_TerminusIndex\nIBIS_TerminusCode\nAI_target_index\ndisplay_target\n";
    let t = |code: i32, id: &str| omsi_vehicle::hof::Terminus {
        code,
        texture_id: id.into(),
        strings: vec![id.into()],
        ..Default::default()
    };
    let hof = omsi_vehicle::Hof {
        termini: vec![t(100, "First"), t(200, "Second")],
        ..Default::default()
    };
    let mut v = script_test_vehicle(osc, vars, "IBIS_terminus_name\n");
    set_player_destination_at(&mut v, &hof, "10", 1, &[]);
    assert_eq!(v.var("display_target"), Some(1.0));
}

/// A route number set by hand keeps the destination a roller blind shows: the row it
/// was cranked to, of which the IBIS knows nothing (taken from the IBIS, at its empty
/// row, a route pick turned the blind back to Empty), its plug-in sign's where one is up,
/// and a pick still waiting for the electrics before either.
#[test]
fn a_route_picked_by_hand_keeps_the_roller_blinds_destination() {
    let osc = "{trigger:rollband_sync}\n{end}\n{trigger:ai_scheduled_settarget}\n(L.L.AI_target_index) (S.L.rlbnd_ziel_target)\n(L.$.SetLineTo) (S.$.rlbnd_line)\n{end}\n";
    let vars = "IBIS_LinieKurs\nIBIS_TerminusIndex\nIBIS_TerminusCode\nAI_target_index\nrlbnd_ziel_target\nrlbnd_steckschild_Termindex\nelec_busbar_main_sw\n";
    let t = |code: i32, id: &str, sign: &str| omsi_vehicle::hof::Terminus {
        code,
        texture_id: id.into(),
        strings: vec![sign.into()],
        ..Default::default()
    };
    let hof = omsi_vehicle::Hof {
        termini: vec![
            t(0, "Empty", ""),
            t(205, "U Ruhleben", "U RUHLEBEN"),
            t(154, "U Rathaus Spandau", "U RATHAUS SPANDAU"),
            t(1001, "Falkensee Bhf", "FALKENSEE BHF"),
        ],
        ..Default::default()
    };
    let mut v = script_test_vehicle(osc, vars, "SetLineTo\nrlbnd_line\nIBIS_terminus_name\n");
    v.set_var("elec_busbar_main_sw", 1.0);
    v.set_var("rlbnd_steckschild_Termindex", -1.0);
    // cranked by hand to row 2; the IBIS (none on the bus) still at the empty row
    v.set_var("rlbnd_ziel_target", 2.0);
    v.set_var("IBIS_TerminusIndex", 0.0);
    v.set_var("IBIS_TerminusCode", 0.0);
    let ti = shown_destination(&v, &hof, None);
    assert_eq!(ti, Some(2));
    let mut pick = set_player_destination_at(&mut v, &hof, "5", ti.unwrap(), &[]);
    turn_roller_blind(&mut v, &mut pick, false);
    assert_eq!(v.var("rlbnd_ziel_target"), Some(2.0));
    assert_eq!(v.str_var("rlbnd_line"), "  5");
    assert_eq!(v.var("IBIS_TerminusCode"), Some(154.0));
    // a plug-in sign up: its row (the script's target_index_int); not one of no sign text
    v.set_var("rlbnd_steckschild_Termindex", 3.0);
    assert_eq!(shown_destination(&v, &hof, None), Some(3));
    v.set_var("rlbnd_steckschild_Termindex", 0.0);
    assert_eq!(shown_destination(&v, &hof, None), Some(2));
    // a destination picked before the bus was switched on, not turned to yet
    assert_eq!(
        shown_destination(
            &v,
            &hof,
            Some(&BlindPick {
                row: 1,
                line: "5".into()
            })
        ),
        Some(1)
    );
    // a bus without a roller blind: the IBIS's row
    let mut ibis = ibis_test_vehicle();
    ibis.set_var("IBIS_TerminusIndex", 1.0);
    ibis.set_var("IBIS_TerminusCode", 205.0);
    assert_eq!(shown_destination(&ibis, &hof, None), Some(1));
}

/// #170: lines 12 and 12A share their stops and terminus; setting the destination for
/// 12A must keep 12A's route, not fall back to the plain 12 route listed before it.
#[test]
fn a_lettered_line_keeps_its_own_route_next_to_the_plain_number() {
    // (the target's route is the route code's last two digits)
    let mut hof = omsi_vehicle::Hof {
        termini: vec![omsi_vehicle::hof::Terminus {
            code: 100,
            strings: vec!["Hafen".into()],
            ..Default::default()
        }],
        ..Default::default()
    };
    for (code, line) in [("1201", "12"), ("1202", "12A"), ("1203", "12B")] {
        hof.info_trips.push(omsi_vehicle::hof::InfoTrip {
            code: code.into(),
            route: "100".into(),
            line: line.into(),
            ..Default::default()
        });
        hof.info_busstop_lists.push(
            ["Markt", "Schule", "Hafen"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        );
    }
    let stops = ["Markt", "Schule", "Hafen"];
    let route = |hof: &omsi_vehicle::Hof, line: &str| {
        ibis_target(hof, line, "Hafen", &stops, None)
            .expect("a target")
            .route
    };
    assert_eq!(route(&hof, "12"), Some(1));
    assert_eq!(route(&hof, "12A"), Some(2));
    assert_eq!(route(&hof, "12B"), Some(3));
    // a lettered line the depot file has no route for falls back to the number's
    assert_eq!(routes_to(&hof, "12C", &[100]), vec![0, 1, 2]);
    // a route under an alias label still counts by its code
    hof.info_trips.push(omsi_vehicle::hof::InfoTrip {
        code: "1204".into(),
        route: "100".into(),
        line: "TML".into(),
        ..Default::default()
    });
    hof.info_busstop_lists.push(
        ["Markt", "Schule", "Hafen"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
    );
    assert_eq!(routes_to(&hof, "12", &[100]), vec![0, 3]);
    assert_eq!(routes_to(&hof, "12A", &[100]), vec![1, 3]);
    assert_eq!(routes_to(&hof, "12C", &[100]), vec![3]);
}

/// A depot file with a row of one destination per route, each of its own code (#738's
/// four "ul. Xutorskaya"): a duty types the route its stops take, and the IBIS shows that
/// route's destination, not the first row's code with no route.
#[test]
fn a_trip_to_a_destination_of_several_codes_types_its_own_route() {
    let t = |code: i32, id: &str, name: &str| omsi_vehicle::hof::Terminus {
        code,
        texture_id: id.into(),
        terminus_stop: Some(id.into()),
        strings: vec![name.into()],
        ..Default::default()
    };
    let mut hof = omsi_vehicle::Hof {
        termini: vec![
            t(0, "Leerfeld", ""),
            t(92, "Xut_92", "ul. Xutorskaya"),
            t(120, "Xut_120", "ul. Xutorskaya"),
            t(123, "Xut_123", "ul. Xutorskaya"),
            t(124, "Xut_124", "ul. Xutorskaya"),
            t(50, "Vokzal", "Vokzal"),
        ],
        ..Default::default()
    };
    let routes: [(&str, &str, &str, &[&str]); 4] = [
        ("3901", "39", "50", &["ul. Xutorskaya", "Rynok", "Vokzal"]),
        ("3902", "39", "120", &["Vokzal", "Rynok", "ul. Xutorskaya"]),
        ("4101", "41", "123", &["Park", "Shkola", "ul. Xutorskaya"]),
        ("4102", "41", "124", &["Vokzal", "Shkola", "ul. Xutorskaya"]),
    ];
    for (code, line, terminus, stops) in routes {
        hof.info_trips.push(omsi_vehicle::hof::InfoTrip {
            code: code.into(),
            route: terminus.into(),
            line: line.into(),
            ..Default::default()
        });
        hof.info_busstop_lists
            .push(stops.iter().map(|s| s.to_string()).collect());
    }
    let target = |hof: &omsi_vehicle::Hof, line: &str, stops: &[&str]| {
        let t = ibis_target(hof, line, "ul. Xutorskaya", stops, None).expect("a target");
        (t.route, t.terminus_code, t.terminus_index)
    };
    assert_eq!(
        target(&hof, "39", &["Vokzal", "Rynok", "ul. Xutorskaya"]),
        (Some(2), None, 2)
    );
    // two routes of line 41 there, each to a row of its own
    assert_eq!(
        target(&hof, "41", &["Park", "Shkola", "ul. Xutorskaya"]),
        (Some(1), None, 3)
    );
    assert_eq!(
        target(&hof, "41", &["Vokzal", "Shkola", "ul. Xutorskaya"]),
        (Some(2), None, 4)
    );
    // no route of the line goes there: the first row's code, typed in the destination mode
    assert_eq!(target(&hof, "7", &[]), (None, Some(92), 1));
    // set without typing (when the typing fails): the same row
    let mut v = ibis_test_vehicle();
    set_player_destination_directly(
        &mut v,
        Some(&hof),
        "41",
        "ul. Xutorskaya",
        &["Vokzal", "Shkola", "ul. Xutorskaya"],
    );
    assert_eq!(
        (v.var("IBIS_TerminusCode"), v.var("IBIS_TerminusIndex")),
        (Some(124.0), Some(4.0))
    );
    // an AI bus shows the first row of the name, as OMSI's AI_target_index does
    set_ai_destination(
        &mut v,
        Some(&hof),
        "41",
        "ul. Xutorskaya",
        &["Vokzal", "Shkola", "ul. Xutorskaya"],
    );
    assert_eq!(v.var("IBIS_TerminusIndex"), Some(1.0));
    // rows of one ident as well
    for term in &mut hof.termini[1..5] {
        term.texture_id = "ul. Xutorskaya".into();
    }
    assert_eq!(
        target(&hof, "39", &["Vokzal", "Rynok", "ul. Xutorskaya"]),
        (Some(2), None, 2)
    );
    assert_eq!(
        target(&hof, "41", &["Vokzal", "Shkola", "ul. Xutorskaya"]),
        (Some(2), None, 4)
    );
}

/// An AI group that names no depot file, or a terminus its depot file has no row for:
/// OMSI still puts the line into `SetLineTo` and runs `ai_scheduled_settarget`, which
/// is what the mods' roller blinds and matrices switch their destination picture on in.
#[test]
fn an_ai_bus_without_a_destination_still_gets_its_line_and_the_ai_trigger() {
    let osc = "{trigger:ai_scheduled_settarget}\n(L.$.SetLineTo) (S.$.shown)\n1 (S.L.elec_busbar_main_sw)\n{end}\n";
    let vars = "AI_target_index\nelec_busbar_main_sw\n";
    let strings = "SetLineTo\nshown\n";
    // no depot file at all (a plain [aigroup_2] group)
    let mut v = script_test_vehicle(osc, vars, strings);
    set_ai_destination(&mut v, None, "82A", "82A Seaview Bay", &[]);
    assert_eq!(v.str_var("SetLineTo"), "82A");
    assert_eq!(v.str_var("shown"), "82A");
    assert_eq!(v.var("elec_busbar_main_sw"), Some(1.0));
    // a terminus the depot file knows nothing of (AI_target_index stays as it was)
    let hof = omsi_vehicle::Hof {
        termini: vec![omsi_vehicle::hof::Terminus {
            code: 92,
            texture_id: "Xut_92".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut v = script_test_vehicle(osc, vars, strings);
    set_ai_destination(&mut v, Some(&hof), "7", "Hafen", &[]);
    assert_eq!(v.str_var("SetLineTo"), "7");
    assert_eq!(v.str_var("shown"), "7");
    assert_eq!(v.var("elec_busbar_main_sw"), Some(1.0));
    assert_eq!(v.var("AI_target_index"), Some(0.0));
}

fn early_departure_duty() -> PlayerDuty {
    PlayerDuty {
        line: "5".into(),
        tour: "1".into(),
        trips: vec![
            planned(100.0, &[(0.0, 100.0, 100.0), (500.0, 200.0, 200.0)]),
            planned(600.0, &[(500.0, 600.0, 600.0), (1000.0, 700.0, 700.0)]),
            planned(900.0, &[(1000.0, 900.0, 900.0), (1500.0, 1000.0, 1000.0)]),
        ],
        trip_index: 0,
        first_trip: 0,
        next_stop: 1,
        at_stop: false,
        arrived_late: None,
        done: false,
        served_terminus: None,
        left_late: Some(0.0),
        held_back: false,
        placed: true,
        trip_changed: false,
        skipped: None,
        picked: true,
        first_update: None,
        heading: 90.0,
        position: None,
        jumped: false,
        skips: Default::default(),
    }
}

/// Aachen's ibox only has `ibox_busstop` and does `+ 1` when the TT index changes. A
/// jump of several stops must pre-position that counter so the `+ 1` lands on the new
/// stop - otherwise the list stays behind the announcement.
#[test]
fn ibox_busstop_is_prepositioned_when_the_timetable_jumps() {
    let stops = ["Elsternplatz", "Bhf. Nordspitze", "Nordsp. Bauernhof"];
    let mut hof = omsi_vehicle::Hof::default();
    for s in stops {
        hof.bus_stops.push(omsi_vehicle::hof::BusStop {
            ident: s.into(),
            strings: vec![s.into(), s.into(), s.into(), s.into()],
        });
    }
    hof.info_busstop_lists
        .push(stops.iter().map(|s| s.to_string()).collect());
    let mut bus = script_test_vehicle(
        "{frame}\n{end}\n",
        "ibox_busstop\nibox_routenindex\nibox_TTBusstopIndexLAST\n",
        "",
    );
    bus.host.hof = Some(std::sync::Arc::new(hof));
    bus.set_var("ibox_routenindex", 0.0);
    bus.set_var("ibox_busstop", 0.0);
    bus.host.tt_busstop_index = 0;
    let planned_stops: Vec<PlannedStop> = stops
        .iter()
        .enumerate()
        .map(|(i, name)| PlannedStop {
            object_id: i as i64,
            name: (*name).into(),
            arr: i as f64 * 60.0,
            dep: i as f64 * 60.0,
            position: Some(glam::DVec3::new(i as f64 * 100.0, 0.0, 0.0)),
            dir: StopDir::default(),
            stops: true,
        })
        .collect();
    let mut duty = PlayerDuty {
        line: "76".into(),
        tour: "2".into(),
        trips: vec![PlannedTrip {
            name: "76_2".into(),
            line: "76".into(),
            terminus: "Nordsp. Bauernhof".into(),
            departure: 0.0,
            end: planned_stops.last().unwrap().arr,
            stops: planned_stops,
        }],
        trip_index: 0,
        first_trip: 0,
        next_stop: 0,
        at_stop: false,
        arrived_late: None,
        done: false,
        served_terminus: None,
        left_late: None,
        held_back: false,
        placed: true,
        trip_changed: false,
        skipped: None,
        picked: true,
        first_update: None,
        heading: 90.0,
        position: None,
        jumped: false,
        skips: Default::default(),
    };
    // jump from the first stop to the last: the script will +1 once, so leave 1 behind
    duty.next_stop = 2;
    duty.feed_host(&mut bus, 0.0);
    assert_eq!(bus.host.tt_busstop_index, 2);
    assert_eq!(
        bus.var("ibox_busstop"),
        Some(1.0),
        "pre-position for the script's +1"
    );
    // after the unit's own +1 the list and the announcement both show stop 2
    bus.set_var("ibox_busstop", bus.var("ibox_busstop").unwrap() + 1.0);
    assert_eq!(bus.var("ibox_busstop"), Some(2.0));
}

/// Aachen's Fahrplan list uses `GetTTBusstopIndex` as the highlighted (bottom) stop. At
/// the start of the line that must be 0. If only a later stop's place is loaded yet,
/// placing must not jump the index to that later stop.
#[test]
fn duty_place_keeps_first_stop_while_its_place_is_unknown() {
    let mut trip = planned(
        100.0,
        &[
            (0.0, 100.0, 100.0),
            (20.0, 200.0, 200.0),
            (40.0, 300.0, 300.0),
        ],
    );
    trip.stops[0].position = None;
    let mut d = PlayerDuty {
        line: "33".into(),
        tour: "1".into(),
        trips: vec![trip],
        trip_index: 0,
        first_trip: 0,
        next_stop: 0,
        at_stop: false,
        arrived_late: None,
        done: false,
        served_terminus: None,
        left_late: None,
        held_back: false,
        placed: true,
        trip_changed: false,
        skipped: None,
        picked: true,
        first_update: None,
        heading: 90.0,
        position: None,
        jumped: false,
        skips: Default::default(),
    };
    // late for the trip, standing at stop 2 whose place is known
    d.place(glam::DVec3::new(20.0, 0.0, 0.0), 250.0);
    assert_eq!(
        d.next_stop, 0,
        "do not highlight stop #2 while stop #1 has no place"
    );
    assert!(d.left_late.is_none());
    let mut bus = timetable_test_vehicle();
    d.feed_host(&mut bus, 250.0);
    assert_eq!(bus.host.tt_busstop_index, 0);
    assert_eq!(bus.host.tt_stops[0].0, "s0");
}

/// Skipping a stop does not make the bus "come from" the stop it never served: the delay
/// toward the stop after it is not worked out between those two stops, which would make
/// it jump by the stretch between them.
#[test]
fn a_skipped_stop_is_not_the_one_the_bus_came_from() {
    let mut duty = early_departure_duty();
    duty.trips[0] = planned(
        100.0,
        &[
            (0.0, 100.0, 100.0),
            (500.0, 200.0, 200.0),
            (1000.0, 300.0, 300.0),
        ],
    );
    duty.next_stop = 1;
    duty.position = Some(glam::DVec3::new(250.0, 0.0, 0.0));
    assert_eq!(
        duty.delay(150.0),
        0.0,
        "half way between the stops 0 and 1, on time"
    );
    assert!(duty.skip_to(2));
    // not -75 (interpolated from stop 1 as if the bus had left it)
    assert_eq!(duty.delay(150.0), 0.0);
}

#[test]
fn serving_the_terminus_then_leaving_60m_starts_the_next_trip_early() {
    for door in ["door_0", "PAX_Exit0_Open"] {
        let mut duty = early_departure_duty();
        let mut bus = timetable_test_vehicle_with_door(door);
        bus.position.x = 500.0;
        bus.set_var(door, 1.0);
        duty.update(&mut bus, 150.0);
        assert_eq!(duty.trip_index, 0, "opening a door does not depart");
        bus.set_var(door, 0.0);
        bus.set_speed(5.0);
        bus.position.x = 559.9;
        assert_eq!(duty.update(&mut bus, 160.0), Some((-50.0, -40.0)));
        assert_eq!(duty.trip_index, 0, "less than 60 m away");
        bus.position.x = 560.0;
        duty.update(&mut bus, 161.0);
        assert_eq!(
            duty.trip_index, 0,
            "a trip due in more than five minutes waits (a bus moved to its layover)"
        );
        duty.update(&mut bus, 301.0);
        assert_eq!((duty.trip_index, duty.next_stop), (1, 1));
        assert_eq!(bus.host.tt_busstop_index, 1);
        assert_eq!(bus.host.tt_stops[0].1, 600.0);
        // (60 m of the 500 to the next stop gone: due there at 612)
        assert_eq!(duty.delay(301.0), -311.0);
        assert!(duty.take_trip_change());
        duty.update(&mut bus, 302.0);
        assert!(!duty.take_trip_change());
        assert_eq!(duty.trip_index, 1, "advance exactly one trip");
        // The previous trip's door opening cannot finish the next trip too.
        bus.position.x = 1000.0;
        duty.update(&mut bus, 400.0);
        bus.position.x = 1060.0;
        duty.update(&mut bus, 410.0);
        assert_eq!(duty.trip_index, 1);
    }
}

#[test]
fn early_trip_progress_requires_stopping_with_a_door_open_at_the_last_stop() {
    for (speed, open) in [(5.0, 1.0), (0.0, 0.0)] {
        let mut duty = early_departure_duty();
        let mut bus = timetable_test_vehicle();
        // Doors opened at another stop do not serve the terminus.
        bus.set_var("door_0", 1.0);
        duty.update(&mut bus, 140.0);
        bus.position.x = 500.0;
        bus.set_speed(speed);
        bus.set_var("door_0", open);
        duty.update(&mut bus, 150.0);
        bus.position.x = 560.0;
        bus.set_speed(5.0);
        bus.set_var("door_0", 0.0);
        duty.update(&mut bus, 160.0);
        assert_eq!(duty.trip_index, 0);
    }
}

/// A circular trip: a terminal, three stops round a block and the terminal again (its
/// two ends are the same place). `(x, y)` of each stop, one every 100 s.
fn circular_duty(places: &[(f64, f64)]) -> PlayerDuty {
    let mut trip = planned(100.0, &[(0.0, 100.0, 100.0), (0.0, 200.0, 200.0)]);
    trip.stops = places
        .iter()
        .enumerate()
        .map(|(i, &(x, y))| PlannedStop {
            object_id: i as i64,
            name: format!("s{i}"),
            arr: 100.0 * (i + 1) as f64,
            dep: 100.0 * (i + 1) as f64,
            position: Some(glam::DVec3::new(x, y, 0.0)),
            dir: StopDir::default(),
            stops: true,
        })
        .collect();
    trip.end = trip.stops.last().unwrap().arr;
    trip.set_dirs();
    let mut duty = early_departure_duty();
    duty.trips = vec![trip];
    duty.next_stop = 0;
    duty.left_late = None;
    duty
}

fn drive(
    duty: &mut PlayerDuty,
    bus: &mut omsi_sim::VehicleInstance,
    to: (f64, f64),
    heading: f64,
    t: f64,
) -> Option<(f64, f64)> {
    bus.position.x = to.0;
    bus.position.y = to.1;
    bus.heading = heading;
    bus.set_speed(5.0);
    duty.update(bus, t)
}

/// Leaving the terminal of a circular line whose arrival platform lies a few metres ahead
/// of the departure platform: the stops of the trip are not passed by it at once.
#[test]
fn leaving_the_terminal_of_a_circular_line_skips_no_stop() {
    // arrive at (30, 0) from the west, leave from (0, 0) the same way: the loop is
    // round a block, back in from the west
    let mut duty = circular_duty(&[
        (0.0, 0.0),
        (400.0, 0.0),
        (400.0, 400.0),
        (-300.0, 400.0),
        (-300.0, 0.0),
        (30.0, 0.0),
    ]);
    let mut bus = timetable_test_vehicle();
    bus.position.x = 0.0;
    bus.heading = 90.0;
    bus.set_speed(0.0);
    bus.set_var("door_0", 1.0);
    duty.update(&mut bus, 100.0);
    bus.set_var("door_0", 0.0);
    for (t, x) in [
        (104.0, 10.0),
        (108.0, 20.0),
        (112.0, 30.0),
        (116.0, 40.0),
        (120.0, 50.0),
    ] {
        drive(&mut duty, &mut bus, (x, 0.0), 90.0, t);
    }
    assert_eq!(duty.take_skipped(), None);
    assert_eq!(duty.next_stop, 1);
    assert!(!duty.done);
}

/// A stop driven past is still caught up when it lies away from the stop just left.
#[test]
fn a_skipped_stop_far_from_the_one_just_left_is_still_caught_up() {
    let mut duty = circular_duty(&[(0.0, 0.0), (400.0, 300.0), (800.0, 0.0), (1200.0, 0.0)]);
    let mut bus = timetable_test_vehicle();
    bus.position.x = 0.0;
    bus.heading = 90.0;
    bus.set_speed(0.0);
    bus.set_var("door_0", 1.0);
    duty.update(&mut bus, 100.0);
    bus.set_var("door_0", 0.0);
    for (t, x) in [
        (110.0, 50.0),
        (200.0, 390.0),
        (210.0, 450.0),
        (300.0, 790.0),
    ] {
        drive(&mut duty, &mut bus, (x, 0.0), 90.0, t);
    }
    assert_eq!(duty.take_skipped(), Some((1, 2, 3)));
    assert_eq!(duty.next_stop, 2);
}

const H: f64 = 3600.0;

/// A tour that crosses midnight (23:40, then 24:10 and 24:40): `Schedule::roll_day` moves
/// the day's midnight on as soon as the clock reaches it, so `tod` never reaches a time
/// past 24:00 and `due_in_window` alone never finds the later trips due (#40, which this
/// test used to characterize). `due_previous_day` is the same check on the clock of the
/// previous midnight, and every trip is due exactly once along the night.
#[test]
fn every_trip_of_a_tour_across_midnight_is_due_once() {
    let window = 60.0;
    let trips = [
        23.0 * H + 40.0 * 60.0,
        24.0 * H + 10.0 * 60.0,
        24.0 * H + 40.0 * 60.0,
    ];
    // the clock of the traffic, from 23:00 to 02:00 the next day, a second at a time; the
    // timetable's day rolls at 24:00 as in `Schedule::tick`
    let mut seen = [0u32; 3];
    for clock in (23 * 3600)..(26 * 3600) {
        let clock = clock as f64;
        let (day_base, tod) = if clock >= DAY {
            (DAY, clock - DAY)
        } else {
            (0.0, clock)
        };
        for (k, &t) in trips.iter().enumerate() {
            // (the first day's mask is the one that runs the whole tour)
            let today = day_base == 0.0 && due_in_window(t, tod, window);
            let yesterday = day_base == DAY && due_previous_day(t, tod, window);
            if today || yesterday {
                seen[k] += 1;
                // the departure is `window` seconds wide: it is due that long
            }
        }
    }
    assert_eq!(
        seen,
        [60, 60, 60],
        "each trip is due during its `window` seconds, once"
    );
}

/// The day mask of a tour is the one of the day its timetable starts; departures written
/// 24:00 or later keep it (maintainer's observation, #40). A Monday-only night tour runs
/// its 00:10 trip in the early hours of Tuesday (it is checked against Monday's bits); a
/// Tuesday-only tour does not run that trip, which belongs to the Monday timetable.
/// Synthetic masks; a single-day tour whose first departure is already past 24:00 does not
/// occur in real maps, and is assumed to follow the same rule (not validated).
#[test]
fn a_trip_past_midnight_keeps_the_mask_of_the_day_the_tour_started() {
    let school = 1 << 9;
    let monday = (1 << 0, school);
    let mondays_only = (1 << 0) | school;
    let tuesdays_only = (1 << 1) | school;
    // at 00:10 on Tuesday the previous day's bits are Monday's
    let yesterday = monday;
    assert!(
        mask_matches(mondays_only, yesterday),
        "Monday's night tour runs its 00:10 trip"
    );
    assert!(
        !mask_matches(tuesdays_only, yesterday),
        "a Tuesday-only tour does not run Monday's 00:10 trip"
    );
    // a Saturday-only tour of 4:47 to 28:25 is still a Saturday tour at 03:00 on Sunday
    let (saturday, sunday) = ((1 << 5, school), (1 << 6, school));
    assert!(mask_matches((1 << 5) | school, saturday));
    assert!(
        !mask_matches((1 << 5) | school, sunday),
        "the mask is not Sunday's"
    );
}

#[test]
fn departures_of_the_previous_day_table() {
    let w = 60.0;
    // (time of the departure, tod of the new day, window, due)
    let cases = [
        (24.0 * H, 0.0, w, true),   // exactly 00:00
        (24.0 * H, 59.0, w, true),  // still inside the window
        (24.0 * H, 60.0, w, false), // window over
        (24.0 * H + 10.0 * 60.0, 10.0 * 60.0, w, true),
        (24.0 * H + 10.0 * 60.0, 9.0 * 60.0 + 59.0, w, false), // not yet
        (23.0 * H + 59.0 * 60.0 + 59.0, 0.0, w, false), // 23:59:59 is an evening trip: today's departure, not yesterday's
        (23.0 * H + 50.0 * 60.0, 5.0 * 60.0, 20.0 * 60.0, false), // clock set to 00:05: the evening trip is not caught up (its flag is today's)
        (24.0 * H + 3.0 * 60.0, 5.0 * 60.0, 20.0 * 60.0, true), // clock set to 00:05: the trip of 00:03 of the night tour is under way
        (36.0 * H, 12.0 * H, w, true), // a very long tour: 36:00 is noon of the day after
        (48.0 * H + 60.0, 60.0, w, false), // a day or more past 24:00 belongs to no previous day
        (10.0 * 60.0, 10.0 * 60.0, w, false), // a morning trip of today is not yesterday's
    ];
    for (time, tod, window, due) in cases {
        assert_eq!(
            due_previous_day(time, tod, window),
            due,
            "time {time} tod {tod} window {window}"
        );
    }
    // today's check is untouched: a trip at 00:10 is due at 00:10 of its own day
    assert!(due_in_window(10.0 * 60.0, 10.0 * 60.0, w));
    // and a trip past 24:00 is never due in the day's own view
    assert!(!(0..DAY as i64).any(|tod| due_in_window(24.0 * H + 600.0, tod as f64, w)));
}

#[test]
fn waiting_at_the_terminus_still_uses_the_scheduled_changeover() {
    let mut duty = early_departure_duty();
    let mut bus = timetable_test_vehicle();
    bus.position.x = 500.0;
    bus.set_var("door_0", 1.0);
    duty.update(&mut bus, 150.0);
    duty.update(&mut bus, 539.0);
    assert_eq!(duty.trip_index, 0);
    duty.update(&mut bus, 540.0);
    assert_eq!((duty.trip_index, duty.next_stop), (1, 0));
    assert!(duty.served_terminus.is_none());
}

#[test]
fn early_departure_does_not_run_past_the_end_of_the_duty() {
    let mut duty = early_departure_duty();
    duty.trips.truncate(1);
    let mut bus = timetable_test_vehicle();
    bus.position.x = 500.0;
    bus.set_var("door_0", 1.0);
    duty.update(&mut bus, 150.0);
    bus.position.x = 560.0;
    bus.set_speed(5.0);
    duty.update(&mut bus, 160.0);
    assert_eq!(duty.trip_index, 0);
    assert!(duty.done);
}

#[test]
fn resumed_duty_keeps_its_trip_and_stop_before_the_first_script_frame() {
    let trips = vec![
        planned(100.0, &[(0.0, 100.0, 100.0), (500.0, 200.0, 220.0)]),
        planned(
            400.0,
            &[
                (500.0, 400.0, 400.0),
                (1000.0, 500.0, 520.0),
                (1500.0, 600.0, 600.0),
            ],
        ),
        planned(800.0, &[(1500.0, 800.0, 800.0)]),
    ];
    let mut d = PlayerDuty {
        line: "5".into(),
        tour: "65104".into(),
        trips,
        trip_index: 0,
        first_trip: 0,
        next_stop: 0,
        at_stop: false,
        arrived_late: None,
        done: false,
        served_terminus: None,
        left_late: None,
        held_back: false,
        placed: false,
        trip_changed: false,
        skipped: None,
        picked: false,
        first_update: None,
        heading: 90.0,
        position: None,
        jumped: false,
        skips: Default::default(),
    };
    // The same name appears twice. Its saved ordinal, rather than its name or the
    // bus's position far from any stop, selects the second occurrence.
    d.trips[1].stops[0].name = "Central".into();
    d.trips[1].stops[1].name = "Central".into();
    d.start_at(1, 0);
    d.restore_progress(1, glam::DVec3::new(-3500.0, 0.0, 0.0));
    assert_eq!(
        d.left_late,
        Some(0.0),
        "past its first stop the bus is on its way"
    );
    assert!(
        !d.take_trip_change(),
        "resume must not request automatic reprogramming"
    );
    let mut v = timetable_test_vehicle();
    // Demonstrate that this device really loses its programming without callbacks.
    let vars = vec![("duty".into(), 65104.0)];
    let strings = vec![("destination".into(), "  Manual destination  ".into())];
    v.restore_script_state(&vars, &strings);
    v.update(0.02);
    assert_eq!(v.var("duty"), Some(0.0));
    v.restore_script_state(&vars, &strings);
    v.position = glam::DVec3::new(-3500.0, 0.0, 0.0);
    d.restore_host(&mut v, 542.0);
    v.update(0.02);
    assert_eq!(v.var("duty"), Some(65104.0));
    let dest = v.ty.program.str_var("destination").unwrap() as usize;
    assert_eq!(v.state.str_vars[dest], "  Manual destination  ");
    assert_eq!(v.var("observed_stop"), Some(1.0));
    assert_eq!(v.var("observed_delay"), Some(42.0));
    d.update(&mut v, 542.0);
    assert_eq!((d.trip_index, d.next_stop), (1, 1));
    assert_eq!(d.tour, "65104");
    d.restore_progress(usize::MAX, glam::DVec3::new(-3500.0, 0.0, 0.0));
    assert_eq!(d.next_stop, 2);
    assert!(!d.done, "away from the last stop the trip is not over yet");
    d.restore_progress(2, glam::DVec3::new(1500.0, 0.0, 0.0));
    assert!(d.done, "saved at the last stop, the trip is over");
    d.restore_progress(0, glam::DVec3::new(-3500.0, 0.0, 0.0));
    assert_eq!(d.next_stop, 0);
    assert_eq!(d.left_late, None);
    assert!(!d.done);
    // Ordinary duty updates retain their existing handling of schedule_active.
    v.host.schedule_active = 0.0;
    v.set_var("schedule_active", 0.0);
    d.update(&mut v, 542.0);
    assert_eq!(v.host.schedule_active, 0.0);
    assert_eq!(v.var("schedule_active"), Some(0.0));
}

/// #58: Transcol's `[aigroup_depot]` groups name garages ("Satelite", "PS14") that no
/// folder has a depot file of; its AI buses carry only Transcol.hof. OMSI runs such a bus
/// with the first depot file of its own folder; without one the sign stayed blank.
#[test]
fn a_depot_group_naming_a_missing_depot_uses_the_buses_own() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("depot-missing-hof-{stamp}"));
    std::fs::create_dir_all(&dir).unwrap();
    let hof = |name: &str, terminus: &str| {
        format!("[name]\n{name}\n\nstringcount_terminus\n1\n\n[addterminus]\n804\n{terminus}\n{terminus}\n")
    };
    std::fs::write(
        dir.join("Transcol.hof"),
        hof("Transcol", "804 T.Laranjeiras"),
    )
    .unwrap();
    // (a later file of the folder: OMSI's index 0 is the first one)
    std::fs::write(
        dir.join("Zona Norte.hof"),
        hof("Zona Norte", "804 Zona Norte"),
    )
    .unwrap();
    let missing = format!("Satelite {stamp}");
    let mut cache = HashMap::new();
    let h = testing::schedule::depot_file(&mut cache, &dir, &missing).expect("the bus's own depot");
    assert_eq!(h.name, "Transcol");
    assert!(testing::schedule::find_terminus(&h, "804 T.Laranjeiras").is_some());
    // a depot file of the name the group gives still wins
    let named = format!("Unimar AR {stamp}");
    std::fs::write(dir.join(format!("{named}.hof")), hof(&named, "804 Unimar")).unwrap();
    let h = testing::schedule::depot_file(&mut cache, &dir, &named).expect("the named depot");
    assert_eq!(h.name, named);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn terminus_index_is_the_depot_terminus_of_that_name() {
    let mut hof = omsi_vehicle::hof::Hof::default();
    for name in ["A", "B", "C"] {
        hof.termini.push(omsi_vehicle::hof::Terminus {
            texture_id: name.into(),
            ..Default::default()
        });
    }
    assert_eq!(tt_terminus_index(Some(&hof), "B"), 1);
    assert_eq!(tt_terminus_index(Some(&hof), "b"), -1);
    assert_eq!(tt_terminus_index(None, "B"), -1);
}

#[test]
fn a_stop_placed_only_once_its_tile_loads_is_reached() {
    // stop 1's object was in no tile loaded when the duty began
    let mut t = planned(
        0.0,
        &[
            (0.0, 0.0, 0.0),
            (500.0, 100.0, 100.0),
            (1000.0, 200.0, 200.0),
        ],
    );
    t.stops[1].position = None;
    let mut d = PlayerDuty {
        line: "5".into(),
        tour: "1".into(),
        trips: vec![t],
        trip_index: 0,
        first_trip: 0,
        next_stop: 0,
        at_stop: false,
        arrived_late: None,
        done: false,
        served_terminus: None,
        left_late: None,
        held_back: false,
        placed: true,
        trip_changed: false,
        skipped: None,
        picked: true,
        first_update: None,
        heading: 90.0,
        position: None,
        jumped: false,
        skips: Default::default(),
    };
    d.advance(glam::DVec3::new(0.0, 0.0, 0.0), 0.0);
    d.advance(glam::DVec3::new(100.0, 0.0, 0.0), 10.0);
    assert_eq!(d.next_stop, 1);
    // its tile comes: the map has it now
    let mut positions = HashMap::new();
    positions.insert(1i64, (glam::DVec3::new(500.0, 0.0, 0.0), [0.0; 3]));
    d.learn_loaded(&positions);
    assert_eq!(
        d.trips[0].stops[1].position,
        Some(glam::DVec3::new(500.0, 0.0, 0.0))
    );
    d.advance(glam::DVec3::new(500.0, 0.0, 0.0), 100.0);
    d.advance(glam::DVec3::new(700.0, 0.0, 0.0), 130.0);
    assert_eq!(d.next_stop, 2);
}

#[test]
fn the_next_trip_starts_at_its_first_stop_though_the_last_one_was_missed() {
    // trip 1 ends at x = 1000 (a stop object the bus never comes within 25 m of: it
    // stands at x = 1040, where trip 2 leaves from)
    let t1 = planned(
        0.0,
        &[
            (0.0, 0.0, 0.0),
            (500.0, 100.0, 100.0),
            (1000.0, 200.0, 200.0),
        ],
    );
    let t2 = planned(400.0, &[(1040.0, 400.0, 400.0), (1500.0, 500.0, 500.0)]);
    let mut d = PlayerDuty {
        line: "5".into(),
        tour: "1".into(),
        trips: vec![t1, t2],
        trip_index: 0,
        first_trip: 0,
        next_stop: 0,
        at_stop: false,
        arrived_late: None,
        done: false,
        served_terminus: None,
        left_late: None,
        held_back: false,
        placed: true,
        trip_changed: false,
        skipped: None,
        picked: true,
        first_update: None,
        heading: 90.0,
        position: None,
        jumped: false,
        skips: Default::default(),
    };
    d.advance(glam::DVec3::new(0.0, 0.0, 0.0), 0.0);
    d.advance(glam::DVec3::new(100.0, 0.0, 0.0), 10.0);
    d.advance(glam::DVec3::new(500.0, 0.0, 0.0), 100.0);
    d.advance(glam::DVec3::new(700.0, 0.0, 0.0), 130.0);
    assert_eq!(d.next_stop, 2);
    d.take_trip_change();
    // at trip 2's first stop a minute before it leaves: trip 2, and the IBIS is told
    d.advance(glam::DVec3::new(1040.0, 0.0, 0.0), 300.0);
    assert_eq!(
        d.trip_index, 0,
        "not before a minute ahead of the departure"
    );
    d.advance(glam::DVec3::new(1040.0, 0.0, 0.0), 345.0);
    assert_eq!(d.trip_index, 1);
    assert!(d.take_trip_change());
    // a bus still on its way (not at trip 2's first stop) stays on trip 1
    let t1 = planned(
        0.0,
        &[
            (0.0, 0.0, 0.0),
            (500.0, 100.0, 100.0),
            (1000.0, 200.0, 200.0),
        ],
    );
    let t2 = planned(400.0, &[(1040.0, 400.0, 400.0), (1500.0, 500.0, 500.0)]);
    let mut d = PlayerDuty {
        line: "5".into(),
        tour: "1".into(),
        trips: vec![t1, t2],
        trip_index: 0,
        first_trip: 0,
        next_stop: 2,
        at_stop: false,
        arrived_late: None,
        done: false,
        served_terminus: None,
        left_late: Some(0.0),
        held_back: false,
        placed: true,
        trip_changed: false,
        skipped: None,
        picked: true,
        first_update: None,
        heading: 90.0,
        position: None,
        jumped: false,
        skips: Default::default(),
    };
    d.advance(glam::DVec3::new(800.0, 0.0, 0.0), 345.0);
    assert_eq!(d.trip_index, 0);
}

/// #1015: the next stop given up from the menu, and at the trip's last one the trip.
#[test]
fn the_next_stop_can_be_skipped() {
    let trip = planned(
        0.0,
        &[
            (0.0, 0.0, 0.0),
            (100.0, 60.0, 60.0),
            (500.0, 120.0, 120.0),
            (1000.0, 200.0, 200.0),
        ],
    );
    let next = planned(400.0, &[(1040.0, 400.0, 400.0), (1500.0, 500.0, 500.0)]);
    let mut d = PlayerDuty {
        line: "5".into(),
        tour: "1".into(),
        trips: vec![trip, next],
        trip_index: 0,
        first_trip: 0,
        next_stop: 0,
        at_stop: false,
        arrived_late: None,
        done: false,
        served_terminus: None,
        left_late: None,
        held_back: false,
        placed: true,
        trip_changed: false,
        skipped: None,
        picked: true,
        first_update: None,
        heading: 90.0,
        position: None,
        jumped: false,
        skips: Default::default(),
    };
    // at the first stop and away from it: the next is s1
    d.advance(glam::DVec3::new(0.0, 0.0, 0.0), 0.0);
    d.advance(glam::DVec3::new(50.0, 0.0, 0.0), 10.0);
    assert_eq!(d.next_stop, 1);
    assert_eq!(d.skip_next().as_deref(), Some("s1"));
    assert_eq!(d.next_stop, 2);
    // passing s1 now serves nothing: s2 is still the one due
    assert_eq!(d.advance(glam::DVec3::new(100.0, 0.0, 0.0), 60.0), None);
    assert_eq!(d.next_stop, 2);
    assert_eq!(d.skip_next().as_deref(), Some("s2"));
    // the last stop skipped: the trip is over, and the tour's next trip follows
    assert!(d.stop_to_skip());
    assert_eq!(d.skip_next().as_deref(), Some("s3"));
    assert!(!d.stop_to_skip());
    assert_eq!(d.skip_next(), None);
    d.advance(glam::DVec3::new(700.0, 0.0, 0.0), 345.0);
    assert_eq!(d.trip_index, 1);
    assert_eq!(d.next_stop, 0);
}

#[test]
fn a_page_can_go_back_to_an_earlier_stop() {
    let trip = planned(
        0.0,
        &[
            (0.0, 0.0, 0.0),
            (100.0, 60.0, 60.0),
            (500.0, 120.0, 120.0),
            (1000.0, 200.0, 200.0),
        ],
    );
    let mut d = PlayerDuty {
        line: "5".into(),
        tour: "1".into(),
        trips: vec![trip],
        trip_index: 0,
        first_trip: 0,
        next_stop: 0,
        at_stop: false,
        arrived_late: None,
        done: false,
        served_terminus: None,
        left_late: None,
        held_back: false,
        placed: true,
        trip_changed: false,
        skipped: None,
        picked: true,
        first_update: None,
        heading: 90.0,
        position: None,
        jumped: false,
        skips: Default::default(),
    };
    assert!(d.skip_to(2));
    assert_eq!(d.next_stop, 2);
    // back one stop: due again
    assert!(d.skip_to(1));
    assert_eq!(d.next_stop, 1);
    // the stop it is already heading for: nothing changes
    assert!(!d.skip_to(1));
    // the bus stands at stop 2: the duty does not jump forward again by itself
    d.advance(glam::DVec3::new(500.0, 0.0, 0.0), 100.0);
    assert_eq!(d.next_stop, 1);
    // from the last stop (done) back reopens the trip, forwards does not
    d.skip_to(3);
    d.advance(glam::DVec3::new(1000.0, 0.0, 0.0), 200.0);
    assert!(d.done);
    assert!(!d.skip_to(3));
    assert!(d.skip_to(2));
    assert!(!d.done);
    assert_eq!(d.next_stop, 2);
}

#[test]
fn a_loop_does_not_jump_to_the_stop_over_the_road() {
    // out along y = 0 to x = 1000, back along y = 12: stop 1 at x = 100 going out, stop 5
    // at x = 100 coming back, 12 m apart (#254)
    let mut trip = planned(
        0.0,
        &[
            (0.0, 0.0, 0.0),
            (100.0, 60.0, 60.0),
            (500.0, 120.0, 120.0),
            (1000.0, 200.0, 200.0),
            (500.0, 280.0, 280.0),
            (100.0, 340.0, 340.0),
            (0.0, 400.0, 400.0),
        ],
    );
    for (i, s) in trip.stops.iter_mut().enumerate() {
        if i >= 4 {
            s.position.as_mut().unwrap().y = 12.0;
        }
    }
    trip.set_dirs();
    let mut d = PlayerDuty {
        line: "5".into(),
        tour: "1".into(),
        trips: vec![trip],
        trip_index: 0,
        first_trip: 0,
        next_stop: 0,
        at_stop: false,
        arrived_late: None,
        done: false,
        served_terminus: None,
        left_late: None,
        held_back: false,
        placed: true,
        trip_changed: false,
        skipped: None,
        picked: true,
        first_update: None,
        heading: 90.0,
        position: None,
        jumped: false,
        skips: Default::default(),
    };
    // at stop 0, then leaving east
    d.advance(glam::DVec3::new(0.0, 0.0, 0.0), 0.0);
    d.advance(glam::DVec3::new(60.0, 0.0, 0.0), 30.0);
    assert_eq!(d.next_stop, 1);
    // at stop 1 heading east: stop 5 (12 m away, the other way round) is not taken
    d.advance(glam::DVec3::new(100.0, 0.0, 0.0), 60.0);
    d.advance(glam::DVec3::new(140.0, 0.0, 0.0), 70.0);
    assert_eq!(
        d.next_stop, 2,
        "the duty goes on to stop 2, not over the road to stop 5"
    );
}

#[test]
fn stops_passed_without_stopping_are_told_once() {
    let mut trip = planned(
        0.0,
        &[
            (0.0, 0.0, 0.0),
            (100.0, 60.0, 60.0),
            (500.0, 120.0, 120.0),
            (1000.0, 200.0, 200.0),
            (1500.0, 280.0, 280.0),
        ],
    );
    trip.set_dirs();
    let mut d = PlayerDuty {
        line: "5".into(),
        tour: "1".into(),
        trips: vec![trip],
        trip_index: 0,
        first_trip: 0,
        next_stop: 0,
        at_stop: false,
        arrived_late: None,
        done: false,
        served_terminus: None,
        left_late: None,
        held_back: false,
        placed: true,
        trip_changed: false,
        skipped: None,
        picked: true,
        first_update: None,
        heading: 90.0,
        position: None,
        jumped: false,
        skips: Default::default(),
    };
    d.advance(glam::DVec3::new(0.0, 0.0, 0.0), 0.0);
    d.advance(glam::DVec3::new(60.0, 0.0, 0.0), 30.0);
    assert_eq!(d.next_stop, 1);
    assert_eq!(d.take_skipped(), None, "leaving a stop served skips none");
    // the bus turns up at stop 4 (numbered from 1) heading on: stops 2 and 3 were passed
    d.advance(glam::DVec3::new(1000.0, 0.0, 0.0), 90.0);
    assert_eq!(d.next_stop, 3);
    assert_eq!(
        d.take_skipped(),
        Some((2, 2, 4)),
        "two stops, due at 2, now at 4"
    );
    assert_eq!(d.take_skipped(), None, "told once");
}

#[test]
fn a_duty_starts_with_the_trip_that_fits_the_time() {
    // Spandau line 5, tour "Mo-Fr 3": a depot run 14:44-15:01, then 15:07 and 16:01
    let trips = vec![
        planned(
            53040.0,
            &[(0.0, 53040.0, 53040.0), (500.0, 54060.0, 54060.0)],
        ),
        planned(
            54420.0,
            &[(500.0, 54420.0, 54420.0), (1000.0, 56400.0, 56400.0)],
        ),
        planned(
            57660.0,
            &[(1000.0, 57660.0, 57660.0), (500.0, 59400.0, 59400.0)],
        ),
    ];
    assert_eq!(
        starting_trip(&trips, 15.0 * 3600.0 + 300.0),
        1,
        "15:05: the 15:07"
    );
    assert_eq!(
        starting_trip(&trips, 14.0 * 3600.0 + 50.0 * 60.0),
        0,
        "14:50: the depot run under way"
    );
    assert_eq!(
        starting_trip(&trips, 15.0 * 3600.0 + 1800.0),
        1,
        "15:30: the 15:07 under way"
    );
    assert_eq!(
        starting_trip(&trips, 23.0 * 3600.0),
        2,
        "after the last: the last"
    );
    let now = 15.0 * 3600.0 + 300.0;
    let mut d = PlayerDuty {
        line: "5".into(),
        tour: "3".into(),
        trips,
        trip_index: 1,
        first_trip: 0,
        next_stop: 0,
        at_stop: false,
        arrived_late: None,
        done: false,
        served_terminus: None,
        left_late: None,
        held_back: false,
        placed: false,
        trip_changed: false,
        skipped: None,
        picked: false,
        first_update: None,
        heading: 0.0,
        position: None,
        jumped: false,
        skips: Default::default(),
    };
    // 200 m from the first stop two minutes before the departure: early, next stop the first
    assert_eq!(d.advance(glam::DVec3::new(300.0, 0.0, 0.0), now), None);
    assert_eq!(d.next_stop, 0);
    assert!((d.delay(now) + 120.0).abs() < 1e-9);
    // at the first stop, leaving a minute late
    d.advance(glam::DVec3::new(500.0, 0.0, 0.0), now + 60.0);
    assert!(d.at_stop);
    assert_eq!(
        d.advance(glam::DVec3::new(560.0, 0.0, 0.0), 54480.0)
            .map(|(_, left)| left),
        Some(60.0)
    );
    assert_eq!(d.next_stop, 1);
    // on the way the delay is what it left with until the next stop is overdue
    assert!((d.delay(55000.0) - 60.0).abs() < 1e-9);
    assert!((d.delay(56600.0) - 200.0).abs() < 1e-9);
    // the next trip does not take over while this one is driven ...
    d.advance(glam::DVec3::new(800.0, 0.0, 0.0), 57620.0);
    assert_eq!(d.trip_index, 1);
    // ... but once its end is reached
    d.advance(glam::DVec3::new(1000.0, 0.0, 0.0), 57630.0);
    assert!(d.done && !d.take_trip_change());
    d.advance(glam::DVec3::new(1000.0, 0.0, 0.0), 57640.0);
    assert_eq!((d.trip_index, d.next_stop), (2, 0));
    assert!(d.take_trip_change());
}

#[test]
fn a_duty_starts_with_a_trip_the_bus_can_reach() {
    let trips = vec![
        planned(
            54420.0,
            &[(500.0, 54420.0, 54420.0), (1000.0, 56400.0, 56400.0)],
        ),
        planned(
            57660.0,
            &[(1000.0, 57660.0, 57660.0), (500.0, 59400.0, 59400.0)],
        ),
    ];
    let duty = |trips: Vec<PlannedTrip>| PlayerDuty {
        line: "5".into(),
        tour: "3".into(),
        trips,
        trip_index: 0,
        first_trip: 0,
        next_stop: 0,
        at_stop: false,
        arrived_late: None,
        done: false,
        served_terminus: None,
        left_late: None,
        held_back: false,
        placed: false,
        trip_changed: false,
        skipped: None,
        picked: false,
        first_update: None,
        heading: 0.0,
        position: None,
        jumped: false,
        skips: Default::default(),
    };
    // 4 km away two minutes before the 15:07 leaves: the duty begins with the 16:01
    let mut d = duty(trips.clone());
    d.advance(glam::DVec3::new(-3500.0, 0.0, 0.0), 54300.0);
    assert_eq!((d.trip_index, d.next_stop), (1, 0));
    assert!(d.delay(54300.0) < -3000.0, "early for the 16:01");
    // under way already and nothing later: the last trip, from its first stop the bus
    // can still make on time
    let mut d = duty(trips[1..].to_vec());
    d.advance(glam::DVec3::new(-3500.0, 0.0, 0.0), 58000.0);
    assert_eq!((d.trip_index, d.next_stop), (0, 1));
    // ... or from its first when none can be
    let mut d = duty(trips[1..].to_vec());
    d.advance(glam::DVec3::new(-3500.0, 0.0, 0.0), 59000.0);
    assert_eq!((d.trip_index, d.next_stop), (0, 0));
}

#[test]
fn ibis_skips_a_service_leg_for_the_player_display() {
    let mut service = planned(100.0, &[(0.0, 100.0, 100.0), (100.0, 200.0, 200.0)]);
    service.line.clear();
    service.terminus = "Betriebsfahrt".into();
    let mut passenger = planned(300.0, &[(100.0, 300.0, 300.0), (200.0, 400.0, 400.0)]);
    passenger.line = "5E".into();
    let d = PlayerDuty {
        line: "5E".into(),
        tour: "1".into(),
        trips: vec![service, passenger],
        trip_index: 0,
        first_trip: 0,
        next_stop: 1,
        at_stop: false,
        arrived_late: None,
        done: false,
        served_terminus: None,
        left_late: None,
        held_back: false,
        placed: false,
        trip_changed: false,
        skipped: None,
        picked: false,
        first_update: None,
        heading: 0.0,
        position: None,
        jumped: false,
        skips: Default::default(),
    };
    let (trip, stop) = d.trip_for_ibis();
    assert_eq!(trip.line, "5E");
    assert_eq!(trip.terminus, "T");
    assert_eq!(stop, 0);
}

#[test]
fn the_player_picks_the_trip_to_start_with() {
    let trips = vec![
        planned(4.0 * 3600.0 + 7.0 * 60.0, &[(0.0, 0.0, 0.0)]),
        planned(4.0 * 3600.0 + 22.0 * 60.0, &[(0.0, 0.0, 0.0)]),
        planned(4.0 * 3600.0 + 37.0 * 60.0, &[(0.0, 0.0, 0.0)]),
    ];
    assert_eq!(chosen_trip(&trips, "04:22"), Some(1));
    assert_eq!(chosen_trip(&trips, "4:30"), Some(2), "the next one leaving");
    assert_eq!(chosen_trip(&trips, "05:00"), None);
    assert_eq!(chosen_trip(&trips, "1"), Some(0));
    assert_eq!(chosen_trip(&trips, "3"), Some(2));
    assert_eq!(chosen_trip(&trips, "4"), None);
}

#[test]
fn bays() {
    // the stop's box offset is kept as it is until the vehicle is known
    for lat in [0.0, 2.0, -4.0] {
        assert_eq!(bay_offset(lat), lat);
    }
}
