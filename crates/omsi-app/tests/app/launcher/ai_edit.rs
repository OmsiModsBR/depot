use omsi_map::ailists::DepotEntry;
use omsi_map::ailists_edit::AiListsDoc;
use omsi_map::ailists_edit::ParkListDoc;
use omsi_map::backup;
use omsi_map::ttl_edit::TtlDoc;
use openomsi_game::testing::launcher::ai_edit::*;
use std::collections::HashMap;
use std::path::PathBuf;

const LISTS: &str =
    "[aigroup_2]\r\nCars\r\n\r\nVehicles\\A\\a.ovh\t2\r\nVehicles\\B\\b.ovh\t1\r\n[end]\r\n";

fn lists() -> Lists {
    Lists {
        ailists_file: "ailists.cfg".into(),
        ailists: AiListsDoc::from_bytes(LISTS.as_bytes()),
        parks: vec![Park {
            file: "parklist_p.txt".into(),
            index: 0,
            doc: ParkListDoc::from_bytes(b"P\\x.sco\r\nP\\x.sco\r\n"),
        }],
        ttls: vec![Ttl {
            file: "TTData/76.ttl".into(),
            doc: TtlDoc::from_bytes(b"[newtour]\r\n1\r\nCars\r\n0\r\n"),
        }],
    }
}

#[test]
fn the_changes_are_named_one_by_one() {
    let saved = lists();
    let mut now = saved.clone();
    assert!(changes(&saved, &now).is_empty());
    assert!(now.to_write(&saved).is_empty());
    now.ailists.set_weight(0, 0, 5.0).unwrap();
    now.ailists.remove_vehicle(0, 1).unwrap();
    now.ailists
        .add_vehicle(0, "Vehicles\\C\\c.ovh", 3.0)
        .unwrap();
    let g = now.ailists.add_group("Taxis").unwrap();
    now.ailists
        .add_vehicle(g, "Vehicles\\T\\t.ovh", 1.0)
        .unwrap();
    now.parks[0].doc.set_count("P\\x.sco", 1).unwrap();
    now.parks[0].doc.add("P\\y.sco").unwrap();
    let c = changes(&saved, &now);
    assert_eq!(
        c,
        [
            Change::Weight {
                group: "Cars".into(),
                file: "Vehicles\\A\\a.ovh".into(),
                from: 2.0,
                to: 5.0
            },
            Change::Added {
                group: "Cars".into(),
                file: "Vehicles\\C\\c.ovh".into(),
                weight: Some(3.0)
            },
            Change::Removed {
                group: "Cars".into(),
                file: "Vehicles\\B\\b.ovh".into()
            },
            Change::GroupAdded("Taxis".into()),
            Change::Added {
                group: "Taxis".into(),
                file: "Vehicles\\T\\t.ovh".into(),
                weight: Some(1.0)
            },
            Change::Parked {
                list: "parklist_p.txt".into(),
                file: "P\\x.sco".into(),
                from: 2,
                to: 1
            },
            Change::Parked {
                list: "parklist_p.txt".into(),
                file: "P\\y.sco".into(),
                from: 0,
                to: 1
            },
        ]
    );
    let files: Vec<String> = now.to_write(&saved).into_iter().map(|f| f.0).collect();
    assert_eq!(files, ["ailists.cfg", "parklist_p.txt"]);
    // a group taken out
    let mut fewer = saved.clone();
    fewer.ailists.remove_vehicle(0, 0).unwrap();
    fewer.ailists.remove_vehicle(0, 0).unwrap();
    fewer.ailists.remove_group(0).unwrap();
    assert!(changes(&saved, &fewer).contains(&Change::GroupRemoved("Cars".into())));
}

#[test]
fn undo_and_redo_walk_the_changes() {
    let lists = lists();
    let mut e = Editor {
        folder: "M".into(),
        name: "M".into(),
        map_dir: PathBuf::new(),
        now: lists.clone(),
        saved: lists,
        undo: Vec::new(),
        redo: Vec::new(),
        group: 0,
        vehicle: None,
        park: 0,
        car: None,
        line: 0,
        tour: 0,
        bus: None,
        dialog: None,
        vehicles: None,
        reading: None,
        exists: HashMap::new(),
        names: HashMap::new(),
    };
    e.act(|x| x.ailists.set_weight(0, 0, 4.0)).unwrap();
    // a change that changes nothing is not kept
    e.act(|x| x.ailists.set_weight(0, 0, 4.0)).unwrap();
    assert_eq!(e.undo.len(), 1);
    assert!(e.dirty());
    e.undo();
    assert!(!e.dirty());
    e.redo();
    assert_eq!(e.now.ailists.groups()[0].vehicles[0].weight, 4.0);
    assert!(e.act(|x| x.ailists.add_group("cars").map(|_| ())).is_err());
    assert_eq!(e.undo.len(), 1);
}

#[test]
fn a_map_folder_is_read_with_its_parking_lists() {
    let dir = std::env::temp_dir().join(format!("depot-ai-edit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("AIlists.cfg"), LISTS).unwrap();
    std::fs::write(dir.join("parklist_p_2.txt"), "b.sco\r\n").unwrap();
    std::fs::write(dir.join("parklist_p.txt"), "a.sco\r\n").unwrap();
    std::fs::write(dir.join("parklist_old.txt"), "c.sco\r\n").unwrap();
    let l = Lists::load(&dir);
    assert_eq!(l.ailists_file, "AIlists.cfg");
    assert_eq!(l.ailists.groups().len(), 1);
    let parks: Vec<(&str, usize)> = l.parks.iter().map(|p| (p.file.as_str(), p.index)).collect();
    assert_eq!(parks, [("parklist_p.txt", 0), ("parklist_p_2.txt", 2)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn buses_and_tours_are_named_and_written_with_their_files() {
    let saved = Lists {
        ailists: AiListsDoc::from_bytes(
            b"[aigroup_depot]\r\nBuses\r\nDepot\r\n\r\n[aigroup_depot_typgroup_2]\r\nV\\b.bus\r\n1\tB-1\tred\r\n2\tB-2\tred\r\n[end]\r\n",
        ),
        ..lists()
    };
    let mut now = saved.clone();
    let bus = |n: &str, plate: &str| DepotEntry {
        number: n.into(),
        registration: plate.into(),
        paint: "red".into(),
        from: None,
        to: None,
    };
    now.ailists.set_bus(0, 0, 0, &bus("1", "B-9")).unwrap();
    now.ailists.remove_bus(0, 0, 1).unwrap();
    now.ailists.add_bus(0, 0, &bus("3", "B-3")).unwrap();
    now.ttls[0].doc.set_group(0, "Buses").unwrap();
    let list: Vec<(String, String)> = changes(&saved, &now)
        .iter()
        .map(|c| {
            let (a, b, _) = change_row(c);
            (a, b)
        })
        .collect();
    assert_eq!(list.len(), 4, "{list:?}");
    // both files are written, the line under its folder
    let files: Vec<String> = now.to_write(&saved).into_iter().map(|f| f.0).collect();
    assert_eq!(files, ["ailists.cfg", "TTData/76.ttl"]);
    assert!(backup::allowed("TTData/76.ttl"));
}
