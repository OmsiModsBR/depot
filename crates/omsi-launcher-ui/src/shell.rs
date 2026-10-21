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
    crumb(ui, page.title(), "", x, top);
    ui.text(
        page.title(),
        Vec2::new(x, top + 80.0),
        30.0,
        Weight::Bold,
        FG,
        Align::Left,
    );
}

/// Where a page's body starts under the top of its heading.
pub const BODY_TOP: f32 = 120.0;

/// A heading as Windows 11 pages have one: `NAME › PLACE` small and letter-spaced (the name in
/// the accent), the title large and one line under it, left at `x`, from `top` down. The title
/// and the line are cut to `w` points.
pub fn heading(ui: &mut Ui, name: &str, parts: (&str, &str, &str), x: f32, top: f32, w: f32) {
    heading_beside(ui, name, parts, x, top, w, w);
}

/// `NAME › PLACE` at `x`, its middle on `top + 38`.
fn crumb(ui: &mut Ui, name: &str, place: &str, x: f32, top: f32) {
    let px = 12.0;
    let cy = top + 38.0;
    let mut at = x;
    at += kit::spaced_by(
        ui,
        &omsi_ui::tr(name).to_uppercase(),
        at,
        cy,
        px,
        Weight::Bold,
        accent(),
        0.08,
    );
    if place.is_empty() {
        return;
    }
    at += 6.0;
    ui.text_in(
        "\u{203a}",
        Rect::new(at, cy - px, px, px * 2.0),
        px + 2.0,
        Weight::Regular,
        MUTED,
        Align::Left,
    );
    at += 12.0;
    kit::spaced_by(
        ui,
        &omsi_ui::tr(place).to_uppercase(),
        at,
        cy,
        px,
        Weight::Medium,
        MUTED,
        0.08,
    );
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
    crumb(ui, name, place, x, top);
    // (translated once, cut at the weight it is drawn in, drawn as it is)
    let title = omsi_ui::tr(title);
    let title = if title_w.is_finite() {
        kit::fit_text_as_is(ui, &title, 30.0, Weight::Bold, title_w)
    } else {
        title.to_string()
    };
    ui.text_as_is(
        &title,
        Vec2::new(x, top + 79.0),
        30.0,
        Weight::Bold,
        FG,
        Align::Left,
    );
    let px = 14.5;
    let line = omsi_ui::tr(line);
    let line = if w.is_finite() {
        kit::fit_text_as_is(ui, &line, px, Weight::Regular, w)
    } else {
        line.to_string()
    };
    ui.text_as_is(
        &line,
        Vec2::new(x, top + 103.0),
        px,
        Weight::Regular,
        MUTED,
        Align::Left,
    );
}

/// A caption button of the window, top right (the launcher draws its own title bar).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Caption {
    Minimize,
    Maximize,
    Close,
}

/// The id a caption button is clickable under.
pub fn caption_id(c: Caption) -> u64 {
    id_of(&format!("caption:{c:?}"))
}

