//! The content index (`omsi_launcher_lib::index`): what the lists of maps, buses and weathers
//! are built from, cached per folder.

use omsi_launcher_lib::{index, vehicle_folder_key, VEHICLE_KEYS};
use std::cell::Cell;
use std::path::PathBuf;

/// The save after a reading of the vehicle folders drops the entries of the folders that are
/// gone: a folder deleted (or renamed) must not stay in the cache, nor its paint folders in the
/// stamp the page polls.
#[test]
fn the_vehicle_save_drops_the_folders_that_are_gone() {
    // (the cache lives in the data folder under the home folder: a home of its own, before
    // anything of the index is touched)
    let home = std::env::temp_dir().join(format!("depot-index-test-{}", std::process::id()));
    std::fs::create_dir_all(&home).unwrap();
    std::env::set_var("HOME", &home);
    std::env::set_var("USERPROFILE", &home);

    let gone = vehicle_folder_key("ENG", &[PathBuf::from("/content/Vehicles/Gone")]);
    let stays = vehicle_folder_key("ENG", &[PathBuf::from("/content/Vehicles/Stays")]);
    let read = Cell::new(0);
    let make = || {
        read.set(read.get() + 1);
        (1u32, Vec::new())
    };
    index::cached(&gone, 1, make);
    index::cached(&stays, 1, make);
    assert_eq!(read.get(), 2);

    index::save(VEHICLE_KEYS, Some(std::slice::from_ref(&stays)));

    // the folder still there comes from the cache, the one gone is read anew
    index::cached(&stays, 1, make);
    assert_eq!(read.get(), 2);
    index::cached(&gone, 1, make);
    assert_eq!(read.get(), 3);

    let _ = std::fs::remove_dir_all(&home);
}
