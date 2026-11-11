//! The list of the map's entry points that "Place" of the "Add vehicle" window opens.

use glam::Vec2;
use omsi_ui::ingame::{entry_points, entry_points_rect, AddVehicleEvent};
use omsi_ui::widgets::{id_of, Ui};

const SIZE: Vec2 = Vec2::new(1672.0, 941.0);

fn frame(ui: &mut Ui, labels: &[String], chosen: Option<usize>) -> Option<AddVehicleEvent> {
    ui.begin(SIZE, 1.0, 0.016);
    let e = entry_points(ui, SIZE, labels, chosen);
    ui.finish();
    e
}

/// A press and its release at `p`; what it chose comes back on one of the frames.
fn click(
    ui: &mut Ui,
    p: Vec2,
    labels: &[String],
    chosen: Option<usize>,
) -> Option<AddVehicleEvent> {
    ui.input.mouse = p;
    ui.input.pressed = true;
    ui.input.down = true;
    let a = frame(ui, labels, chosen);
    ui.input.pressed = false;
    ui.input.released = true;
    ui.input.down = false;
    let b = frame(ui, labels, chosen);
    ui.input.released = false;
    a.or(b)
}

fn labels() -> Vec<String> {
    vec!["Depot".into(), "Terminus".into(), "Bridge".into()]
}

#[test]
fn a_line_clicked_chooses_its_entry_point() {
    let mut ui = Ui::new();
    let labels = labels();
    frame(&mut ui, &labels, None);
    let row = ui.drawn[&id_of("add:entry:row:1")];
    assert!(entry_points_rect(SIZE).contains(row.center()));
    assert_eq!(
        click(&mut ui, row.center(), &labels, None),
        Some(AddVehicleEvent::EntryPoint(1))
    );
}

#[test]
fn confirm_needs_an_entry_point_chosen() {
    let mut ui = Ui::new();
    let labels = labels();
    frame(&mut ui, &labels, None);
    let confirm = ui.drawn[&id_of("add:entry:btn:0")].center();
    assert_eq!(click(&mut ui, confirm, &labels, None), None);
    assert_eq!(
        click(&mut ui, confirm, &labels, Some(2)),
        Some(AddVehicleEvent::ConfirmEntryPoint)
    );
}

#[test]
fn cancel_closes_the_list() {
    let mut ui = Ui::new();
    let labels = labels();
    frame(&mut ui, &labels, Some(0));
    let cancel = ui.drawn[&id_of("add:entry:btn:1")].center();
    assert_eq!(
        click(&mut ui, cancel, &labels, Some(0)),
        Some(AddVehicleEvent::Cancel)
    );
}

#[test]
fn a_map_without_entry_points_can_only_cancel() {
    let mut ui = Ui::new();
    frame(&mut ui, &[], None);
    assert!(!ui.drawn.contains_key(&id_of("add:entry:row:0")));
    let cancel = ui.drawn[&id_of("add:entry:btn:1")].center();
    assert_eq!(
        click(&mut ui, cancel, &[], None),
        Some(AddVehicleEvent::Cancel)
    );
}