/// The window's minimise, maximise (restore when `maximized`) and close buttons at the top
/// right of a window `w` points wide; which one was clicked. Close turns red under the mouse,
/// as Windows' does.
pub fn caption_buttons(ui: &mut Ui, w: f32, maximized: bool) -> Option<Caption> {
    let mut out = None;
    for (k, c) in [Caption::Minimize, Caption::Maximize, Caption::Close]
        .into_iter()
        .enumerate()
    {
        let r = Rect::new(w - CAPTION_W * (3 - k) as f32, 0.0, CAPTION_W, CAPTION_H);
        ui.solid(r);
        let (hover, down, clicked) = ui.interact(caption_id(c), r);
        let close = c == Caption::Close;
        if hover || down {
            let fill = if close {
                omsi_ui::Color::hex(0xc42b1c)
            } else {
                omsi_ui::Color::WHITE.alpha(if down { 0.04 } else { 0.07 })
            };
            ui.p().rect(r, fill);
        }
        let ink = if close && (hover || down) {
            omsi_ui::Color::WHITE
        } else {
            MUTED
        };
        let m = r.center();
        let p = ui.p();
        match c {
            Caption::Minimize => p.rect(Rect::new(m.x - 5.0, m.y, 10.0, 1.0), ink),
            Caption::Maximize if maximized => {
                p.rounded_border(Rect::new(m.x - 5.0, m.y - 3.0, 8.0, 8.0), 1.0, 1.0, ink);
                p.rect(Rect::new(m.x - 3.0, m.y - 5.0, 8.0, 1.0), ink);
                p.rect(Rect::new(m.x + 4.0, m.y - 5.0, 1.0, 8.0), ink);
            }
            Caption::Maximize => {
                p.rounded_border(Rect::new(m.x - 5.0, m.y - 5.0, 10.0, 10.0), 1.0, 1.0, ink)
            }
            Caption::Close => {
                p.line(m - Vec2::new(5.0, 5.0), m + Vec2::new(5.0, 5.0), 1.0, ink);
                p.line(m + Vec2::new(-5.0, 5.0), m + Vec2::new(5.0, -5.0), 1.0, ink);
            }
        }
        if clicked {
            out = Some(c);
        }
    }
    out
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
            // (the wordmark 27 points high, as the concept's lettering)
            let lh = 27.0;
            let lw = (lh / aspect.max(0.01)).min(SIDEBAR_W - 40.0);
            ui.image(Rect::new(18.0, 26.0, lw, lw * aspect), tex, 0.0);
        }
        let mut nav = None;
        let row = nav_row(h);
        for (i, &page) in Page::listed().iter().enumerate() {
            let top = NAV_TOP + i as f32 * row;
            if self.entry(ui, page, Rect::new(8.0, top, SIDEBAR_W - 17.0, row - 2.0)) {
                nav = Some(Nav::Page(page));
            }
        }
        let foot = Page::foot();
        for (i, &page) in foot.iter().enumerate() {
            let top = h - WHO_H - 8.0 - (foot.len() - i) as f32 * FOOT_ROW;
            if self.entry(
                ui,
                page,
                Rect::new(8.0, top, SIDEBAR_W - 17.0, FOOT_ROW - 2.0),
            ) {
                nav = Some(Nav::Page(page));
            }
        }
        ui.p()
            .rect(Rect::new(8.0, h - WHO_H - 1.0, SIDEBAR_W - 17.0, 1.0), LINE);
        let line = Rect::new(8.0, h - WHO_H, SIDEBAR_W - 17.0, WHO_H);
        let (hover, _, clicked) = ui.interact_key(footer_id(), line);
        if hover {
            ui.p().rounded(line.inset(2.0), 4.0, HOVER_LAYER);
        }
        let (name, what) = self.footer;
        let what_w = ui.width(what, 12.0, Weight::Regular);
        let cy = line.center().y;
        let name = kit::fit_text_as_is(
            ui,
            name,
            12.5,
            Weight::Bold,
            (line.w - 28.0 - what_w - 12.0).max(0.0),
        );
        ui.text_in(
            &name,
            Rect::new(line.x + 12.0, cy - 12.0, line.w, 24.0),
            12.5,
            Weight::Bold,
            FG,
            Align::Left,
        );
        ui.text_in(
            what,
            Rect::new(line.x, cy - 12.0, line.w - 12.0, 24.0),
            12.0,
            Weight::Regular,
            MUTED,
            Align::Right,
        );
        if clicked {
            nav = Some(Nav::Footer);
        }
        nav
    }

    fn entry(&self, ui: &mut Ui, page: Page, r: Rect) -> bool {
        let on = page == self.page;
        let id = nav_id(page);
        let (hover, down, clicked) = ui.interact_key(id, r);
        let r = Rect::new(r.x, r.y, r.w, NAV_ITEM_H.min(r.h));
        let lit = ui.anim(id, if on { 1.0 } else { 0.0 }, 0.08);
        // (Windows 11's navigation view: the chosen entry a layer lighter with a short accent
        // bar at its left, the others lit faintly under the mouse)
        if on {
            ui.p().rounded(r, CONTROL_RADIUS, SELECTED_LAYER);
            let bar = 18.0 * lit;
            ui.p().rounded(
                Rect::new(r.x, r.center().y - bar * 0.5, 3.0, bar),
                1.5,
                accent(),
            );
        } else if hover || down {
            ui.p().rounded(r, CONTROL_RADIUS, HOVER_LAYER);
        }
        let ic = if on { accent() } else { NAV_TEXT };
        ui.icon(page.icon(), Vec2::new(r.x + 22.0, r.center().y), 20.0, ic);
        ui.text_in(
            page.title(),
            Rect::new(r.x + 46.0, r.y, r.w - 46.0 - 34.0, r.h),
            14.5,
            Weight::Regular,
            if on { FG } else { NAV_TEXT },
            Align::Left,
        );
        if let Some((_, n, color)) = self.badges.iter().find(|b| b.0 == page) {
            ui.text_in(
                n,
                Rect::new(r.x, r.y, r.w - 12.0, r.h),
                12.5,
                Weight::Bold,
                *color,
                Align::Right,
            );
        }
        clicked
    }
}

/// The height of a listed page's row in a window `h` design points high: the design's, or less
/// when the foot entries would otherwise cover the last pages.
fn nav_row(h: f32) -> f32 {
    let foot_top = h - WHO_H - 8.0 - Page::foot().len() as f32 * FOOT_ROW;
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
            let foot_top = h - WHO_H - 8.0 - Page::foot().len() as f32 * FOOT_ROW;
            assert!(
                end <= foot_top + 0.01,
                "h={h}: pages end at {end}, foot starts at {foot_top}"
            );
            assert!(row <= NAV_ROW);
        }
        assert_eq!(nav_row(941.0), NAV_ROW);
    }
}
