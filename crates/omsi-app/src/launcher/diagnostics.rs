//! The Diagnostics page: what to look at, or send along, when something goes wrong - this
//! build, the OMSI 2 folder and whether its essential files are there, the game's log.
//! Nothing on it changes a setting.

use super::{Launcher, Page};
use glam::Vec2;
use omsi_launcher_lib as core;
use omsi_launcher_ui::kit::{action, card, clipped, dot, text, Status};
use omsi_launcher_ui::look::*;
use omsi_ui::{Rect, Weight};

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
        1 => super::integrity::draw(l, below),
        2 => super::ai_traffic::draw(l, below),
        _ => {
            // (the status at the right of the tabs, where the bar has room)
            let at = tabs_r.right() + 16.0;
            status_strip(l, Rect::new(at, bar.y, (bar.right() - at).max(0.0), bar.h));
            overview(l, below)
        }
    }
}

/// The first tab: this build, the folder, the log and the games running.
fn overview(l: &mut Launcher, area: Rect) {
    let y = area.y;
    let col = (area.w - 16.0) * 0.5;
    let left = Rect::new(area.x, y, col, 300.0);
    let right = Rect::new(area.x + col + 16.0, y, col, 300.0);
    build_card(l, left);
    folder_card(l, right);
    let y2 = y + 316.0;
    let rest = (area.bottom() - y2 - 24.0).max(200.0);
    log_card(l, Rect::new(area.x, y2, area.w, rest));
}

/// What the launcher watches, side by side in the bar of the tabs: the game, the OMSI 2 folder and the preview
/// helper (the pills that were at the foot of every page). The dot is green when the thing is
/// as it should be, amber when the folder is missing, and none while there is nothing to say.
fn status_strip(l: &mut Launcher, r: Rect) {
    if r.w < 360.0 {
        return;
    }
    let games = l.state.instances.iter().any(|i| i.running);
    let root_ok = l.root_ok();
    let helper = l.preview.live;
    let rows: [(&str, &str, Option<omsi_ui::Color>); 3] = [
        (
            "o-play",
            if games {
                "Game: running"
            } else {
                "Game: not running"
            },
            games.then_some(OK),
        ),
        (
            "o-folder",
            if root_ok {
                "OMSI 2 folder: read"
            } else {
                "OMSI 2 folder: not chosen"
            },
            Some(if root_ok { OK } else { WARN }),
        ),
        (
            "o-package",
            if helper {
                "Preview helper: running, stops when the page closes"
            } else {
                "Preview helper: stopped"
            },
            helper.then_some(OK),
        ),
    ];
    let col = r.w / 3.0;
    for (i, (icon, text, dot)) in rows.into_iter().enumerate() {
        let x = r.x + col * i as f32;
        let cy = r.center().y;
        l.ui.icon(icon, Vec2::new(x + 10.0, cy), 16.0, MUTED);
        if let Some(c) = dot {
            l.ui.p().circle(Vec2::new(x + 30.0, cy), 3.0, c);
        }
        l.ui.text_in_tip(
            text,
            Rect::new(x + 40.0, cy - 12.0, col - 48.0, 24.0),
            12.5,
            Weight::Regular,
            FG,
            omsi_ui::paint::Align::Left,
        );
    }
}

/// A label and its value on one line of a card, `y` its middle.
fn line(l: &mut Launcher, r: Rect, y: f32, label: &str, value: &str) {
    l.ui.p()
        .rect(Rect::new(r.x + 1.0, y - 20.0, r.w - 2.0, 1.0), LINE);
    text(
        &mut l.ui,
        label,
        r.x + 19.0,
        y,
        13.0,
        Weight::Regular,
        MUTED,
    );
    // (the value starts after the label when a translation is longer than the usual column)
    let label_w =
        l.ui.fonts
            .width(&omsi_ui::tr(label), 13.0 * TEXT_SCALE, Weight::Regular);
    let x = (r.x + 170.0).max(r.x + 19.0 + label_w + 14.0);
    clipped(
        &mut l.ui,
        value,
        x,
        y,
        (r.right() - 20.0 - x).max(40.0),
        13.0,
        Weight::Regular,
        FG,
    );
}

