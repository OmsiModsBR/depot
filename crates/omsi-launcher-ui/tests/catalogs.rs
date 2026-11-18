//! The Vehicles and Maps pages' catalogs while the host reads the content lists.

use omsi_launcher_ui::maps::MapsView;
use omsi_launcher_ui::vehicles::VehiclesView;

/// While the host reads the lists the pages start no reading of their own (it read every
/// vehicle folder a second time at once); once it is done they may, as before.
#[test]
fn the_pages_wait_for_the_hosts_reading() {
    // (a reading the pages start looks for the launcher's settings under the home folder)
    let home = std::env::temp_dir().join(format!("depot-catalogs-test-{}", std::process::id()));
    std::fs::create_dir_all(&home).unwrap();
    std::env::set_var("HOME", &home);
    std::env::set_var("USERPROFILE", &home);

    let mut v = VehiclesView::default();
    v.set_host_reading(true);
    v.load();
    assert!(!v.busy());
    v.set_host_reading(false);
    v.load();
    assert!(v.busy());

    let mut m = MapsView::default();
    m.set_host_reading(true);
    m.load();
    assert!(!m.busy());
    m.set_host_reading(false);
    m.load();
    assert!(m.busy());
}

/// A reading again that finds as many buses as before, but other paints or another bus, is
/// what the Vehicles page shows: it took the host's list only when the number of buses
/// changed, and kept a bus without its paints and without the bus that came.
#[test]
fn the_vehicles_page_follows_a_list_read_again_with_as_many_buses() {
    let sample = omsi_launcher_ui::sample::vehicles();
    let mut bare = sample[0].clone();
    bare.paints.clear();
    let mut painted = sample[0].clone();
    painted.paints = vec!["SP 2000 - VCB".into()];
    let first = vec![bare, sample[1].clone()];
    let again = vec![painted, sample[2].clone()];

    let mut v = VehiclesView::default();
    v.follow_host(1, &first);
    v.follow_host(2, &again);
    v.select(&again[0].file, "SP 2000 - VCB");
    assert_eq!(v.chosen(), (0, Some(0)), "the paint read again is offered");
    v.select(&again[1].file, "");
    assert_eq!(v.chosen().0, 1, "the bus read again is listed");

    // (the same list again, or an empty one, changes nothing)
    v.follow_host(2, &first);
    v.follow_host(3, &[]);
    v.select(&again[0].file, "SP 2000 - VCB");
    assert_eq!(v.chosen(), (0, Some(0)));
}

/// The Maps page likewise takes a list read again with as many maps.
#[test]
fn the_maps_page_follows_a_list_read_again_with_as_many_maps() {
    let mut list = omsi_launcher_ui::sample::maps();
    list.truncate(2);
    let mut again = list.clone();
    again[1] = omsi_launcher_ui::sample::maps()[2].clone();

    let mut m = MapsView::default();
    m.follow_host(1, &list);
    m.follow_host(2, &again);
    m.select(&again[1].file);
    assert_eq!(m.chosen(), 1, "the map read again is listed");
}
