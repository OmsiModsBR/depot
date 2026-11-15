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
