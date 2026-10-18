//! #46: which depot file (`.hof`) a bus gets for a map. Synthetic content only: tiny hand-written
//! files in a temporary content root, nothing from any real map or bus.

use omsi_vehicle::hof::{depot_anywhere, depot_files, depot_in};
use omsi_vehicle::Hof;
use std::path::{Path, PathBuf};

/// A temporary content root that is registered on creation and removed on drop.
struct Root(PathBuf);

impl Root {
    fn new(tag: &str) -> Root {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let p =
            std::env::temp_dir().join(format!("depot-t46-{tag}-{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(&p).unwrap();
        omsi_cfg::add_content_root(p.clone());
        Root(p)
    }

    fn dir(&self, rel: &str) -> PathBuf {
        let d = self.0.join(rel);
        std::fs::create_dir_all(&d).unwrap();
        d
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        omsi_cfg::remove_content_root(&self.0);
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn write(dir: &Path, file: &str, text: &str) {
    std::fs::write(dir.join(file), text).unwrap();
}

fn hof_text(name: &str, termini: &[(i32, &str, &str)]) -> String {
    let mut s = format!("[name]\r\n{name}\r\n\r\nstringcount_terminus\r\n1\r\n\r\n");
    for (code, id, text) in termini {
        s.push_str(&format!(
            "[addterminus]\r\n{code}\r\n{id}\r\n{text}\r\n\r\n"
        ));
    }
    s
}

/// The map names a depot that only another bus's folder carries (a mod bus made for a different
/// map): the bus gets it through the search of all vehicle folders, not its own folder.
#[test]
fn map_depot_in_another_vehicle_folder_is_found_anywhere_but_not_in_the_bus_folder() {
    let root = Root::new("elsewhere");
    let bus = root.dir("Vehicles/BusA");
    let other = root.dir("Vehicles/BusB");
    write(
        &bus,
        "own.hof",
        &hof_text("Own Depot T46a", &[(1, "A", "AAA")]),
    );
    write(
        &other,
        "mapdepot.hof",
        &hof_text("Map Depot T46a", &[(7, "M", "MMM")]),
    );

    assert!(
        depot_in(&bus, "Map Depot T46a").is_none(),
        "the bus folder does not carry it"
    );
    let h = depot_anywhere("Map Depot T46a").expect("found in the other vehicle folder");
    assert_eq!(h.name, "Map Depot T46a");
    assert_eq!(h.termini[0].code, 7);
}

/// A depot in the game root's `HOFs/` folder is offered to every bus.
#[test]
fn map_depot_in_the_shared_hofs_folder_is_found_from_any_bus() {
    let root = Root::new("shared");
    let bus = root.dir("Vehicles/BusA");
    let shared = root.dir("HOFs");
    write(
        &bus,
        "own.hof",
        &hof_text("Own Depot T46b", &[(1, "A", "AAA")]),
    );
    write(
        &shared,
        "mapdepot.hof",
        &hof_text("Map Depot T46b", &[(7, "M", "MMM")]),
    );

    assert_eq!(
        depot_in(&bus, "Map Depot T46b").map(|h| h.name),
        Some("Map Depot T46b".into())
    );
    assert!(depot_files(&bus)
        .iter()
        .any(|p| p.ends_with("mapdepot.hof")));
}

/// A bus with its own depot file keeps it: the map's name does not replace a file the bus has.
#[test]
fn a_bus_with_its_own_depot_file_keeps_it() {
    let root = Root::new("own");
    let bus = root.dir("Vehicles/BusA");
    write(
        &bus,
        "own.hof",
        &hof_text("Own Depot T46c", &[(1, "A", "AAA")]),
    );

    let h = depot_in(&bus, "Own Depot T46c").expect("own file by its [name]");
    assert_eq!(h.termini[0].code, 1);
    let by_stem = depot_in(&bus, "own").expect("own file by its file name");
    assert_eq!(by_stem.path, h.path);
}

/// The lookup by name ignores case, surrounding blanks and CRLF line ends, and does not need the
/// file name to match the `[name]`.
#[test]
fn depot_lookup_ignores_case_blanks_and_line_ends() {
    let root = Root::new("spelling");
    let bus = root.dir("Vehicles/BusA");
    write(
        &bus,
        "x.hof",
        "[name]\r\n  Map Depot T46d  \r\n\r\nstringcount_terminus\r\n0\r\n",
    );

    assert!(depot_in(&bus, "map depot t46d").is_some());
    assert!(depot_in(&bus, "  MAP DEPOT T46D ").is_some());
    assert!(depot_anywhere("map depot t46d").is_some());
}

/// Hypothesis from #46 (not confirmed, no reporter data yet): a depot file that sits in the map's
/// own folder, not in a vehicle folder or `HOFs/`. Nothing searches there today. Kept ignored
/// until the reporter's map shows where its depot file really is; delete or enable then.
#[test]
#[ignore = "hypothesis for #46: needs the real map layout from the reporter"]
fn depot_in_the_map_folder_is_found_for_a_bus() {
    let root = Root::new("mapfolder");
    let bus = root.dir("Vehicles/BusA");
    let map = root.dir("Maps/Synthetic");
    write(
        &bus,
        "own.hof",
        &hof_text("Own Depot T46e", &[(1, "A", "AAA")]),
    );
    write(
        &map,
        "mapdepot.hof",
        &hof_text("Map Depot T46e", &[(7, "M", "MMM")]),
    );

    assert!(
        depot_in(&bus, "Map Depot T46e").is_some() || depot_anywhere("Map Depot T46e").is_some()
    );
}

/// The related symptom in #46's comment: the destinations of a depot file are read in the file's
/// order, every one of them, in both record styles. (The game's menu then sorts them by name,
/// `game_lists.rs`; whether the original does too is an open question for #49.)
#[test]
fn every_destination_is_read_in_the_files_order() {
    let text = "[name]\r\nOrder T46\r\n\r\nstringcount_terminus\r\n2\r\n\r\n\
        [addterminus]\r\n30\r\nZulu\r\nZULU\r\nz\r\n\
        [addterminus]\r\n10\r\nAlpha\r\nALPHA\r\na\r\n\
        [addterminus_allexit]\r\n20\r\nMike\r\nMIKE\r\nm\r\n\
        [addterminus_list]\r\n\t5\tBravo\tBRAVO\tb\r\n\t6\tYankee\tYANKEE\ty\r\n[end]\r\n";
    let h = Hof::parse(&omsi_cfg::CfgFile::from_str("order.hof", text));
    let codes: Vec<i32> = h.termini.iter().map(|t| t.code).collect();
    assert_eq!(codes, vec![30, 10, 20, 5, 6]);
    let names: Vec<String> = h.termini.iter().map(|t| t.menu_name()).collect();
    assert_eq!(names, vec!["ZULU", "ALPHA", "MIKE", "BRAVO", "YANKEE"]);
    for t in &h.termini {
        assert_eq!(h.terminus_by_code(t.code).map(|x| x.code), Some(t.code));
    }
}

/// Destination names with accents, punctuation and digits survive the read unchanged, so a search
/// over the names can find them.
#[test]
fn destination_names_keep_accents_and_punctuation() {
    let text = hof_text(
        "Names T46",
        &[
            (1, "Łódź Główna", "ŁÓDŹ GŁÓWNA"),
            (2, "St. Mary's (Nord)", "ST. MARY'S"),
            (3, "12/14 Süd", "12/14 SÜD"),
        ],
    );
    let h = Hof::parse(&omsi_cfg::CfgFile::from_str("names.hof", &text));
    let ids: Vec<&str> = h.termini.iter().map(|t| t.texture_id.as_str()).collect();
    assert_eq!(ids, vec!["Łódź Główna", "St. Mary's (Nord)", "12/14 Süd"]);
    assert_eq!(h.termini[0].display_name(), "ŁÓDŹ GŁÓWNA");
}
