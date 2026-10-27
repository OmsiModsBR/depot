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
        Shell::new(Page::Maps)
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
            footer_ask: false,
            compact: false,
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
                // (the game's launcher draws the start: here only its card)
                if let Some(r) = self.maps.start_area() {
                    kit::card(ui, r, "Start", None);
                }
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
        // (named as Windows names its own)
        ui.tooltip(
            r,
            match c {
                Caption::Minimize => "Minimize",
                Caption::Maximize if maximized => "Restore down",
                Caption::Maximize => "Maximize",
                Caption::Close => "Close",
            },
        );
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
    /// The right-hand part of the foot line asks for something ("Create" a driver): in the
    /// accent, not muted.
    pub footer_ask: bool,
    /// Only the icons, in a narrow strip (a narrow window, as Windows 11's navigation view
    /// does): each name in a tooltip.
    pub compact: bool,
}

/// The sidebar's width: whole, or only its icons (`Sidebar::compact`).
pub fn sidebar_w(compact: bool) -> f32 {
    if compact {
        SIDEBAR_COMPACT_W
    } else {
        SIDEBAR_W
    }
}

/// Where the chosen entry's accent mark stands: at `page`'s entry, or on its way there from
/// the entry chosen before, through the ones between (`marks`: the entries in the order they
/// are laid out). None when `page` has no entry.
fn sliding(ui: &mut Ui, name: &str, marks: &[(Page, Rect)], page: Page) -> Option<Rect> {
    let at = marks.iter().position(|(p, _)| *p == page)?;
    let last = marks.len() - 1;
    let k = ui
        .tween(id_of(name), at as f32, omsi_ui::theme::MOTION_SLOW)
        .clamp(0.0, last as f32);
    let i = (k.floor() as usize).min(last);
    let f = k - i as f32;
    let (a, b) = (marks[i].1, marks[(i + 1).min(last)].1);
    let mix = |a: f32, b: f32| a + (b - a) * f;
    Some(Rect::new(
        mix(a.x, b.x),
        mix(a.y, b.y),
        mix(a.w, b.w),
        mix(a.h, b.h),
    ))
}

/// The id the foot line is clickable under.
pub fn footer_id() -> u64 {
    id_of("nav:footer")
}

