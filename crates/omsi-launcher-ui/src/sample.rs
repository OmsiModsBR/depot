//! Made-up vehicles for screenshots (`--sample`): the page is checked against a catalog that
//! needs no installation of the original game.

use omsi_launcher_lib::VehicleInfo;

fn bus(name: &str, maker: &str, kind: &str, installed: bool, paints: &[&str]) -> VehicleInfo {
    VehicleInfo {
        name: name.into(),
        manufacturer: maker.into(),
        type_name: kind.into(),
        file: format!("Vehicles/{name}/{name}.bus"),
        folder: name.into(),
        description: "Technical sheet\nEngine, power, gearbox, maximum speed, length and weight, as the author wrote them (example text).\n\nCredits\nModelling and conversion, as credited by the author (example text).".into(),
        default_paint: paints.first().map(|p| p.to_string()).unwrap_or_default(),
        // the first is the bus's own livery; the rest are repaints, as the catalog lists them
        paints: paints.iter().skip(1).map(|p| p.to_string()).collect(),
        hofs: vec!["Depot A".into(), "Depot B".into()],
        installed,
        missing_packs: Vec::new(),
        numbers: vec![("50044".into(), "XX-AB 123".into())],
    }
}

/// A few vehicles, the last one missing a pack it borrows from.
pub fn vehicles() -> Vec<VehicleInfo> {
    let mut last = bus(
        "Example bus C",
        "Maker Two",
        "Articulated bus",
        true,
        &["Default paint"],
    );
    last.missing_packs = vec!["Example pack".into()];
    last.hofs.clear();
    vec![
        bus(
            "Example bus A",
            "Maker One",
            "City bus",
            false,
            &["Default paint", "Paint 2", "Paint 3", "Paint 4"],
        ),
        bus(
            "Example bus B",
            "Maker Two",
            "City bus",
            true,
            &["Default paint", "Paint 2"],
        ),
        last,
    ]
}

/// A few maps; the last has no start point and no depot file.
pub fn maps() -> Vec<omsi_launcher_lib::MapInfo> {
    use omsi_launcher_lib::{EntryInfo, MapInfo};
    let entry = |i: i32, n: &str| EntryInfo {
        index: i,
        name: n.into(),
    };
    let map = |name: &str, friendly: &str, installed: bool, entries: Vec<EntryInfo>, hof: &str| {
        MapInfo {
        name: name.into(),
        friendly: friendly.into(),
        file: format!("maps/{name}/global.cfg"),
        description: "Example map. The author describes the area, the lines and the period here (example text).".into(),
        entry_points: entries,
        hof: hof.into(),
        installed,
    }
    };
    vec![
        map(
            "Example map",
            "Example map",
            false,
            vec![
                entry(0, "Central depot"),
                entry(1, "North terminus"),
                entry(2, "Harbour"),
            ],
            "Depot A",
        ),
        map(
            "Example city 2",
            "Example city 2",
            true,
            vec![entry(0, "Main station")],
            "Depot B",
        ),
        map("Example town", "Example town", true, Vec::new(), ""),
    ]
}
