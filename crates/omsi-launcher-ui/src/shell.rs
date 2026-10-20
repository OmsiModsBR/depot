//! The launcher without a window: the sidebar, the page chosen and its body.

use crate::kit;
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
        let bar = Sidebar {
            page: self.page,
            logo: self.logo,
            badges: &[],
            footer: ("Project Depot", "preview"),
        };
        if let Some(Nav::Page(p)) = bar.draw(ui, h) {
            self.page = p;
        }
    }

    fn body(&mut self, ui: &mut Ui, w: f32, h: f32) {
        let x0 = SIDEBAR_W + 28.0;
        page_heading(ui, self.page, x0, 0.0);
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
            "The game's launcher draws this page.",
            Rect::new(r.x + 24.0, r.y, r.w - 48.0, 72.0),
            15.0,
            Weight::Regular,
            MUTED,
            Align::Left,
        );
    }
}

/// The page's small letter-spaced name and its title, left at `x`, from `top` down (the page's
/// body starts `BODY_TOP` points under `top`).
pub fn page_heading(ui: &mut Ui, page: Page, x: f32, top: f32) {
    if let Some(h) = page.heading() {
        heading(ui, page.title(), h, x, top, f32::INFINITY);
        return;
    }
    ui.text(
        &page.title().to_uppercase(),
        Vec2::new(x, top + 46.0),
        13.0 * TEXT_SCALE,
        Weight::Medium,
        ACCENT,
        Align::Left,
    );
    ui.text(
        page.title(),
        Vec2::new(x, top + 82.0),
        40.0,
        Weight::Bold,
        FG,
        Align::Left,
    );
}

/// Where a page's body starts under the top of its heading.
pub const BODY_TOP: f32 = 137.0;

/// A heading as the concept draws one: `name › place` small and letter-spaced, the title large
/// and one line under it, left at `x`, from `top` down. The title and the line are cut to `w`
/// points.
pub fn heading(ui: &mut Ui, name: &str, parts: (&str, &str, &str), x: f32, top: f32, w: f32) {
    heading_beside(ui, name, parts, x, top, w, w);
}

/// `heading` with its title cut to `title_w` points, the line to `w` (a page with tabs at the
/// right of its title).
pub fn heading_beside(
    ui: &mut Ui,
    name: &str,
    (place, title, line): (&str, &str, &str),
    x: f32,
    top: f32,
    w: f32,
    title_w: f32,
) {
    // (as the concept: "VEHICLES › PREVIEW" letter-spaced, the question large, a line)
    let px = 13.0 * TEXT_SCALE;
    let mut at = x;
    at += kit::spaced(
        ui,
        &omsi_ui::tr(name).to_uppercase(),
        at,
        top + 42.0,
        px,
        Weight::Bold,
        ACCENT,
    );
    at += 12.0;
    ui.text_in(
        "\u{203a}",
        Rect::new(at, top + 42.0 - px, px, px * 2.0),
        px,
        Weight::Regular,
        MUTED,
        Align::Left,
    );
    at += 14.0;
    kit::spaced(
        ui,
        &omsi_ui::tr(place).to_uppercase(),
        at,
        top + 42.0,
        px,
        Weight::Medium,
        MUTED,
    );
    let title = if title_w.is_finite() {
        kit::fit_text(ui, &omsi_ui::tr(title), 38.0, title_w)
    } else {
        title.to_string()
    };
    ui.text(
        &title,
        Vec2::new(x, top + 80.0),
        38.0,
        Weight::Bold,
        FG,
        Align::Left,
    );
    let px = 16.5 * TEXT_SCALE;
    let line = if w.is_finite() {
        kit::fit_text(ui, &omsi_ui::tr(line), px, w)
    } else {
        line.to_string()
    };
    ui.text(
        &line,
        Vec2::new(x, top + 112.0),
        px,
        Weight::Regular,
        MUTED,
        Align::Left,
    );
}

/// What a click on the sidebar asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nav {
    Page(Page),
    /// The line at the very foot (the driver, in the game's launcher).
    Footer,
}

/// The sidebar: the logo, the pages' list, Settings and Setup at its foot and one line under
/// them, on the left `SIDEBAR_W` points of a window `h` points high.
pub struct Sidebar<'a> {
    /// The page shown (lit).
    pub page: Page,
    /// The logo's texture and its height over its width.
    pub logo: Option<(usize, f32)>,
    /// A short count beside a page's name (games running, downloads) and its colour.
    pub badges: &'a [(Page, String, omsi_ui::Color)],
    /// The line at the foot: a name and, smaller, what it is.
    pub footer: (&'a str, &'a str),
}

