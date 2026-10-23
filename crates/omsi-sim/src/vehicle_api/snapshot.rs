//! The snapshot of a vehicle that the API serves: its motion, engine, electrics, doors,
//! passengers, lights, brakes, wipers, cabin and condition, read from the script variables.

use super::{map, num, opt, round_dp, ApiValue};

/// Everything the snapshot is made from. [`VehicleInstance::html_api_snapshot`] fills it from
/// a running vehicle; tests fill it by hand.
pub struct Inputs<'a> {
    /// A script variable by name (any letter case), `None` when the bus has none.
    pub var: &'a dyn Fn(&str) -> Option<f32>,
    /// A text variable by name (empty when the bus has none).
    pub text: &'a dyn Fn(&str) -> String,
    pub speed_kmh: f32,
    pub steer_deg: f32,
    pub heading: f64,
    pub pitch: f32,
    pub bank: f32,
    pub position: (f64, f64, f64),
    pub engine_running: bool,
    pub interior_light: f32,
    pub crashes: u32,
    pub last_impact_j: f32,
    pub dirt: f32,
    pub trailers: usize,
}

/// Most doors of a bus the snapshot lists, and the most boarding doors (`PAX_Entry0..7`).
const MAX_DOORS: usize = 16;
const MAX_PAX_DOORS: usize = 8;

/// A script variable's type, as the snapshot reads the bus's variables.
type Var<'a> = &'a dyn Fn(&str) -> Option<f32>;

/// A switch: the variable past half, and off when the bus has none of it.
fn switch(var: Var<'_>, name: &str) -> bool {
    var(name).unwrap_or(0.0) > 0.5
}

/// Whether any of the switches is on.
fn any_switch(var: Var<'_>, names: &[&str]) -> bool {
    names.iter().any(|n| switch(var, n))
}

/// The first of some alternative names that the bus has.
fn first_var(var: Var<'_>, names: &[&str]) -> Option<f32> {
    names.iter().find_map(|n| var(n))
}

/// A switch that reads as `null` when the bus has none of the variables.
fn opt_switch(var: Var<'_>, names: &[&str]) -> Option<bool> {
    first_var(var, names).map(|v| v > 0.5)
}

fn info(i: &Inputs) -> ApiValue {
    map(vec![
        ("number", ApiValue::Str((i.text)("number"))),
        ("ident", ApiValue::Str((i.text)("ident"))),
        ("yard", ApiValue::Str((i.text)("yard"))),
        ("route", ApiValue::Str((i.text)("act_route"))),
        ("nextStop", ApiValue::Str((i.text)("act_busstop"))),
    ])
}

fn motion(i: &Inputs) -> ApiValue {
    let var = i.var;
    let odometer =
        var("kmcounter_km").map(|km| km as f64 + var("kmcounter_m").unwrap_or(0.0) as f64 / 1000.0);
    map(vec![
        ("speedKmh", num(i.speed_kmh, 1)),
        ("heading", ApiValue::Num(round_dp(i.heading, 1))),
        ("pitch", num(i.pitch, 2)),
        ("bank", num(i.bank, 2)),
        ("steeringDeg", num(i.steer_deg, 1)),
        ("x", ApiValue::Num(round_dp(i.position.0, 2))),
        ("y", ApiValue::Num(round_dp(i.position.1, 2))),
        ("z", ApiValue::Num(round_dp(i.position.2, 2))),
        (
            "odometerKm",
            odometer.map_or(ApiValue::Null, |v| ApiValue::Num(round_dp(v, 3))),
        ),
    ])
}

/// `engine_n` is in rpm.
fn engine(i: &Inputs, var: Var<'_>) -> ApiValue {
    map(vec![
        ("running", ApiValue::Bool(i.engine_running)),
        (
            "rpm",
            opt(
                first_var(var, &["engine_n", "engine_rpm", "motor_n", "motor_rpm"]),
                0,
            ),
        ),
        ("throttle", opt(var("throttle"), 2)),
        ("brake", opt(var("brake"), 2)),
        ("clutch", opt(var("clutch"), 2)),
        (
            "gear",
            opt(first_var(var, &["antrieb_getr_aktugang", "gear"]), 0),
        ),
        ("tankContent", opt(var("engine_tank_content"), 1)),
    ])
}

fn electrics(var: Var<'_>) -> ApiValue {
    let busbar_main = switch(var, "elec_busbar_main");
    let busbar_avail = switch(var, "elec_busbar_avail");
    map(vec![
        ("on", ApiValue::Bool(busbar_main || busbar_avail)),
        ("busbarMain", ApiValue::Bool(busbar_main)),
        ("busbarAvailable", ApiValue::Bool(busbar_avail)),
        (
            "failure",
            ApiValue::Bool(switch(var, "elec_failure_general")),
        ),
    ])
}

fn battery(var: Var<'_>) -> ApiValue {
    map(vec![(
        "on",
        opt_switch(var, &["elec_battery_on", "battery_on", "batterie_on"])
            .map_or(ApiValue::Null, ApiValue::Bool),
    )])
}

