//! The launcher made for a phone (or a tablet), not the desktop's pages squeezed onto it: the
//! tab bar, the Play screen and its sheets, and More are drawn by `omsi_launcher_ui::window::phone`;
//! the window draws the desktop page it shows, full width, between it and its foot.

use super::{stage, Launcher, Page};
use omsi_launcher_ui::window::{self, Tab};
use omsi_launcher_ui::Page as UiPage;

/// The whole launcher on a phone (instead of the rail and the desktop pages).
pub fn draw(l: &mut Launcher) {
    let mut room = stage::GameShowroom {
        preview: &mut l.preview,
        tex: l.preview_tex,
        rect: &mut l.preview_rect,
    };
    let shown = window::phone(
        frame!(l),
        &mut l.phone,
        &mut room,
        &mut l.page_scroll,
        &mut l.page_max,
    );
    if shown.sheet {
        return;
    }
    if let Some((page, content)) = shown.page {
        page_in(l, page, content);
    }
    let tab = window::phone_foot(frame!(l), &mut l.phone, &mut l.page_scroll, &shown);
    if let Some(page) = shown.open {
        l.go(Page::of(page));
    }
    match tab {
        Some(Tab::Mods) => l.go(Page::Mods),
        Some(Tab::Play) => l.go(Page::Drive),
        Some(Tab::Online) => l.go(Page::Multiplayer),
        Some(Tab::More) | None => {}
    }
    window::phone_toast(frame!(l), &shown);
}

/// One of the desktop's pages in `content`, full width.
fn page_in(l: &mut Launcher, page: UiPage, content: omsi_ui::Rect) {
    match page {
        UiPage::Drive => l.drive_page(content),
        // (the phone chooses the bus and the map in its sheets)
        UiPage::Vehicles | UiPage::Maps => l.drive_page(content),
        UiPage::Diagnostics => super::diagnostics::draw(l, content),
        UiPage::Profile => {
            omsi_launcher_ui::window::profile(frame!(l), &mut l.pages.profile, content)
        }
        UiPage::Settings => l.settings_page(content),
        UiPage::Controls => super::pages::controls(l, content),
        UiPage::Mods => omsi_launcher_ui::window::mods(frame!(l), content),
        UiPage::Tutorials => omsi_launcher_ui::window::tutorials(frame!(l), content),
        UiPage::Timetable => l.timetable_page(content),
        UiPage::Setup => l.setup_page(content),
    }
}