/// The id the foot line is clickable under.
pub fn footer_id() -> u64 {
    id_of("nav:footer")
}

impl Sidebar<'_> {
    /// Draw it; what was clicked.
    pub fn draw(&self, ui: &mut Ui, h: f32) -> Option<Nav> {
        let side = Rect::new(0.0, 0.0, SIDEBAR_W, h);
        ui.solid(side);
        ui.p().rect(side, SIDE);
        ui.p().rect(Rect::new(SIDEBAR_W - 1.0, 0.0, 1.0, h), LINE);
        if let Some((tex, aspect)) = self.logo {
            let lw = SIDEBAR_W - 40.0;
            ui.image(Rect::new(20.0, 24.0, lw, lw * aspect), tex, 0.0);
        }
        let mut nav = None;
        let row = nav_row(h);
        for (i, &page) in Page::listed().iter().enumerate() {
            let top = NAV_TOP + i as f32 * row;
            if self.entry(ui, page, Rect::new(0.0, top, SIDEBAR_W - 1.0, row)) {
                nav = Some(Nav::Page(page));
            }
        }
        let foot = Page::foot();
        for (i, &page) in foot.iter().enumerate() {
            let top = h - 45.0 - (foot.len() - i) as f32 * FOOT_ROW;
            if self.entry(ui, page, Rect::new(0.0, top, SIDEBAR_W - 1.0, FOOT_ROW)) {
                nav = Some(Nav::Page(page));
            }
        }
        ui.p()
            .rect(Rect::new(0.0, h - 45.0, SIDEBAR_W - 1.0, 1.0), LINE);
        let line = Rect::new(0.0, h - 44.0, SIDEBAR_W - 1.0, 44.0);
        let (hover, _, clicked) = ui.interact_key(footer_id(), line);
        if hover {
            ui.p().rect(line, omsi_ui::Color::WHITE.alpha(0.05));
        }
        let (name, what) = self.footer;
        let what_w = ui.width(what, 12.0, Weight::Regular);
        crate::kit::clipped(
            ui,
            name,
            28.0,
            h - 22.0,
            (SIDEBAR_W - 56.0 - what_w - 12.0).max(0.0),
            13.0 * TEXT_SCALE,
            Weight::Medium,
            NAV_TEXT,
        );
        crate::kit::right(
            ui,
            what,
            SIDEBAR_W - 22.0,
            h - 22.0,
            12.0,
            Weight::Regular,
            MUTED,
        );
        if clicked {
            nav = Some(Nav::Footer);
        }
        nav
    }

    fn entry(&self, ui: &mut Ui, page: Page, r: Rect) -> bool {
        let on = page == self.page;
        let (hover, _, clicked) = ui.interact_key(nav_id(page), r);
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
        if let Some((_, n, color)) = self.badges.iter().find(|b| b.0 == page) {
            ui.text(
                n,
                Vec2::new(r.right() - 22.0, r.y + r.h * 0.5 + 5.0),
                13.0,
                Weight::Bold,
                if on { ON_ACCENT } else { *color },
                Align::Right,
            );
        }
        clicked
    }
}

/// The height of a listed page's row in a window `h` design points high: the design's, or less
/// when the foot entries would otherwise cover the last pages.
fn nav_row(h: f32) -> f32 {
    let foot_top = h - 45.0 - Page::foot().len() as f32 * FOOT_ROW;
    NAV_ROW.min((foot_top - NAV_TOP) / Page::listed().len() as f32)
}

#[cfg(test)]
mod nav_tests {
    use super::*;

    #[test]
    fn listed_pages_never_reach_the_foot_entries() {
        for h in [540.0_f32, 720.0, 941.0, 2160.0] {
            let row = nav_row(h);
            let end = NAV_TOP + Page::listed().len() as f32 * row;
            let foot_top = h - 45.0 - Page::foot().len() as f32 * FOOT_ROW;
            assert!(
                end <= foot_top + 0.01,
                "h={h}: pages end at {end}, foot starts at {foot_top}"
            );
            assert!(row <= NAV_ROW);
        }
        assert_eq!(nav_row(941.0), NAV_ROW);
    }
}