impl Sidebar<'_> {
    /// Draw it; what was clicked.
    pub fn draw(&self, ui: &mut Ui, h: f32) -> Option<Nav> {
        let w = sidebar_w(self.compact);
        let side = Rect::new(0.0, 0.0, w, h);
        ui.solid(side);
        ui.p().rect(side, omsi_ui::theme::surface(SIDE));
        ui.p().rect(Rect::new(w - 1.0, 0.0, 1.0, h), LINE);
        // (the wordmark needs the whole width: a compact bar goes without it)
        if let Some((tex, aspect)) = self.logo.filter(|_| !self.compact) {
            // (the wordmark 27 points high, as the concept's lettering)
            let lh = 27.0;
            let lw = (lh / aspect.max(0.01)).min(SIDEBAR_W - 40.0);
            ui.image(Rect::new(18.0, 26.0, lw, lw * aspect), tex, 0.0);
        }
        let mut nav = None;
        let mut marks = Vec::new();
        let row = nav_row(h);
        for (i, &page) in Page::listed().iter().enumerate() {
            let top = NAV_TOP + i as f32 * row;
            let r = Rect::new(8.0, top, w - 17.0, row - 2.0);
            if self.entry(ui, page, r) {
                nav = Some(Nav::Page(page));
            }
            marks.push((page, r));
        }
        let foot = Page::foot();
        for (i, &page) in foot.iter().enumerate() {
            let top = h - WHO_H - 8.0 - (foot.len() - i) as f32 * FOOT_ROW;
            let r = Rect::new(8.0, top, w - 17.0, FOOT_ROW - 2.0);
            if self.entry(ui, page, r) {
                nav = Some(Nav::Page(page));
            }
            marks.push((page, r));
        }
        // (Windows 11's navigation view: a short accent bar at the chosen entry's left,
        // sliding there from the entry chosen before)
        if let Some(r) = sliding(ui, "nav:side-mark", &marks, self.page) {
            let r = Rect::new(r.x, r.y, r.w, NAV_ITEM_H.min(r.h));
            ui.p()
                .rounded(Rect::new(r.x, r.center().y - 9.0, 3.0, 18.0), 1.5, accent());
        }
        ui.p()
            .rect(Rect::new(8.0, h - WHO_H - 1.0, w - 17.0, 1.0), LINE);
        let line = Rect::new(8.0, h - WHO_H, w - 17.0, WHO_H);
        let (hover, _, clicked) = ui.interact_key(footer_id(), line);
        if hover {
            ui.p().rounded(line.inset(2.0), 4.0, HOVER_LAYER);
        }
        let (name, what) = self.footer;
        if self.compact {
            // (the driver's icon only, the name and what it is in its tooltip)
            ui.icon(
                "o-user",
                line.center(),
                16.0,
                if self.footer_ask { accent() } else { MUTED },
            );
            ui.tooltip(
                line,
                &format!("{} · {}", omsi_ui::tr(name), omsi_ui::tr(what)),
            );
            if clicked {
                nav = Some(Nav::Footer);
            }
            return nav;
        }
        let what_w = ui.width(what, 12.0, Weight::Regular);
        let cy = line.center().y;
        // (the driver's icon, then the name, then what it is - or what to do - at the right)
        ui.icon("o-user", Vec2::new(line.x + 20.0, cy), 16.0, MUTED);
        let name = kit::fit_text_as_is(
            ui,
            name,
            12.5,
            Weight::Bold,
            (line.w - 40.0 - what_w - 12.0).max(0.0),
        );
        ui.text_in(
            &name,
            Rect::new(line.x + 36.0, cy - 12.0, line.w, 24.0),
            12.5,
            Weight::Bold,
            FG,
            Align::Left,
        );
        let what_c = if self.footer_ask {
            if hover {
                accent().lighten(0.15)
            } else {
                accent()
            }
        } else {
            MUTED
        };
        ui.text_in(
            what,
            Rect::new(line.x, cy - 12.0, line.w - 12.0, 24.0),
            12.0,
            if self.footer_ask {
                Weight::Medium
            } else {
                Weight::Regular
            },
            what_c,
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
        // (Windows 11's navigation view: the chosen entry a layer lighter - its accent bar is
        // drawn over the entries, sliding - the others lit faintly under the mouse)
        if on {
            ui.p().rounded(r, CONTROL_RADIUS, SELECTED_LAYER);
        } else if hover || down {
            ui.p().rounded(r, CONTROL_RADIUS, HOVER_LAYER);
        }
        let ic = if on { accent() } else { NAV_TEXT };
        if self.compact {
            // (the icon in the middle, a count as a dot, the name in a tooltip)
            ui.icon(page.icon(), r.center(), 20.0, ic);
            if let Some((_, _, color)) = self.badges.iter().find(|b| b.0 == page) {
                ui.p()
                    .circle(Vec2::new(r.right() - 7.0, r.y + 9.0), 3.5, *color);
            }
            ui.tooltip(r, page.title());
            return clicked;
        }
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

/// The bar along the top (Settings > Interface > Navigation: Top bar): the logo, the pages in
/// a row, Settings and Setup at its right before the caption buttons. When the names do not
/// all fit, the entries show their icons only: the open page keeps its name, and an entry
/// under the mouse opens to show its own.
pub struct TopBar<'a> {
    /// The page shown (lit).
    pub page: Page,
    /// The logo's texture and its height over its width.
    pub logo: Option<(usize, f32)>,
    /// A short count beside a page's name (games running, downloads) and its colour.
    pub badges: &'a [(Page, String, omsi_ui::Color)],
}

/// A top bar entry's name.
const TOP_TEXT_PX: f32 = 14.0;
/// Space between the top bar's entries, and between its two groups.
const TOP_GAP: f32 = 2.0;
const TOP_GROUP_GAP: f32 = 24.0;
/// The logo's height in the top bar.
const TOP_LOGO_H: f32 = 22.0;

/// The id of the animation that opens a top bar entry to show its name.
fn top_open_id(page: Page) -> u64 {
    id_of(&format!("topnav-open:{}", page.title()))
}

impl TopBar<'_> {
    /// The width of `page`'s entry with its name.
    fn full_w(&self, ui: &Ui, page: Page) -> f32 {
        let badge = self
            .badges
            .iter()
            .find(|b| b.0 == page)
            .map_or(0.0, |b| ui.width(&b.1, 12.5, Weight::Bold) + 8.0);
        12.0 + 20.0 + 10.0 + ui.width(page.title(), TOP_TEXT_PX, Weight::Regular) + badge + 14.0
    }

    /// The logo's width (and the space after it).
    fn logo_w(&self) -> f32 {
        match self.logo {
            Some((_, aspect)) => TOP_LOGO_H / aspect.max(0.01),
            None => 140.0,
        }
    }

    /// Whether the entries show only their icons in a window `w` points wide.
    pub fn compact(&self, ui: &Ui, w: f32) -> bool {
        let pages = Page::listed().iter().chain(Page::foot());
        let full: f32 = pages.map(|&p| self.full_w(ui, p) + TOP_GAP).sum();
        full + TOP_GROUP_GAP > top_space(w, self.logo_w())
    }

    /// Draw it across a window `w` points wide; what was clicked.
    pub fn draw(&self, ui: &mut Ui, w: f32) -> Option<Nav> {
        let bar = Rect::new(0.0, 0.0, w, TOPBAR_H);
        ui.solid(bar);
        ui.p().rect(bar, omsi_ui::theme::surface(SIDE));
        ui.p().rect(Rect::new(0.0, TOPBAR_H - 1.0, w, 1.0), LINE);
        if let Some((tex, aspect)) = self.logo {
            let lw = self.logo_w();
            ui.image(
                Rect::new(18.0, (TOPBAR_H - lw * aspect) * 0.5, lw, lw * aspect),
                tex,
                0.0,
            );
        }
        let compact = self.compact(ui, w);
        let start = 18.0 + self.logo_w() + 22.0;
        let end = start + top_space(w, self.logo_w());
        let mut nav = None;
        // (Settings and Setup at the right, laid out from there leftwards)
        let foot: Vec<(Page, f32)> = Page::foot()
            .iter()
            .map(|&p| (p, self.entry_w(ui, p, compact)))
            .collect();
        let mut marks = Vec::new();
        let mut x = start;
        for &page in Page::listed() {
            let ew = self.entry_w(ui, page, compact);
            let r = Rect::new(x, 0.0, ew, TOPBAR_H);
            if self.entry(ui, page, r, compact) {
                nav = Some(Nav::Page(page));
            }
            marks.push((page, r));
            x += ew + TOP_GAP;
        }
        let mut x = end - foot.iter().map(|(_, ew)| ew + TOP_GAP).sum::<f32>();
        for (page, ew) in foot {
            let r = Rect::new(x, 0.0, ew, TOPBAR_H);
            if self.entry(ui, page, r, compact) {
                nav = Some(Nav::Page(page));
            }
            marks.push((page, r));
            x += ew + TOP_GAP;
        }
        // (Windows 11's top navigation: the chosen entry marked by an accent bar along the
        // bar's edge, sliding there from the entry chosen before)
        if let Some(r) = sliding(ui, "nav:top-mark", &marks, self.page) {
            let bar = r.w - 24.0;
            ui.p().rounded(
                Rect::new(r.center().x - bar * 0.5, TOPBAR_H - 3.0, bar, 3.0),
                1.5,
                accent(),
            );
        }
        nav
    }

    /// The width `page`'s entry has in this frame: with its name, or its icon only (opening
    /// while the mouse is on it).
    fn entry_w(&self, ui: &mut Ui, page: Page, compact: bool) -> f32 {
        let full = self.full_w(ui, page);
        if !compact || page == self.page {
            return full;
        }
        TOPBAR_ICON_W + (full - TOPBAR_ICON_W) * ui.anim_now(top_open_id(page))
    }

    fn entry(&self, ui: &mut Ui, page: Page, r: Rect, compact: bool) -> bool {
        let on = page == self.page;
        let id = nav_id(page);
        let (hover, down, clicked) = ui.interact_key(id, r);
        let open = if compact && !on {
            ui.anim(top_open_id(page), if hover { 1.0 } else { 0.0 }, 0.09)
        } else {
            1.0
        };
        let tile = Rect::new(r.x, 8.0, r.w, TOPBAR_H - 16.0);
        // (the chosen entry is marked only by the accent bar under it, drawn over the
        // entries - a lighter tile as well marked it twice; the tile is the mouse's, as on
        // the other entries)
        if hover || down {
            ui.p().rounded(tile, CONTROL_RADIUS, HOVER_LAYER);
        }
        let ic = if on { accent() } else { NAV_TEXT };
        let cy = r.center().y;
        // (an entry with only its icon has it in the middle; opening, the icon moves to where
        // it stands before the name)
        let icon_x = if compact && !on {
            r.x + TOPBAR_ICON_W * 0.5 + (22.0 - TOPBAR_ICON_W * 0.5) * open
        } else {
            r.x + 22.0
        };
        ui.icon(page.icon(), Vec2::new(icon_x, cy), 20.0, ic);
        let badge = self.badges.iter().find(|b| b.0 == page);
        if open > 0.02 {
            let tx = Rect::new(r.x + 42.0, r.y, (r.w - 42.0 - 8.0).max(0.0), r.h);
            let ink = if on { FG } else { NAV_TEXT };
            // (cut by the entry while it opens, not shortened)
            ui.push_clip(tx, 0.0);
            ui.text_in(
                page.title(),
                Rect::new(tx.x, tx.y, tx.w + 200.0, tx.h),
                TOP_TEXT_PX,
                Weight::Regular,
                ink.alpha(open),
                Align::Left,
            );
            ui.pop_clip();
            if let Some((_, n, color)) = badge.filter(|_| open > 0.98) {
                ui.text_in(
                    n,
                    Rect::new(r.x, r.y, r.w - 14.0, r.h),
                    12.5,
                    Weight::Bold,
                    *color,
                    Align::Right,
                );
            }
        } else if let Some((_, _, color)) = badge {
            // (an entry with only its icon: a dot says there is something to see)
            ui.p()
                .circle(Vec2::new(icon_x + 11.0, cy - 9.0), 3.5, *color);
        }
        clicked
    }
}

