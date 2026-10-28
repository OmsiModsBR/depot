use omsi_content::input::gamectrler::DeviceCfg;
use omsi_ui::Rect;
use openomsi_game::testing::launcher::keyboard::*;
use serde_json::json;
use serde_json::Value;

fn kb(vehicles: Value, game: Value) -> Value {
    json!({ "vehicles": vehicles, "game": game })
}

#[test]
fn a_scrolled_list_still_draws_down_to_its_foot() {
    let area = Rect::new(0.0, 100.0, 400.0, 300.0);
    // the content scrolled 1000 px up: the row that stands at the view's foot is drawn
    let rr = Rect::new(0.0, area.bottom() - 30.0, 400.0, 32.0);
    assert!(near_view(rr, area));
    assert!(!near_view(
        Rect::new(0.0, area.bottom() + 80.0, 400.0, 32.0),
        area
    ));
    assert!(!near_view(
        Rect::new(0.0, area.y - 120.0, 400.0, 32.0),
        area
    ));
}

#[test]
fn actions_fall_into_their_groups() {
    assert_eq!(GROUPS[group_of(0, "throttle")], "Driving");
    assert_eq!(GROUPS[group_of(0, "parking_brake_toggle")], "Driving");
    assert_eq!(
        GROUPS[group_of(0, "kw_scheinwerfer_toggle")],
        "Cab and lights"
    );
    assert_eq!(GROUPS[group_of(0, "bus_doorfront0")], "Doors and tickets");
    assert_eq!(GROUPS[group_of(0, "ticket_give")], "Doors and tickets");
    assert_eq!(GROUPS[group_of(1, "view_set_driver")], "Cameras and views");
    assert_eq!(GROUPS[group_of(1, "sim_pause")], "Game and menus");
    assert_eq!(GROUPS[group_of(1, "vr_recenter")], "VR");
    assert_eq!(GROUPS[group_of(1, "door_1")], "Doors and tickets");
    assert_eq!(GROUPS[group_of(0, "ASS_toggle")], "Advanced");
}

/// An action's entries make one row (its key and second key); a key two actions share is
/// flagged on both; a key that differs from the original is "changed", one only the Depot
/// has never is.
#[test]
fn rows_flag_shared_and_changed_keys() {
    let now = kb(
        json!([{"action":"horn","scan_code":35,"modifier":1},{"action":"horn","scan_code":36,"modifier":0},{"action":"parking_brake_toggle","scan_code":52,"modifier":0}]),
        json!([{"action":"sim_pause","scan_code":36,"modifier":0},{"action":"vr_recenter","scan_code":0,"modifier":0}]),
    );
    let orig = kb(
        json!([{"action":"horn","scan_code":35,"modifier":1},{"action":"parking_brake_toggle","scan_code":83,"modifier":0}]),
        json!([{"action":"sim_pause","scan_code":36,"modifier":0}]),
    );
    let r = rows(&now, Some(&orig), None, &|a| a.to_string());
    let get = |a: &str| r.iter().find(|x| x.action == a).unwrap();
    assert_eq!(get("horn").entries, vec![0, 1]);
    assert!(get("horn").shared && get("sim_pause").shared);
    assert!(!get("parking_brake_toggle").shared);
    assert!(get("parking_brake_toggle").changed);
    assert!(get("horn").changed, "a second key the original has not");
    assert!(!get("sim_pause").changed);
    assert!(!get("vr_recenter").changed && get("vr_recenter").unassigned());
    assert!(
        get("vr_recenter").in_list(10)
            && get("horn").in_list(8)
            && get("parking_brake_toggle").in_list(9)
    );
    // (OMSI 2's own layout shares keys on purpose: the manual and the automatic R)
    let stock = kb(
        json!([{"action":"kw_s_R","scan_code":19,"modifier":0},{"action":"automatic_R","scan_code":19,"modifier":0}]),
        json!([]),
    );
    assert!(rows(&stock, None, None, &|a| a.to_string())
        .iter()
        .all(|r| !r.shared));
    // (the factory's own keys: `keyboard_reset.cfg` when OMSI 2 has one)
    let factory = kb(
        json!([{"action":"parking_brake_toggle","scan_code":52,"modifier":0}]),
        json!([]),
    );
    let r = rows(&now, Some(&orig), Some(&factory), &|a| a.to_string());
    let get = |a: &str| r.iter().find(|x| x.action == a).unwrap();
    assert!(!get("parking_brake_toggle").off_factory);
    assert!(
        !get("horn").off_factory,
        "the factory file has no horn: nothing to go back to"
    );
}

