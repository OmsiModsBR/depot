//! The depot file (`.hof`) a start uses when none was picked by hand.

use super::super::domain::choice::Choice;
use super::super::interface::output::{depot_names_with_line, MapInfo, VehicleInfo};

/// The depot file for the chosen bus `bus` on the chosen map `map` (OMSI 2 in `root`): the
/// one the map's own buses use on the chosen date (the chrono scenarios change it: Berlin's
/// 1994 depot has line 137 where 1986's had 92), which the bus has, else its first. With a
/// line chosen, a depot file of the bus that carries it comes first. Without a bus, the
/// map's own.
pub fn default_hof(
    root: &str,
    choice: &Choice,
    map: Option<&MapInfo>,
    bus: Option<&VehicleInfo>,
) -> String {
    let on_date = map.and_then(|m| {
        let dir = omsi_cfg::resolve_path(std::path::Path::new(root), &m.file);
        omsi_map::ailists::depot_hof_on(dir.parent()?, omsi_map::ailists::date_code(&choice.date)?)
    });
    let want = on_date
        .or_else(|| map.map(|m| m.hof.clone()))
        .unwrap_or_default();
    let Some(v) = bus else { return want };
    // (the bus's own depot of the same place before the map's borrowed from another
    // bus, and one named like the map before its first, #896)
    let names: Vec<&str> = v.hofs.iter().map(|h| h.as_str()).collect();
    let like =
        |hints: &[&str]| omsi_vehicle::hof::closest_name(&names, hints).map(|i| v.hofs[i].clone());
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
        .or_else(|| like(&[want.as_str()]))
        .or(Some(want.clone()).filter(|w| !w.is_empty()))
        .or_else(|| like(&map_hints))
        .or_else(|| v.hofs.first().cloned())
        .unwrap_or_default()
}
