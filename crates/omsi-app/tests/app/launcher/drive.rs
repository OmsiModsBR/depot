use openomsi_game::testing::launcher::drive::*;

fn vehicle(folder: &str, maker: &str, name: &str, file: &str) -> omsi_launcher_lib::VehicleInfo {
    omsi_launcher_lib::VehicleInfo {
        name: format!("{maker} {name}"),
        manufacturer: maker.into(),
        type_name: name.into(),
        folder: folder.into(),
        file: format!("Vehicles/{folder}/{file}.bus"),
        description: String::new(),
        paints: vec!["Paint".into()],
        hofs: vec![],
        installed: false,
        missing_packs: vec![],
        numbers: vec![],
        default_paint: "Beige".into(),
    }
}

#[test]
fn a_tour_search_finds_the_next_trip_of_a_route_and_skips_the_ones_gone() {
    let trip = |index: usize, name: &str, departure: f64| omsi_launcher_lib::TripInfo {
        name: name.into(),
        index,
        line: "9106".into(),
        from: "Massy".into(),
        terminus: "Cormeilles".into(),
        departure,
        arrival: departure + 1800.0,
        stops: Vec::new(),
        km: 10.0,
    };
    let tour = |trips: Vec<omsi_launcher_lib::TripInfo>| omsi_launcher_lib::TourInfo {
        number: "12".into(),
        ai_group: String::new(),
        first: 0.0,
        last: 0.0,
        days: "Mon-Fri".into(),
        runs: true,
        next_run: None,
        trips,
    };
    let h = |x: f64| x * 3600.0;
    let morning_b = tour(vec![
        trip(1, "9106B_HC_MASSY", h(7.0)),
        trip(2, "9106A_HC_CORMEILLES", h(8.0)),
        trip(3, "9106A_HC_MASSY", h(13.0)),
    ]);
    assert_eq!(tour_trip(&morning_b, h(12.0), "9106b"), (false, None));
    assert_eq!(tour_trip(&morning_b, h(12.0), ""), (false, Some(2)));
    let later_b = tour(vec![
        trip(1, "9106A_HC_MASSY", h(11.5)),
        trip(2, "9106B_HC_CORMEILLES", h(12.5)),
        trip(3, "9106B_HC_MASSY", h(14.0)),
    ]);
    assert_eq!(tour_trip(&later_b, h(12.0), "9106b"), (false, Some(1)));
    assert_eq!(tour_trip(&later_b, h(12.0), "12"), (false, Some(1)));
    assert_eq!(tour_trip(&later_b, h(15.0), ""), (true, Some(2)));
    assert_eq!(tour_trip(&later_b, h(15.0), "9106b"), (true, Some(1)));
}

#[test]
fn empty_vehicle_types_use_the_file_name() {
    for empty in ["", "   "] {
        let manufacturers = build_bus_manufacturers(
            &[vehicle("Pack", "MAN", empty, "NL_202")],
            None,
            &Default::default(),
        );
        assert_eq!(manufacturers[0].variants[0].variant, "NL 202");
    }
}

#[test]
fn omsi_manufacturer_groups_dl_and_lions_city_across_packs() {
    let vehicles = vec![
        vehicle("MAN_DL05", "MAN", "DL05", "dl05"),
        vehicle("MAN_DL05", "MAN", "DL07", "dl07"),
        vehicle("MAN_DL05", "MAN", "DL08", "dl08"),
        vehicle("MAN_DL05", "MAN", "DL09", "dl09"),
        vehicle("MAN_LC_MVG", "MAN", "Lion's City (MVG)", "mvg"),
        vehicle("MAN_LC_GUE", "MAN", "Lion's City G (ORN)", "orn"),
    ];
    let manufacturers = build_bus_manufacturers(&vehicles, None, &Default::default());
    assert_eq!(manufacturers.len(), 1);
    assert_eq!(manufacturers[0].name, "MAN");
    assert_eq!(manufacturers[0].variants.len(), 6);
    assert!(manufacturers[0]
        .variants
        .iter()
        .any(|v| v.variant == "DL07"));
    assert!(manufacturers[0]
        .variants
        .iter()
        .any(|v| v.variant == "Lion's City G (ORN)"));
    assert!(manufacturer_matches(&manufacturers[0], "orn"));
    assert!(!manufacturer_matches(&manufacturers[0], "not a bus"));
}

#[test]
fn author_defined_manufacturer_and_complete_type_are_preserved() {
    let vehicles = vec![
        vehicle(
            "Pack",
            "Mercedes-Benz Release",
            "MB_C2_E6_GN_BVG_Leasing 2",
            "leasing",
        ),
        vehicle("Pack", "Mercedes-Benz", "O530", "o530"),
    ];
    let manufacturers = build_bus_manufacturers(&vehicles, None, &Default::default());
    assert_eq!(manufacturers.len(), 2);
    let maker = manufacturers
        .iter()
        .find(|m| m.name == "Mercedes-Benz Release")
        .unwrap();
    assert_eq!(maker.variants[0].variant, "MB C2 E6 GN BVG Leasing 2");
}

#[test]
fn duplicate_names_remain_selectable_and_host_filter_is_respected() {
    let vehicles = vec![
        vehicle("MAN", "MAN", "NL202", "en92"),
        vehicle("MAN", "MAN", "NL202", "en93"),
        vehicle("OtherPack", "MAN", "NL202", "en92"),
    ];
    let manufacturers = build_bus_manufacturers(&vehicles, None, &Default::default());
    let labels: std::collections::HashSet<_> = manufacturers[0]
        .variants
        .iter()
        .map(|v| &v.variant)
        .collect();
    assert_eq!(labels.len(), 3);
    let allowed = std::collections::HashSet::from([vehicles[1].file.to_lowercase()]);
    let filtered = build_bus_manufacturers(&vehicles, Some(&allowed), &Default::default());
    assert_eq!(filtered[0].variants.len(), 1);
    assert_eq!(filtered[0].variants[0].file, vehicles[1].file);
}

/// Each type in a family's dropdown says how many liveries it has, as the Livery list
/// beside counts them: its own paint and its repaints (#717).
#[test]
fn each_bus_type_says_how_many_liveries_it_has() {
    let mut one = vehicle("MAN_SD200", "MAN", "SD77", "sd77");
    one.paints.clear();
    let mut many = vehicle("MAN_SD200", "MAN", "SD78", "sd78");
    many.paints = vec!["BVG".into(), "Werbung".into(), "Neu".into()];
    let manufacturers = build_bus_manufacturers(&[one, many], None, &Default::default());
    let options: Vec<String> = manufacturers[0]
        .variants
        .iter()
        .map(variant_option)
        .collect();
    assert_eq!(
        options,
        vec![
            "SD77 · 1 livery".to_string(),
            "SD78 · 4 liveries".to_string()
        ]
    );
}

#[test]
fn numbers_in_type_names_sort_naturally_and_default_livery_has_its_omsi_name() {
    assert_eq!(bus_name_cmp("DL9", "DL10"), std::cmp::Ordering::Less);
    assert_eq!(bus_name_cmp("MAN", "man"), std::cmp::Ordering::Equal);
    let mut vehicle = vehicle("MAN", "MAN", "DL07", "dl07");
    assert_eq!(default_livery_label(&vehicle), "Beige");
    vehicle.default_paint.clear();
    assert_eq!(default_livery_label(&vehicle), "Default paint");
}
