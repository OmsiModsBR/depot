//! The Diagnostics page: what to look at, or send along, when something goes wrong - this
//! build, the OMSI 2 folder and whether its essential files are there, the game's log
//! (`omsi_launcher_ui::window::diagnostics_overview`), a check of a map or a vehicle
//! (`window::integrity_check`) and a map's AI traffic (`ai_traffic`). Its tabs are drawn here.

use super::{Launcher, Page};
use omsi_launcher_ui::window;
use omsi_ui::Rect;

/// The Diagnostics page's tab and what its tabs keep.
#[derive(Default)]
pub struct DiagView {
    /// The page's tab: 0 the overview, 1 the integrity check, 2 the AI traffic.
    pub tab: usize,
    /// The Integrity tab (`omsi_launcher_ui::window::integrity_check`).
    pub check: omsi_launcher_ui::window::CheckView,
    /// The AI traffic tab.
    pub(super) ai: super::ai_traffic::AiView,
}

impl DiagView {
    /// A window lies over the page (the page sees no mouse meanwhile).
    pub fn dialog_open(&self) -> bool {
        (self.tab == 1 && self.check.export_open()) || (self.tab == 2 && self.ai.dialog_open())
    }

    /// The AI traffic tab shows one of its vehicles in the preview.
    pub fn shows_vehicle(&self) -> bool {
        self.tab == 2 && self.ai.shows_vehicle()
    }
}

/// The page in `area` (the sidebar is outside it).
pub fn draw(l: &mut Launcher, area: Rect) {
    // (a phone keeps its compact heading, as the other pages)
    let body = l.page_title(area, omsi_launcher_ui::Page::Diagnostics);
    let bar = Rect::new(body.x, body.y, body.w, 36.0);
    let mut tab = l.pages.diag.tab;
    let tabs_r = Rect::new(bar.x, bar.y, 420.0f32.min(bar.w), bar.h);
    if l.ui.segmented(
        "diag-tab",
        tabs_r,
        &mut tab,
        &["Overview", "Integrity", "AI traffic"],
    ) {
        l.pages.diag.tab = tab;
    }
    l.ui.segmented_tips(
        tabs_r,
        &[
            "This build, the OMSI 2 folder, the game's log and the games running.",
            "A complete check of a map or a vehicle, on request.",
            "A map's AI lists as the game takes them on a chosen day: car groups, bus fleet, parked cars, people and traffic density.",
        ],
    );
    let below = Rect::new(area.x, body.y + 52.0, area.w, area.bottom() - body.y - 52.0);
    match l.pages.diag.tab {
        1 => window::integrity_check(frame!(l), &mut l.pages.diag.check, below),
        2 => super::ai_traffic::draw(l, below),
        _ => {
            // (the status at the right of the tabs, where the bar has room)
            let at = tabs_r.right() + 16.0;
            let adapter = l.adapter.clone();
            let seen = window::Overview {
                preview_live: l.preview.live,
                adapter: &adapter,
            };
            let strip = Rect::new(at, bar.y, (bar.right() - at).max(0.0), bar.h);
            window::diagnostics_status(frame!(l), strip, &seen);
            if let Some(p) = window::diagnostics_overview(frame!(l), below, &seen) {
                l.go(Page::of(p));
            }
        }
    }
}

/// The window over the page: the AI traffic's editor, or the integrity report's export.
pub fn dialog(l: &mut Launcher) {
    if l.pages.diag.tab == 2 {
        super::ai_edit::dialog(l);
    } else {
        window::integrity_export(frame!(l), &mut l.pages.diag.check);
    }
}