/// Reloading gives every action the original has its keys (the held bit with them) and
/// keeps the Depot's own; the counts say what happens.
#[test]
fn a_reload_takes_the_originals_keys_and_keeps_the_depots_own() {
    let now = kb(
        json!([{"action":"throttle","scan_code":72,"modifier":1},{"action":"parking_brake_toggle","scan_code":52,"modifier":0}]),
        json!([{"action":"vr_recenter","scan_code":19,"modifier":0}]),
    );
    let orig = kb(
        json!([{"action":"throttle","scan_code":72,"modifier":1},{"action":"parking_brake_toggle","scan_code":83,"modifier":0},{"action":"parking_brake_toggle","scan_code":25,"modifier":4}]),
        json!([]),
    );
    let plan = reload_plan(&now, &orig, &|a| a.to_string());
    assert_eq!(
        (plan.identical, plan.changes.len(), plan.depot_only),
        (1, 1, 1)
    );
    let v = plan.merged["vehicles"].as_array().unwrap();
    assert_eq!(v.len(), 3);
    assert_eq!(v[1]["scan_code"], 83);
    assert_eq!(v[2]["modifier"], 4);
    assert_eq!(plan.merged["game"][0]["action"], "vr_recenter");
    assert_eq!(plan.changes[0].0, "parking_brake_toggle");
    assert_eq!(plan.changes[0].2, vec![(83, 0), (25, 4)]);
}

/// The Depot's own actions (its gear keys, VR) are not in the original's file: a reload
/// counts them as the Depot's and leaves their keys as the player set them.
#[test]
fn a_reload_keeps_the_depots_own_actions_as_they_are() {
    let now = kb(
        json!([{"action":"horn","scan_code":35,"modifier":0},{"action":"gear_up","scan_code":30,"modifier":0}]),
        json!([{"action":"vr_recenter","scan_code":30,"modifier":0},{"action":"sim_pause","scan_code":25,"modifier":0}]),
    );
    // (as `original_keybindings` reads it: the file's actions only)
    let orig = kb(
        json!([{"action":"horn","scan_code":35,"modifier":0}]),
        json!([{"action":"sim_pause","scan_code":25,"modifier":0}]),
    );
    let plan = reload_plan(&now, &orig, &|a| a.to_string());
    assert_eq!(
        (plan.identical, plan.changes.len(), plan.depot_only),
        (2, 0, 2)
    );
    assert_eq!(plan.merged, now);
    let r = rows(&now, Some(&orig), None, &|a| a.to_string());
    assert!(r.iter().all(|x| !x.changed));
}

/// The held bit is part of what a reload makes the same.
#[test]
fn a_reload_takes_the_held_bit_too() {
    let now = kb(
        json!([{"action":"throttle","scan_code":72,"modifier":0}]),
        json!([]),
    );
    let orig = kb(
        json!([{"action":"throttle","scan_code":72,"modifier":1}]),
        json!([]),
    );
    let plan = reload_plan(&now, &orig, &|a| a.to_string());
    assert_eq!(plan.changes.len(), 1);
    assert_eq!(plan.merged["vehicles"][0]["modifier"], 1);
}

/// The original's controllers replace the Depot's of their name; the Depot's others stay.
#[test]
fn the_originals_controllers_go_in_by_name() {
    let dev = |name: &str| DeviceCfg {
        name: name.into(),
        ..Default::default()
    };
    let mut own = vec![dev("Logitech G29"), dev("Thrustmaster pedals")];
    own[0].ff_scale = Some((50.0, 50.0));
    merge_devices(&mut own, vec![dev("LOGITECH G29"), dev("Saitek panel")]);
    let names: Vec<&str> = own.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(
        names,
        ["LOGITECH G29", "Thrustmaster pedals", "Saitek panel"]
    );
    assert_eq!(own[0].ff_scale, None);
}

/// The game's key names (`.kyb`) name a key, with its Shift / Ctrl / Alt; a key the table
/// lacks keeps the Depot's name.
#[test]
fn keys_are_named_from_the_language_table() {
    let names = KeyNames(vec![(72, "Num 8".into()), (42, "Shift (left)".into())]);
    assert_eq!(names.name((72, 0)), "Num 8");
    assert_eq!(
        names.name((72, omsi_content::input::KEY_CTRL as i64)),
        "Ctrl+Num 8"
    );
    assert_eq!(names.name((17, 0)), "W");
    assert_eq!(names.list(&[]), "none");
}

#[test]
fn the_factory_keys_are_omsi_2s_own() {
    assert_eq!(factory_keys(None, "parking_brake_toggle"), vec![(52, 0)]);
    assert!(factory_keys(None, "no_such_action").is_empty());
    assert_eq!(day(0), "01/01/1970");
    assert_eq!(day(1_717_200_000), "01/06/2024");
}
