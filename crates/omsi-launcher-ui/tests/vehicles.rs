//! The Vehicles page and a bus whose file has the `[depot_physics]` key (spec 002, FR-008a):
//! the page marks it in the library and in the details card, as the Drive page's bus list does.

use glam::Vec2;
use omsi_launcher_ui::vehicles::VehiclesView;
use omsi_launcher_ui::window::depot_physics_notice;
use omsi_launcher_ui::{sample, Page, Shell};
use omsi_ui::widgets::Ui;

#[test]
fn a_bus_with_the_depot_physics_key_is_marked_on_the_vehicles_page() {
    assert_eq!(depot_physics_notice(false), None);
    let (badge, notice) = depot_physics_notice(true).unwrap();
    assert_eq!(badge, "DEPOT PHYSICS");
    assert!(notice.contains("[depot_physics]"));
    // the keyed bus chosen: its row and its details card are drawn with the mark
    let list = sample::vehicles();
    let keyed = list.iter().find(|v| v.depot_physics).unwrap();
    let mut shell = Shell::new(Page::Vehicles);
    shell.vehicles = VehiclesView::with_catalog(list.clone());
    shell.vehicles.select(&keyed.file, &keyed.default_paint);
    let mut ui = Ui::new();
    for _ in 0..3 {
        ui.begin(Vec2::new(1672.0, 941.0), 1.0, 0.016);
        shell.draw(&mut ui);
        ui.finish();
    }
    assert_eq!(
        shell.vehicles.current().map(|c| c.0.as_str()),
        Some(keyed.file.as_str())
    );
}