/// The room the top bar's entries have in a window `w` points wide after a logo `logo_w`
/// wide: from after the logo to before the caption buttons.
fn top_space(w: f32, logo_w: f32) -> f32 {
    (w - CAPTION_W * 3.0 - 8.0 - (18.0 + logo_w + 22.0)).max(0.0)
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

    /// A top bar too narrow for the names shows the icons, the open page with its name; the
    /// entry under the mouse opens to show its own, and closes when the mouse leaves.
    #[test]
    fn a_narrow_top_bar_shows_icons_and_opens_the_one_under_the_mouse() {
        let bar = TopBar {
            page: Page::Maps,
            logo: None,
            badges: &[],
        };
        let mut ui = Ui::new();
        assert!(!bar.compact(&ui, 4000.0));
        assert!(bar.compact(&ui, 1100.0));
        let frame = |ui: &mut Ui| {
            ui.begin(Vec2::new(1100.0, 700.0), 1.0, 0.05);
            bar.draw(ui, 1100.0);
        };
        frame(&mut ui);
        let drawn = |ui: &Ui, p: Page| ui.drawn[&nav_id(p)];
        let maps = drawn(&ui, Page::Maps);
        let mods = drawn(&ui, Page::Mods);
        assert!(
            maps.w > TOPBAR_ICON_W + 20.0,
            "the open page keeps its name"
        );
        assert!(
            (mods.w - TOPBAR_ICON_W).abs() < 0.5,
            "the others show their icons"
        );
        ui.input.mouse = mods.center();
        for _ in 0..20 {
            frame(&mut ui);
        }
        assert!(drawn(&ui, Page::Mods).w > TOPBAR_ICON_W + 20.0);
        ui.input.mouse = Vec2::new(-100.0, -100.0);
        for _ in 0..20 {
            frame(&mut ui);
        }
        assert!((drawn(&ui, Page::Mods).w - TOPBAR_ICON_W).abs() < 1.0);
    }

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
