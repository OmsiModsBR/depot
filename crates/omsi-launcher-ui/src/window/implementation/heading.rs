//! A page's heading, where its tabs go and a card's titles.

use super::super::interface::output::{kit, mobile, shell, Page, Ui};
use glam::Vec2;
use omsi_ui::paint::Align;
use omsi_ui::theme::{TEXT, TEXT_DIM};
use omsi_ui::{Rect, Weight};

/// A page's heading in the new look, as Vehicles and Maps draw theirs (see `shell::heading`):
/// "PAGE › PLACE" small and letter-spaced, the title large and a line on what the page is for
/// (`Page::heading`). Returns the page's body under it.
pub fn page_title(ui: &mut Ui, r: Rect, page: Page) -> Rect {
    let h = page.heading().unwrap_or((page.title(), page.title(), ""));
    page_title_with(ui, r, page.title(), h)
}

/// `page_title` with its parts given: the page's name, (where on it, the title, the line).
/// A phone keeps its compact heading: the name and the line.
pub fn page_title_with(ui: &mut Ui, r: Rect, name: &str, parts: (&str, &str, &str)) -> Rect {
    page_title_beside(ui, r, name, parts, 0.0)
}

/// `page_title_with` for a page whose tabs, `tabs_w` wide, sit at the right of its title
/// (`title_tabs`): the title is cut short of them.
pub fn page_title_beside(
    ui: &mut Ui,
    r: Rect,
    name: &str,
    (place, title, line): (&str, &str, &str),
    tabs_w: f32,
) -> Rect {
    if mobile() {
        ui.text(
            name,
            Vec2::new(r.x, r.y + 22.0),
            22.0,
            Weight::Bold,
            TEXT,
            Align::Left,
        );
        if !line.is_empty() {
            // (a narrow window: the line stops short of the tabs some pages put top right,
            // it ran under them on a phone)
            let w = if r.w < 1100.0 { r.w - 340.0 } else { r.w };
            ui.text_in(
                line,
                Rect::new(r.x, r.y + 34.0, w, 20.0),
                12.5,
                Weight::Regular,
                TEXT_DIM,
                Align::Left,
            );
        }
        return Rect::new(r.x, r.y + 64.0, r.w, (r.h - 64.0).max(0.0));
    }
    // (the heading starts at the window's top, as Vehicles' and Maps': the page's area
    // leaves a margin above it)
    let top = r.y - 28.0;
    let title_w = if tabs_w > 0.0 {
        (r.w - tabs_w - 24.0).max(0.0)
    } else {
        r.w
    };
    shell::heading_beside(ui, name, (place, title, line), r.x, top, r.w, title_w);
    let y = top + shell::BODY_TOP;
    Rect::new(r.x, y, r.w, (r.bottom() - y).max(0.0))
}

/// Where a page puts its tabs, `w` wide, over its `body` (as `page_title` returned it): at
/// the right of the title on a computer, of the line on a phone.
pub fn title_tabs(body: Rect, w: f32) -> Rect {
    let y = if mobile() {
        body.y - 46.0
    } else {
        body.y - shell::BODY_TOP + 56.0
    };
    Rect::new(body.right() - w, y, w, 34.0)
}

/// A card's title (`kit::heading`, letter-spaced) at the top of `r`, the panel less its
/// padding; returns the rest of `r` under it. A phone keeps the compact one.
pub fn card_title(ui: &mut Ui, r: Rect, title: &str) -> Rect {
    if mobile() {
        ui.heading(r, title, None)
    } else {
        kit::heading(ui, r, title)
    }
}

/// The title of a part further down a card, letter-spaced and quiet (a phone: as a card's).
pub fn part_title(ui: &mut Ui, r: Rect, title: &str) {
    if mobile() {
        ui.heading(r, title, None);
    } else {
        kit::subheading(ui, r, title);
    }
}
