//! The pages' shared heading without a window: where a page's body and its tabs go.

use glam::Vec2;
use omsi_launcher_ui::shell::BODY_TOP;
use omsi_launcher_ui::window::{page_title, page_title_beside, title_tabs};
use omsi_launcher_ui::Page;
use omsi_ui::widgets::Ui;
use omsi_ui::Rect;

fn drawn(f: impl FnOnce(&mut Ui) -> Rect) -> Rect {
    let mut ui = Ui::new();
    ui.begin(Vec2::new(1600.0, 900.0), 1.0, 0.016);
    let r = f(&mut ui);
    ui.finish();
    r
}

#[test]
fn a_page_body_starts_under_its_heading() {
    let area = Rect::new(40.0, 60.0, 1000.0, 700.0);
    let body = drawn(|ui| page_title(ui, area, Page::Tutorials));
    let top = area.y - 28.0;
    assert_eq!(
        body,
        Rect::new(40.0, top + BODY_TOP, 1000.0, area.bottom() - top - BODY_TOP)
    );
}

#[test]
fn tabs_beside_the_title_leave_the_body_as_it_was() {
    let area = Rect::new(40.0, 60.0, 1000.0, 700.0);
    let plain = drawn(|ui| page_title(ui, area, Page::Mods));
    let beside = drawn(|ui| page_title_beside(ui, area, "Mods", ("MODS", "Mods", "line"), 300.0));
    assert_eq!(plain, beside);
}

#[test]
fn tabs_sit_at_the_right_of_the_title() {
    let body = Rect::new(40.0, 152.0, 1000.0, 600.0);
    let r = title_tabs(body, 300.0);
    assert_eq!(r, Rect::new(740.0, 152.0 - BODY_TOP + 56.0, 300.0, 34.0));
}
