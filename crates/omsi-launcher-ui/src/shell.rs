//! The launcher without a window: the sidebar, the page chosen and its body.

use crate::look::*;
use crate::maps::MapsView;
use crate::pages::Page;
use crate::vehicles::VehiclesView;
use glam::Vec2;
use omsi_ui::paint::Align;
use omsi_ui::widgets::{id_of, Ui};
use omsi_ui::{Rect, Weight};

/// What the launcher shows and which page it is on.
pub struct Shell {
    pub page: Page,
    /// The texture of the logo in the interface pipeline, when it has been loaded.
    pub logo: Option<(usize, f32)>,
    pub vehicles: VehiclesView,
    pub maps: MapsView,
}

impl Default for Shell {
    fn default() -> Self {
        Shell::new(Page::Drive)
    }
}

/// The id the sidebar entry of `page` is clickable under.
pub fn nav_id(page: Page) -> u64 {
    id_of(&format!("nav:{}", page.title()))
}

impl Shell {
    pub fn new(page: Page) -> Shell {
        Shell {
            page,
            logo: None,
            vehicles: VehiclesView::default(),
            maps: MapsView::default(),
        }
    }

    /// Draw one frame into `ui` (already begun, in design units: the window's size divided by
    /// the scale `look::scale_for` gives).
    /// Whether a page still waits for something, so the window keeps drawing without input.
    pub fn busy(&self) -> bool {
        self.vehicles.busy() || self.maps.busy()
    }

    pub fn draw(&mut self, ui: &mut Ui) {
        let (w, h) = (ui.size.x, ui.size.y);
        ui.p().rect(Rect::new(0.0, 0.0, w, h), BG);
        self.sidebar(ui, h);
        self.body(ui, w, h);
    }

    fn sidebar(&mut self, ui: &mut Ui, h: f32) {
        let side = Rect::new(0.0, 0.0, SIDEBAR_W, h);
        ui.solid(side);
        ui.p().rect(side, SIDE);
        ui.p().rect(Rect::new(SIDEBAR_W - 1.0, 0.0, 1.0, h), LINE);
        if let Some((tex, aspect)) = self.logo {
            let lw = SIDEBAR_W - 40.0;
            ui.image(Rect::new(20.0, 24.0, lw, lw * aspect), tex, 0.0);
        }
        for (i, &page) in Page::listed().iter().enumerate() {
            let top = NAV_TOP + i as f32 * NAV_ROW;
            self.entry(ui, page, Rect::new(0.0, top, SIDEBAR_W - 1.0, NAV_ROW));
        }
        let foot = Page::foot();
        for (i, &page) in foot.iter().enumerate() {
            let top = h - 45.0 - (foot.len() - i) as f32 * FOOT_ROW;
            self.entry(ui, page, Rect::new(0.0, top, SIDEBAR_W - 1.0, FOOT_ROW));
        }
        ui.p()
            .rect(Rect::new(0.0, h - 45.0, SIDEBAR_W - 1.0, 1.0), LINE);
        ui.text(
            "draft",
            Vec2::new(28.0, h - 17.0),
            13.0 * TEXT_SCALE,
            Weight::Regular,
            MUTED,
            Align::Left,
        );
    }

    fn entry(&mut self, ui: &mut Ui, page: Page, r: Rect) {
        let on = page == self.page;
        let (hover, _, clicked) = ui.interact(nav_id(page), r);
        if on {
            ui.p().rect(r, ACCENT);
        } else if hover {
            ui.p().rect(r, omsi_ui::Color::WHITE.alpha(0.05));
        }
        let c = if on { ON_ACCENT } else { NAV_TEXT };
        ui.icon(page.icon(), Vec2::new(39.0, r.y + r.h * 0.5), 22.0, c);
        let weight = if on { Weight::Medium } else { Weight::Regular };
        ui.text(
            page.title(),
            Vec2::new(68.0, r.y + r.h * 0.5 + 6.0),
            17.0 * TEXT_SCALE,
            weight,
            c,
            Align::Left,
        );
        if clicked {
            self.page = page;
        }
    }

    fn body(&mut self, ui: &mut Ui, w: f32, h: f32) {
        let x0 = SIDEBAR_W + 28.0;
        ui.text(
            &self.page.title().to_uppercase(),
            Vec2::new(x0, 46.0),
            13.0 * TEXT_SCALE,
            Weight::Medium,
            ACCENT,
            Align::Left,
        );
        ui.text(
            self.page.title(),
            Vec2::new(x0, 82.0),
            40.0,
            Weight::Bold,
            FG,
            Align::Left,
        );
        let area = Rect::new(x0, 0.0, w - x0 - 28.0, h);
        match self.page {
            Page::Vehicles => {
                self.vehicles.draw(ui, area);
                return;
            }
            Page::Maps => {
                self.maps.draw(ui, area);
                return;
            }
            _ => {}
        }
        let r = Rect::new(x0, 130.0, w - x0 - 28.0, (h - 130.0 - 60.0).max(120.0));
        ui.panel(r);
        ui.text_in(
            "This page is built in a later step. The launcher in the game binary still has it.",
            Rect::new(r.x + 24.0, r.y, r.w - 48.0, 72.0),
            15.0,
            Weight::Regular,
            MUTED,
            Align::Left,
        );
    }
}
