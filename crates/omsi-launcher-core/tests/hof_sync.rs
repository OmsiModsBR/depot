//! The depot files of the root `hof`/`hofs` folders reaching every playable vehicle without
//! the OMSI 2 folder being written to.

use omsi_launcher_lib::hof_sync::*;
use std::path::{Path, PathBuf};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("depot-hofsync-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn put(p: &Path, text: &str) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

fn tree(d: &Path) -> Vec<String> {
    fn walk(d: &Path, out: &mut Vec<String>) {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out)
            } else {
                out.push(format!(
                    "{}={}",
                    p.display(),
                    std::fs::read_to_string(&p).unwrap()
                ));
            }
        }
    }
    let mut v = Vec::new();
    walk(d, &mut v);
    v.sort();
    v
}

#[test]
fn sources_come_from_hof_and_hofs_folders_in_any_case() {
    let root = tmp("sources");
    put(&root.join("HOFs/A.hof"), "a");
    put(&root.join("hof/b.HOF"), "b");
    put(&root.join("Hof/c.hof"), "c");
    put(&root.join("HOFs/notes.txt"), "x");
    put(&root.join("HOFs/sub/deep.hof"), "x");
    put(&root.join("Other/d.hof"), "x");
    let names: Vec<String> = source_files(&[root.clone()])
        .iter()
        .map(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .to_ascii_lowercase()
        })
        .collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(sorted, ["a.hof", "b.hof", "c.hof"]);
    // a missing folder is no error
    assert!(source_files(&[root.join("nope")]).is_empty());
}

#[test]
fn the_omsi_folder_is_never_written_and_the_overlay_gets_the_file() {
    let omsi = tmp("omsi");
    let depot = tmp("depot");
    put(&omsi.join("Vehicles/Bus/bus.bus"), "bus");
    put(&omsi.join("Vehicles/Bus/Own.hof"), "own");
    put(&omsi.join("hofs/Map.hof"), "map");
    put(&omsi.join("hofs/Own.hof"), "root own");
    let before = tree(&omsi);
    let sources = source_files(&[depot.clone(), omsi.clone()]);
    let v = VehicleDir {
        overlay: depot.join("Vehicles/Bus"),
        existing: vec![omsi.join("Vehicles/Bus")],
    };
    let s = sync(&sources, &[v.clone()]);
    assert_eq!((s.created, s.failed), (2, 0));
    assert_eq!(tree(&omsi), before);
    assert_eq!(
        std::fs::read_to_string(depot.join("Vehicles/Bus/Map.hof")).unwrap(),
        "map"
    );
    // the overlay's own wins over the vehicle's differing one
    assert_eq!(
        std::fs::read_to_string(depot.join("Vehicles/Bus/Own.hof")).unwrap(),
        "root own"
    );
    // a second run changes nothing
    let after = tree(&depot);
    let s = sync(&sources, &[v]);
    assert_eq!((s.created, s.updated, s.identical), (0, 0, 2));
    assert_eq!(tree(&depot), after);
}

#[test]
fn a_file_the_vehicle_has_already_is_not_copied() {
    let omsi = tmp("same-omsi");
    let depot = tmp("same-depot");
    put(&omsi.join("Vehicles/Bus/Map.hof"), "map");
    put(&omsi.join("HOFs/map.hof"), "map");
    let s = sync(
        &source_files(&[omsi.clone()]),
        &[VehicleDir {
            overlay: depot.join("Vehicles/Bus"),
            existing: vec![omsi.join("Vehicles/Bus")],
        }],
    );
    assert_eq!((s.created, s.identical), (0, 1));
    assert!(!depot.join("Vehicles").exists());
}

#[test]
fn folders_shared_by_several_buses_are_done_once_and_a_newer_source_updates() {
    let depot = tmp("upd-depot");
    let src = tmp("upd-src");
    put(&src.join("hof/M.hof"), "new");
    put(&depot.join("Vehicles/Bus/m.hof"), "old");
    // an overlay file as new as the source stays (it may be someone's edit)
    let v = VehicleDir {
        overlay: depot.join("Vehicles/Bus"),
        existing: vec![],
    };
    let sources = source_files(&[src.clone()]);
    let s = sync(&sources, &[v.clone(), v.clone()]);
    assert_eq!((s.vehicles, s.kept, s.updated), (1, 1, 0));
    std::thread::sleep(std::time::Duration::from_millis(50));
    put(&src.join("hof/M.hof"), "newer");
    let s = sync(&sources, &[v]);
    assert_eq!((s.updated, s.kept), (1, 0));
    // (it keeps the spelling of the file that was there)
    assert_eq!(
        std::fs::read_to_string(depot.join("Vehicles/Bus/m.hof")).unwrap(),
        "newer"
    );
}

#[cfg(unix)]
#[test]
fn a_read_only_target_is_counted_not_fatal() {
    use std::os::unix::fs::PermissionsExt;
    let depot = tmp("ro-depot");
    let src = tmp("ro-src");
    put(&src.join("hof/M.hof"), "x");
    let dir = depot.join("Vehicles/Bus");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    let s = sync(
        &source_files(&[src]),
        &[VehicleDir {
            overlay: dir.clone(),
            existing: vec![],
        }],
    );
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    // (running as root can write anyway)
    assert_eq!(s.created + s.failed, 1);
}
