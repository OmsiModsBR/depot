fn drive() -> omsi_launcher_lib::Session {
    serde_json::from_value(serde_json::json!({"time":0,"driver":"T","map":"maps/Spandau/global.cfg","bus":"Vehicles/X/a.bus","line":"136","tour":"2","seconds":1500.0,"metres":8400.0,"stops":14,"early":1,"late":2,"tickets":4,"cash":7.5,"crashes":0,"hurt":0,"jolts":2,"driving":96.0,"comfort":88.0,"ticketing":100.0})).unwrap()
}

/// The stops on time are the ones served neither early nor late (the personnel file
/// counts all of them, and of those the early and the late ones).
#[test]
fn a_drives_details_count_the_stops_on_time() {
    let rows = openomsi_game::testing::launcher::pages::drive_rows(&drive());
    let get = |k: &str| rows.iter().find(|r| r.0 == k).map(|r| r.1.clone()).unwrap();
    assert_eq!(get("On time / early / late"), "11 / 1 / 2");
    assert_eq!(get("Line"), "136 / 2");
    assert_eq!(get("Distance"), "8.4 km");
    assert_eq!(
        openomsi_game::testing::launcher::pages::drive_title(&drive()),
        "Line 136 / 2 · Spandau"
    );
}
