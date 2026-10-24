//! The shell without a window: clicks on the sidebar change the page.

use glam::Vec2;
use omsi_launcher_ui::look::{scale_for, DESIGN_H, DESIGN_W};
use omsi_launcher_ui::shell::nav_id;
use omsi_launcher_ui::{Page, Shell};
use omsi_ui::widgets::Ui;

/// One frame at the design size; `click` is a point of the design to press and release on.
fn frame(shell: &mut Shell, ui: &mut Ui, click: Option<Vec2>) {
    let scale = scale_for(DESIGN_W as u32, DESIGN_H as u32);
    if let Some(p) = click {
        ui.input.mouse = p;
        ui.input.pressed = true;
        ui.input.down = true;
        ui.begin(Vec2::new(DESIGN_W, DESIGN_H) / scale, scale, 0.016);
        shell.draw(ui);
        ui.finish();
        ui.input.released = true;
        ui.input.down = false;
    }
    ui.begin(Vec2::new(DESIGN_W, DESIGN_H) / scale, scale, 0.016);
    shell.draw(ui);
    ui.finish();
}

#[test]
fn clicking_a_sidebar_entry_opens_its_page() {
    let mut shell = Shell::new(Page::Maps);
    let mut ui = Ui::new();
    frame(&mut shell, &mut ui, None);
    for &page in Page::listed().iter().chain(Page::foot()) {
        let r = *ui.drawn.get(&nav_id(page)).expect("entry drawn");
        frame(
            &mut shell,
            &mut ui,
            Some(Vec2::new(r.x + 10.0, r.y + r.h * 0.5)),
        );
        assert_eq!(shell.page, page);
    }
}

#[test]
fn the_foot_stays_inside_a_small_window() {
    let mut shell = Shell::new(Page::Maps);
    let mut ui = Ui::new();
    // the smallest window the shell allows, in design units
    let size = Vec2::new(960.0, 540.0) / scale_for(960, 540);
    ui.begin(size, scale_for(960, 540), 0.016);
    shell.draw(&mut ui);
    ui.finish();
    for &page in Page::listed().iter().chain(Page::foot()) {
        let r = ui.drawn[&nav_id(page)];
        assert!(r.bottom() <= size.y + 0.5, "{page:?} is below the window");
    }
}

#[test]
fn a_click_on_a_vehicle_row_and_a_paint_chooses_them() {
    use omsi_launcher_ui::vehicles::VehiclesView;
    let mut shell = Shell::new(Page::Vehicles);
    shell.vehicles = VehiclesView::with_catalog(omsi_launcher_ui::sample::vehicles());
    let mut ui = Ui::new();
    frame(&mut shell, &mut ui, None);
    assert_eq!(shell.vehicles.chosen(), (0, None));
    let row = ui.drawn[&omsi_ui::widgets::id_of("vehicles:row:1")];
    frame(
        &mut shell,
        &mut ui,
        Some(Vec2::new(row.x + 30.0, row.y + 20.0)),
    );
    assert_eq!(shell.vehicles.chosen(), (1, None));
    let chip = ui.drawn[&omsi_ui::widgets::id_of("vehicles:paint:0")];
    frame(
        &mut shell,
        &mut ui,
        Some(Vec2::new(chip.x + 10.0, chip.y + 10.0)),
    );
    assert_eq!(shell.vehicles.chosen(), (1, Some(0)));
    // the bus's own livery is a chip of its own, before the repaints
    let chip = ui.drawn[&omsi_ui::widgets::id_of("vehicles:paint:default")];
    frame(
        &mut shell,
        &mut ui,
        Some(Vec2::new(chip.x + 10.0, chip.y + 10.0)),
    );
    assert_eq!(shell.vehicles.chosen(), (1, None));
}

