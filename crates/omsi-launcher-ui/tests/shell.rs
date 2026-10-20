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
    let mut shell = Shell::new(Page::Drive);
    let mut ui = Ui::new();
    frame(&mut shell, &mut ui, None);
    for page in Page::ALL {
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
    let mut shell = Shell::new(Page::Drive);
    let mut ui = Ui::new();
    // the smallest window the shell allows, in design units
    let size = Vec2::new(960.0, 540.0) / scale_for(960, 540);
    ui.begin(size, scale_for(960, 540), 0.016);
    shell.draw(&mut ui);
    ui.finish();
    for page in Page::ALL {
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
fn a_click_on_a_map_row_and_a_start_point_chooses_them() {
    use omsi_launcher_ui::maps::MapsView;
    let mut shell = Shell::new(Page::Maps);
    shell.maps = MapsView::with_catalog(omsi_launcher_ui::sample::maps());
    let mut ui = Ui::new();
    frame(&mut shell, &mut ui, None);
    assert_eq!(shell.maps.chosen(), (0, None));
    let start = ui.drawn[&omsi_ui::widgets::id_of("maps:start:2")];
    frame(
        &mut shell,
        &mut ui,
        Some(Vec2::new(start.x + 30.0, start.y + 10.0)),
    );
    assert_eq!(shell.maps.chosen(), (0, Some(2)));
    let auto = ui.drawn[&omsi_ui::widgets::id_of("maps:start:auto")];
    frame(
        &mut shell,
        &mut ui,
        Some(Vec2::new(auto.x + 30.0, auto.y + 10.0)),
    );
    assert_eq!(shell.maps.chosen(), (0, None));
    let row = ui.drawn[&omsi_ui::widgets::id_of("maps:row:1")];
    frame(
        &mut shell,
        &mut ui,
        Some(Vec2::new(row.x + 30.0, row.y + 20.0)),
    );
    // a new map starts on the automatic start point
    assert_eq!(shell.maps.chosen(), (1, None));
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