fn build_card(l: &mut Launcher, r: Rect) {
    card(&mut l.ui, r, "This build", None);
    line(l, r, r.y + 72.0, "Version", crate::startup::VERSION);
    line(l, r, r.y + 112.0, "Build", crate::startup::BUILD);
    line(l, r, r.y + 152.0, "System", std::env::consts::OS);
    let adapter = if l.adapter.is_empty() {
        "not known yet".to_string()
    } else {
        l.adapter.clone()
    };
    line(l, r, r.y + 192.0, "Graphics", &adapter);
    let b = Rect::new(r.x + 19.0, r.bottom() - 84.0, r.w - 38.0, 44.0);
    if action(
        &mut l.ui,
        "diagnostics:report",
        b,
        "REPORT A PROBLEM",
        "Opens a new issue on the project's page, with this build filled in.",
        true,
    ) {
        let body = format!(
            "Project Depot {} (build {}, {})\n\nWhat happened:\n",
            crate::startup::VERSION,
            crate::startup::BUILD,
            std::env::consts::OS
        );
        crate::about::open_url(&format!(
            "{}/issues/new?body={}",
            crate::about::REPO_URL,
            url_encode(&body)
        ));
    }
}

fn folder_card(l: &mut Launcher, r: Rect) {
    let root = l.state.config.root.clone();
    let missing = omsi_cfg::missing_original_essentials(std::path::Path::new(&root));
    card(&mut l.ui, r, "OMSI 2 folder", None);
    line(
        l,
        r,
        r.y + 72.0,
        "Folder",
        if root.trim().is_empty() {
            "not chosen"
        } else {
            &root
        },
    );
    let (status, said) = if missing.is_empty() {
        (Status::Found, "The essential files are there.".to_string())
    } else {
        (Status::Missing, super::state::root_problem(&root))
    };
    // (the dot at the middle of the paragraph's first line)
    dot(&mut l.ui, r.x + 24.0, r.y + 129.0, status);
    l.ui.paragraph(
        &said,
        Vec2::new(r.x + 42.0, r.y + 117.0),
        r.w - 61.0,
        13.0,
        Weight::Regular,
        status.color(),
    );
    let b = Rect::new(r.x + 19.0, r.bottom() - 84.0, r.w - 38.0, 44.0);
    if action(
        &mut l.ui,
        "diagnostics:setup",
        b,
        "OPEN SETUP",
        "Choose the folder or look at what is installed.",
        true,
    ) {
        l.go(Page::Setup);
    }
}

fn log_card(l: &mut Launcher, r: Rect) {
    let log = core::data_dir().join("game.log");
    card(&mut l.ui, r, "The game's log", None);
    line(l, r, r.y + 72.0, "File", &log.to_string_lossy());
    line(
        l,
        r,
        r.y + 108.0,
        "Launcher",
        &core::data_dir().join("launcher.log").to_string_lossy(),
    );
    line(
        l,
        r,
        r.y + 144.0,
        "Preview helper",
        &super::preview::helper_log().to_string_lossy(),
    );
    // (the next games started from here write what they load and every failure)
    let mut detailed = l
        .state
        .settings
        .get("detailed_log")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let row = Rect::new(r.x + 19.0, r.y + 166.0, r.w - 38.0, 32.0);
    if l.ui.toggle(
        "diagnostics:detailed-log",
        row,
        &mut detailed,
        "Detailed log",
    ) {
        l.state.settings["detailed_log"] = serde_json::json!(detailed);
        l.state.settings_dirty = 0.3;
    }
    l.ui.tooltip(
        row,
        "Each is written again when it starts (the game's with each game). With the detailed log on, the next games also write everything they load and every failure, so the file is larger.",
    );
    let b = Rect::new(
        r.x + 19.0,
        (r.y + 206.0).max(r.bottom() - 84.0),
        r.w - 38.0,
        44.0,
    );
    if action(
        &mut l.ui,
        "diagnostics:copy-log",
        b,
        "COPY ITS PATH",
        "The path goes on the clipboard, to paste where you send it from.",
        true,
    ) {
        l.ui.clipboard_out = Some(log.to_string_lossy().to_string());
        l.state
            .set_status("The log's path is on the clipboard.", false);
    }
}

/// `text` for a URL's query (everything but the unreserved characters as `%XX`).
pub fn url_encode(text: &str) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}
