//! The depot file (`.hof`) a start uses when none was picked by hand.

use super::super::domain::choice::Choice;
use super::super::interface::output::{depot_names_with_line, MapInfo, VehicleInfo};

/// The depot file for the chosen bus `bus` on the chosen map `map` (OMSI 2 in `root`): the
/// one the map's own buses use on the chosen date (the chrono scenarios change it: Berlin's
/// 1994 depot has line 137 where 1986's had 92), which the bus has, else its first (see below). With a
/// line chosen, a depot file of the bus that carries it comes first. Without a bus, the
/// map's own.
///
/// When the bus lacks the map's file the order is: the exact file in another vehicle folder
/// (the game borrows it, as it does for the AI buses), else the bus's first depot file. A
/// file with a merely similar name is never picked.
pub fn default_hof(
    root: &str,
    choice: &Choice,
    map: Option<&MapInfo>,
    bus: Option<&VehicleInfo>,
) -> String {
    pick(root, choice, map, bus, &|name| {
        omsi_vehicle::hof::depot_anywhere_exists(name)
    })
}

/// [`default_hof`] with `installed_elsewhere` telling whether a depot file of that exact name
/// is installed in any vehicle folder.
fn pick(
    root: &str,
    choice: &Choice,
    map: Option<&MapInfo>,
    bus: Option<&VehicleInfo>,
    installed_elsewhere: &dyn Fn(&str) -> bool,
) -> String {
    let on_date = map.and_then(|m| {
        let dir = omsi_cfg::resolve_path(std::path::Path::new(root), &m.file);
        omsi_map::ailists::depot_hof_on(dir.parent()?, omsi_map::ailists::date_code(&choice.date)?)
    });
    let want = on_date
        .or_else(|| map.map(|m| m.hof.clone()))
        .unwrap_or_default();
    let Some(v) = bus else { return want };
    // (the bus's own exact file first, then the same file in another vehicle folder, then
    // the bus's first: a similar name must not win over an installed exact file, #46)
    let map_hints: Vec<String> = map
        .map(|m| {
            vec![
                m.name.clone(),
                m.friendly.clone(),
                m.file
                    .trim_end_matches("/global.cfg")
                    .rsplit('/')
                    .next()
                    .unwrap_or("")
                    .to_string(),
            ]
        })
        .unwrap_or_default();
    let map_hints: Vec<&str> = map_hints.iter().map(|h| h.as_str()).collect();
    // a depot file carrying the line driven: the day's, one named like the map, else the fullest
    let with_line = match (&choice.line, choice.free) {
        (Some(line), false) => depot_names_with_line(std::path::Path::new(root), &v.file, line),
        _ => Vec::new(),
    };
    if !with_line.is_empty() && !with_line.iter().any(|h| h.eq_ignore_ascii_case(&want)) {
        let names: Vec<&str> = with_line.iter().map(|h| h.as_str()).collect();
        let hints: Vec<&str> = std::iter::once(want.as_str())
            .chain(map_hints.iter().copied())
            .collect();
        let pick = omsi_vehicle::hof::closest_name(&names, &hints).unwrap_or(0);
        return with_line[pick].clone();
    }
    v.hofs
        .iter()
        .find(|h| h.eq_ignore_ascii_case(&want))
        .cloned()
        .or_else(|| Some(want.clone()).filter(|w| !w.is_empty() && installed_elsewhere(w)))
        .or_else(|| v.hofs.first().cloned())
        .or_else(|| Some(want.clone()).filter(|w| !w.is_empty()))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(hof: &str) -> MapInfo {
        MapInfo {
            name: "Mapa SP Zona Oeste".into(),
            friendly: "Mapa SP Zona Oeste".into(),
            file: "maps/Mapa SP Zona Oeste/global.cfg".into(),
            description: String::new(),
            entry_points: Vec::new(),
            hof: hof.into(),
            installed: false,
        }
    }

    fn bus(hofs: &[&str]) -> VehicleInfo {
        VehicleInfo {
            name: "Bus".into(),
            manufacturer: String::new(),
            type_name: String::new(),
            file: "Vehicles/Bus/bus.bus".into(),
            folder: "Bus".into(),
            description: String::new(),
            default_paint: String::new(),
            paints: Vec::new(),
            hofs: hofs.iter().map(|h| h.to_string()).collect(),
            installed: false,
            missing_packs: Vec::new(),
            numbers: Vec::new(),
        }
    }

    fn run(m: &MapInfo, b: &VehicleInfo, elsewhere: &[&str]) -> String {
        let has = |n: &str| elsewhere.iter().any(|e| e.eq_ignore_ascii_case(n));
        pick("/nonexistent", &Choice::default(), Some(m), Some(b), &has)
    }

    /// The bus lacks the map's file, which is installed with another bus: it is borrowed,
    /// not replaced by a bus file that shares one word with the map's name.
    #[test]
    fn a_file_installed_elsewhere_beats_a_similar_name() {
        let got = run(
            &map("Mapa SP Zona Oeste"),
            &bus(&["Habana Oeste", "Mapa Centro"]),
            &["Mapa SP Zona Oeste"],
        );
        assert_eq!(got, "Mapa SP Zona Oeste");
    }

    /// The map's file is installed nowhere: the bus's first file, never a look-alike.
    #[test]
    fn a_missing_file_falls_back_to_the_first_of_the_bus() {
        let got = run(
            &map("Mapa SP Zona Oeste"),
            &bus(&["Habana Oeste", "Mapa Centro"]),
            &[],
        );
        assert_eq!(got, "Habana Oeste");
    }

    /// No map file given: the bus's first; a bus with no files: the map's name (as before).
    #[test]
    fn empty_cases() {
        assert_eq!(run(&map(""), &bus(&["A", "B"]), &[]), "A");
        assert_eq!(run(&map("M"), &bus(&[]), &[]), "M");
    }

    /// The bus has the file itself: unchanged, and no other folder is consulted.
    #[test]
    fn the_bus_own_file_is_kept() {
        let got = run(
            &map("mapa sp zona oeste"),
            &bus(&["Habana Oeste", "Mapa SP Zona Oeste"]),
            &["Other"],
        );
        assert_eq!(got, "Mapa SP Zona Oeste");
    }
}