#[test]
fn a_click_on_a_maker_heading_folds_its_models_away_and_back() {
    use omsi_launcher_ui::vehicles::VehiclesView;
    let mut shell = Shell::new(Page::Vehicles);
    shell.vehicles = VehiclesView::with_catalog(omsi_launcher_ui::sample::vehicles());
    let mut ui = Ui::new();
    frame(&mut shell, &mut ui, None);
    let id = |n: &str| omsi_ui::widgets::id_of(n);
    assert!(ui.drawn.contains_key(&id("vehicles:row:0")));
    let head = ui.drawn[&id("vehicles:group:maker one")];
    frame(
        &mut shell,
        &mut ui,
        Some(Vec2::new(head.x + 30.0, head.y + 10.0)),
    );
    frame(&mut shell, &mut ui, None);
    // (the sample's first bus is the only one of "Maker One": its row is gone, the other
    // maker's are still there)
    assert!(!ui.drawn.contains_key(&id("vehicles:row:0")));
    assert!(ui.drawn.contains_key(&id("vehicles:row:1")));
    let head = ui.drawn[&id("vehicles:group:maker one")];
    frame(
        &mut shell,
        &mut ui,
        Some(Vec2::new(head.x + 30.0, head.y + 10.0)),
    );
    frame(&mut shell, &mut ui, None);
    assert!(ui.drawn.contains_key(&id("vehicles:row:0")));
}

#[test]
fn a_click_on_a_map_row_chooses_it_and_explore_map_is_reported_once() {
    use omsi_launcher_ui::maps::MapsView;
    let mut shell = Shell::new(Page::Maps);
    shell.maps = MapsView::with_catalog(omsi_launcher_ui::sample::maps());
    let mut ui = Ui::new();
    frame(&mut shell, &mut ui, None);
    assert_eq!(shell.maps.chosen(), 0);
    assert!(shell.maps.start_area().is_some());
    let row = ui.drawn[&omsi_ui::widgets::id_of("maps:row:1")];
    frame(
        &mut shell,
        &mut ui,
        Some(Vec2::new(row.x + 30.0, row.y + 20.0)),
    );
    assert_eq!(shell.maps.chosen(), 1);
    let explore = ui.drawn[&omsi_ui::widgets::id_of("maps:explore")];
    frame(
        &mut shell,
        &mut ui,
        Some(Vec2::new(explore.x + 10.0, explore.y + 10.0)),
    );
    assert!(shell.maps.take_explore());
    assert!(!shell.maps.take_explore());
}

#[test]
fn a_catalog_read_again_keeps_the_bus_shown() {
    use omsi_launcher_ui::vehicles::VehiclesView;
    let list = omsi_launcher_ui::sample::vehicles();
    let mut view = VehiclesView::with_catalog(list[..1].to_vec());
    view.set_catalog(list.clone());
    assert_eq!(view.catalog_len(), Some(list.len()));
    view.select(&list[1].file, "");
    // a longer list (the host's scan went on): the same bus stays chosen
    let mut longer = list.clone();
    longer.insert(0, list[0].clone());
    view.set_catalog(longer);
    assert_eq!(view.chosen(), (2, None));
}

#[test]
fn use_this_bus_reports_the_bus_and_what_the_card_holds() {
    use omsi_launcher_ui::vehicles::VehiclesView;
    let mut shell = Shell::new(Page::Vehicles);
    shell.vehicles = VehiclesView::with_catalog(omsi_launcher_ui::sample::vehicles());
    let mut ui = Ui::new();
    frame(&mut shell, &mut ui, None);
    // (the card starts over for the bus shown, once; the host fills in the next start's)
    let file = shell.vehicles.fresh_bus().expect("a bus shown");
    assert_eq!(shell.vehicles.fresh_bus(), None);
    shell
        .vehicles
        .prepare(&file, Some("Depot B"), "50044", "AB 1");
    let b = ui.drawn[&omsi_ui::widgets::id_of("vehicles:use")];
    frame(&mut shell, &mut ui, Some(Vec2::new(b.x + 10.0, b.y + 10.0)));
    let used = shell.vehicles.take_use().expect("Use this bus clicked");
    assert_eq!(used.file, file);
    assert_eq!(used.hof.as_deref(), Some("Depot B"));
    assert_eq!(used.number.as_deref(), Some("50044"));
    assert_eq!(used.plate, "AB 1");
    assert!(shell.vehicles.take_use().is_none());
    // (another bus: automatic again)
    let row = ui.drawn[&omsi_ui::widgets::id_of("vehicles:row:1")];
    frame(
        &mut shell,
        &mut ui,
        Some(Vec2::new(row.x + 30.0, row.y + 20.0)),
    );
    let b = ui.drawn[&omsi_ui::widgets::id_of("vehicles:use")];
    frame(&mut shell, &mut ui, Some(Vec2::new(b.x + 10.0, b.y + 10.0)));
    let used = shell.vehicles.take_use().unwrap();
    assert_ne!(used.file, file);
    assert_eq!((used.hof, used.plate.as_str()), (None, ""));
}
