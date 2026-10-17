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
        paints: paints.iter().map(|p| p.to_string()).collect(),
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