/// The doors and the passengers' boarding, as the page's hints read them: a door list
/// (`door_0`, `door_1` ... as far as the bus has them; 0 shut, 1 open) and the boarding
/// doors (`PAX_Entry<n>_Open/_Req`, `PAX_Exit<n>_Open/_Req`).
fn doors_and_boarding(var: Var<'_>) -> (ApiValue, ApiValue) {
    let door_list: Vec<ApiValue> = (0..MAX_DOORS)
        .map_while(|n| var(&format!("door_{n}")).map(|open| (n, open)))
        .map(|(n, open)| {
            map(vec![
                ("number", ApiValue::Num((n + 1) as f64)),
                ("open", num(open, 2)),
                ("isOpen", ApiValue::Bool(open > 0.05)),
            ])
        })
        .collect();
    let door_open = |n: usize| var(&format!("door_{n}")).unwrap_or(0.0) > 0.05;

    let pax = |kind: &str| -> Vec<ApiValue> {
        (0..MAX_PAX_DOORS)
            .filter_map(|n| {
                let open = var(&format!("PAX_{kind}{n}_Open"));
                let req = var(&format!("PAX_{kind}{n}_Req"));
                if open.is_none() && req.is_none() {
                    return None;
                }
                Some(map(vec![
                    ("number", ApiValue::Num((n + 1) as f64)),
                    ("open", ApiValue::Bool(open.unwrap_or(0.0) > 0.5)),
                    ("requested", ApiValue::Bool(req.unwrap_or(0.0) > 0.5)),
                ]))
            })
            .collect()
    };
    let entries = pax("Entry");
    let exits = pax("Exit");
    let pax_open = entries
        .iter()
        .chain(exits.iter())
        .any(|d| d.get("open") == Some(&ApiValue::Bool(true)));
    // what the passengers are told is open, where the bus has that; else the door leaves
    // (the same rule the game's own hints use)
    let any_open = if entries.is_empty() && exits.is_empty() {
        (0..door_list.len()).any(door_open)
    } else {
        pax_open
    };
    let doors = map(vec![
        ("count", ApiValue::Num(door_list.len() as f64)),
        ("anyOpen", ApiValue::Bool(any_open)),
        ("list", ApiValue::List(door_list)),
    ]);
    let passengers = map(vec![
        ("onboard", opt(var("humans_count"), 0)),
        ("entries", ApiValue::List(entries)),
        ("exits", ApiValue::List(exits)),
    ]);
    (doors, passengers)
}

fn lights(i: &Inputs, var: Var<'_>) -> ApiValue {
    let headlights = if switch(var, "lights_fern") {
        3
    } else if switch(var, "lights_abbl")
        || switch(var, "lights_main")
        || var("Spot_Select").is_some_and(|s| s >= 0.0)
    {
        2
    } else if switch(var, "lights_stand") {
        1
    } else {
        0
    };
    let (lamp_l, lamp_r) = (
        switch(var, "lights_blinker_l"),
        switch(var, "lights_blinker_r"),
    );
    // the indicator switch where the script has one (0 off, 1 left, 2 right, 3 hazard),
    // else the lamps
    let indicator = if switch(var, "lights_sw_warnblinker") {
        3
    } else {
        match var("lights_sw_blinker") {
            Some(s) if (0.5..2.5).contains(&s) => s.round() as i32,
            _ => match (lamp_l, lamp_r) {
                (true, true) => 3,
                (true, false) => 1,
                (false, true) => 2,
                _ => 0,
            },
        }
    };
    map(vec![
        ("headlights", ApiValue::Num(headlights as f64)),
        ("brake", ApiValue::Bool(switch(var, "lights_brems"))),
        ("reverse", ApiValue::Bool(switch(var, "lights_rueckfahr"))),
        ("fog", ApiValue::Bool(switch(var, "lights_nebelschluss"))),
        ("indicator", ApiValue::Num(indicator as f64)),
        ("indicatorLeft", ApiValue::Bool(lamp_l)),
        ("indicatorRight", ApiValue::Bool(lamp_r)),
        ("hazard", ApiValue::Bool(indicator == 3)),
        ("interior", num(i.interior_light, 2)),
    ])
}

fn brakes(var: Var<'_>) -> ApiValue {
    map(vec![
        (
            "parking",
            ApiValue::Bool(any_switch(var, &["bremse_feststell", "parking_brake"])),
        ),
        (
            "stop",
            ApiValue::Bool(any_switch(
                var,
                &["bremse_halte", "bremse_halte_sw", "bus_stop_brake"],
            )),
        ),
        (
            "kneeling",
            ApiValue::Bool(any_switch(
                var,
                &["bremse_kneeling", "vdv_kneel", "ecas_kneel", "kneeling"],
            )),
        ),
    ])
}

fn wipers(var: Var<'_>) -> ApiValue {
    map(vec![(
        "running",
        ApiValue::Bool(any_switch(var, &["wiperrunning", "wiper_running"])),
    )])
}

fn condition(i: &Inputs, var: Var<'_>) -> ApiValue {
    map(vec![
        ("dirt", num(i.dirt, 2)),
        ("crashes", ApiValue::Num(i.crashes as f64)),
        ("lastImpactKJ", num(i.last_impact_j / 1000.0, 1)),
        ("streetCondition", opt(var("StreetCond"), 2)),
    ])
}

/// The snapshot of a vehicle, as the page reads it: the same keys whatever the bus has.
pub fn snapshot(i: &Inputs) -> ApiValue {
    let var = i.var;
    let (doors, passengers) = doors_and_boarding(var);
    map(vec![
        ("api", ApiValue::Num(1.0)),
        ("info", info(i)),
        ("motion", motion(i)),
        ("engine", engine(i, var)),
        ("electrics", electrics(var)),
        ("battery", battery(var)),
        ("doors", doors),
        ("passengers", passengers),
        ("lights", lights(i, var)),
        ("brakes", brakes(var)),
        ("wipers", wipers(var)),
        (
            "cabin",
            map(vec![("temperature", opt(var("Cabinair_Temp"), 1))]),
        ),
        ("condition", condition(i, var)),
        (
            "train",
            map(vec![("trailers", ApiValue::Num(i.trailers as f64))]),
        ),
    ])
}
