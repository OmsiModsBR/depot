//! The pages behind the Esc menu's lines in the new interface: the settings windows
//! (Options, Controls, Vehicle options, World options, the keyboard's and the game
//! controllers'), the lists (line and tour, drivers, fleet numbers, destinations, depot files,
//! start points, placing a vehicle, the administration) and their drop-downs.
//!
//! The pages stay the HUD's own: what they show is the HUD's frame (`ui::Frame`, made from the
//! game's lists as before), and what a key or a click does is the game's code as it was. This
//! module only draws them, with `omsi_ui::menu_page`, in the room right of the Esc menu's
//! column (still drawn, its line of the page marked), and hands the HUD the rectangles the
//! page was drawn in: the HUD's mouse code hit-tests those as it did its own (`hud::Hits`:
//! `menu_rects`, `menu_ctl`, `menu_side`, `dd_rects` ...), so nothing of how the pages work
//! changes. On a triple screen the page stands on the centre screen with the column, and the
//! rectangles are where it is drawn there; the headset shows the window's picture of it (see
//! `new_ui`).

use super::super::domain::pages::{showing, Before, Ctl, PageFrame, Showing};
use super::super::interface::output::{Area, Hits};
use glam::Vec2;
use omsi_ui::menu_page::{self, Control, Dropdown, Layout, Lead, Page, Pane, Row};
use omsi_ui::pause_menu;
use omsi_ui::widgets::Ui;
use omsi_ui::{Color, Rect};

fn control(c: &Ctl) -> Control<'_> {
    match c {
        Ctl::None => Control::None,
        Ctl::Switch(on) => Control::Switch(*on),
        Ctl::Slider(frac, value) => Control::Slider { frac: *frac, value },
        Ctl::Stepper(v) => Control::Stepper(v),
        Ctl::Opens(v) => Control::Opens(v),
        Ctl::Info(v) => Control::Info(v),
        Ctl::Editing(v) => Control::Editing(v),
        Ctl::Button(v) => Control::Button(v),
    }
}

/// `r` (points, at `s` pixels each, in an area whose corner is at `o`) as the HUD's
/// rectangles are: `[x0, y0, x1, y1]` in window pixels.
pub fn px(r: Rect, s: f32, o: Vec2) -> [f32; 4] {
    [
        o.x + r.x * s,
        o.y + r.y * s,
        o.x + r.right() * s,
        o.y + r.bottom() * s,
    ]
}

impl PageFrame {
    pub(crate) fn with_page<R>(&self, f: impl FnOnce(&Page) -> R) -> R {
        let rows: Vec<Row> = self
            .rows
            .iter()
            .map(|r| Row {
                label: &r.label,
                desc: &r.desc,
                control: control(&r.ctl),
                lead: match &r.lead {
                    Before::None => Lead::None,
                    Before::Sign(s) => Lead::Sign(s),
                    Before::Code(s) => Lead::Code(s),
                },
                heading: r.heading,
            })
            .collect();
        let tabs: Vec<&str> = self.tabs.iter().map(String::as_str).collect();
        let pane_rows: Vec<(&str, &str)> = self
            .pane
            .iter()
            .flat_map(|p| p.rows.iter().map(|(a, b)| (a.as_str(), b.as_str())))
            .collect();
        let dd_items: Vec<&str> = self
            .dropdown
            .iter()
            .flat_map(|d| d.items.iter().map(String::as_str))
            .collect();
        let page = Page {
            title: &self.title,
            note: &self.note,
            tabs_title: &self.tabs_title,
            tabs: &tabs,
            tab: self.tab,
            rows: &rows,
            fields: self.fields,
            selected: self.selected,
            kbd: self.kbd,
            active: self.active,
            top: self.top,
            back: self.back,
            pane: self.pane.as_ref().map(|p| Pane {
                title: &p.title,
                meta: &p.meta,
                rows: &pane_rows,
                chosen: p.chosen,
                button: p.button.as_deref(),
                time: p.time.as_deref(),
            }),
            pane_first: self.pane_first,
            dropdown: self.dropdown.as_ref().map(|d| Dropdown {
                row: d.row,
                items: &dd_items,
                sel: d.sel,
                top: d.top,
                current: d.current,
            }),
        };
        f(&page)
    }

