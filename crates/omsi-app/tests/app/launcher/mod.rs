mod ai_edit;
mod ai_traffic;
mod backdrop;
mod chrome;
mod diagnostics;
mod dialogs;
mod drive;
mod integrity;
mod keyboard;
mod mapview;
mod original_ui;
mod pages;
mod prerender;
mod settings;
mod setup;
mod start;
mod thumbs;
mod timetable_plan;
mod tray;
mod ui;

use omsi_launcher_ui::Page as UiPage;
use openomsi_game::testing::launcher::Page;

/// Every page of the sidebar is a page of the launcher and back, and the names the input
/// script and `OMSI_LAUNCHER_PAGE` use find them.
#[test]
fn the_sidebar_pages_are_the_launchers() {
    for p in UiPage::ALL {
        let page = Page::of(p);
        assert_eq!(page.shown(), Some(p));
        assert_eq!(Page::named(&p.title().to_lowercase()), Some(page));
    }
    assert_eq!(Page::Multiplayer.shown(), None);
    assert_eq!(Page::named("nope"), None);
}