    /// Pixels per design point in a window of `size` pixels: the column's.
    pub(crate) fn scale(&self, size: (u32, u32)) -> f32 {
        self.column.scale(size)
    }

    /// The room of the page in a window of `pts` points: right of the column, under the bar.
    fn area(pts: Vec2) -> Rect {
        Rect::new(
            pause_menu::COLUMN_W,
            pause_menu::BAR_H,
            (pts.x - pause_menu::COLUMN_W).max(1.0),
            (pts.y - pause_menu::BAR_H).max(1.0),
        )
    }

    /// Where the page's parts are in a window of `size` pixels (in points), and the scale.
    pub fn layout(&self, size: (u32, u32)) -> (f32, Layout) {
        let s = self.scale(size);
        let pts = Vec2::new(size.0 as f32, size.1 as f32) / s;
        (s, self.with_page(|p| menu_page::layout(p, Self::area(pts))))
    }

    /// Hand the HUD the rectangles the page is drawn in, as its own list would have set them,
    /// for its mouse code (a row, a slider's track, a page, the way back, a drop-down's entry,
    /// the timetable's stops, its arrows and its button, the scroll bars): in window pixels,
    /// the page laid out in `area` (the centre screen of a triple screen, else the window).
    pub fn write_hits(&self, hits: &mut Hits, area: Area) {
        let (s, l) = self.layout(area.size);
        let o = area.origin;
        let px = |r: Rect| px(r, s, o);
        hits.menu_rects = l.rows.iter().map(|r| px(*r)).collect();
        hits.menu_ctl = l.controls.iter().map(|c| c.map(px)).collect();
        hits.menu_arrows.clear();
        hits.menu_side = l.tabs.iter().map(|r| px(*r)).collect();
        hits.menu_start = l.start;
        hits.menu_rows = l.shown;
        hits.menu_row_h = l.row_h * s;
        hits.menu_scroll_track = l.scroll.map(|(t, _)| px(t));
        hits.menu_scroll_thumb = l.scroll.map(|(_, g)| px(g));
        hits.menu_pane = l.pane_rows.iter().map(|r| px(*r)).collect();
        hits.menu_pane_start = l.pane_first;
        hits.menu_pane_go = l.pane_go.map(px);
        hits.menu_pane_box = l.pane_card.filter(|_| l.pane_go.is_some()).map(px);
        hits.menu_pane_scroll = l.pane_scroll.map(|(t, g, n, fit)| (px(t), px(g), n, fit));
        hits.menu_time = l.time.iter().map(|r| px(*r)).collect();
        hits.dd_rects = l.dd.iter().map(|r| px(*r)).collect();
        hits.dd_top = l.dd_top;
        if l.dd_panel.is_some() {
            hits.dd_rows = l.dd_rows;
        }
        hits.dd_scroll = l.dd_scroll.map(|(t, g)| (px(t), px(g)));
    }

    /// Draw the page over a window of `size` points: the world dimmed, the column and its bar,
    /// the page right of them.
    pub(crate) fn draw(&self, ui: &mut Ui, size: Vec2, logo: Option<(usize, f32)>) {
        ui.p().rect(
            Rect::new(0.0, 0.0, size.x, size.y),
            Color::hex(0x000000).alpha(0.35),
        );
        self.column.with_view(
            |v| {
                pause_menu::draw(ui, v);
            },
            logo,
        );
        let area = Self::area(size);
        // (the room of the page a little darker still: the cards stand out of the world)
        ui.p().rect(area, Color::hex(0x000000).alpha(0.25));
        self.with_page(|p| {
            menu_page::draw(ui, p, area);
        });
    }
}

impl crate::app::App {
    /// Whether the new interface draws the open list (a page behind the Esc menu): a list is
    /// open from the Esc menu.
    pub(crate) fn new_pages_active(&self) -> bool {
        showing(self.game_menu.is_some(), self.chooser.is_some()) == Showing::Page
    }
}
