//! The Diagnostics page: what to look at, or send along, when something goes wrong - this
//! build, the OMSI 2 folder and whether its essential files are there, the game's log, and
//! the games running now. Nothing on it changes a setting.

use super::{Launcher, Page};
use glam::Vec2;
use omsi_launcher_lib as core;
use omsi_launcher_ui::kit::{action, card, clipped, dot, text, Status};
use omsi_launcher_ui::look::*;
use omsi_ui::{Rect, Weight};

/// The page in `area` (the sidebar is outside it).
pub fn draw(l: &mut Launcher, area: Rect) {
    let top = area.y - 28.0;
    omsi_launcher_ui::shell::page_heading(
        &mut l.ui,
        omsi_launcher_ui::Page::Diagnostics,
        area.x,
        top,
    );
    let y = top + omsi_launcher_ui::shell::BODY_TOP;
    let col = (area.w - 16.0) * 0.5;
    let left = Rect::new(area.x, y, col, 300.0);
    let right = Rect::new(area.x + col + 16.0, y, col, 300.0);
    build_card(l, left);
    folder_card(l, right);
    let y2 = y + 316.0;
    let rest = (area.bottom() - y2 - 24.0).max(200.0);
    log_card(l, Rect::new(area.x, y2, col, rest));
    games_card(l, Rect::new(area.x + col + 16.0, y2, col, rest));
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
    clipped(
        &mut l.ui,
        value,
        r.x + 170.0,
        y,
        r.w - 190.0,
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
    dot(&mut l.ui, r.x + 24.0, r.y + 112.0, status);
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
    line(l, r, r.y + 108.0, "Launcher", &core::data_dir().join("launcher.log").to_string_lossy());
    line(l, r, r.y + 144.0, "Preview helper", &super::preview::helper_log().to_string_lossy());
    l.ui.paragraph(
        "Each is written again when it starts (the game's with each game). A game started with RUST_LOG=debug writes more.",
        Vec2::new(r.x + 19.0, r.y + 172.0),
        r.w - 38.0,
        12.5,
        Weight::Regular,
        MUTED,
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

fn games_card(l: &mut Launcher, r: Rect) {
    let running: Vec<String> = l
        .state
        .instances
        .iter()
        .filter(|i| i.running)
        .map(|i| format!("{} (process {})", super::state::short_map(&i.map), i.pid))
        .collect();
    card(
        &mut l.ui,
        r,
        "Games running",
        Some(&running.len().to_string()),
    );
    if running.is_empty() {
        clipped(
            &mut l.ui,
            "No game started from here is running.",
            r.x + 19.0,
            r.y + 72.0,
            r.w - 38.0,
            13.0,
            Weight::Regular,
            MUTED,
        );
    } else {
        let mut y = r.y + 72.0;
        for g in running.iter().take(4) {
            dot(&mut l.ui, r.x + 24.0, y, Status::Found);
            clipped(
                &mut l.ui,
                g,
                r.x + 42.0,
                y,
                r.w - 61.0,
                13.0,
                Weight::Regular,
                FG,
            );
            y += 30.0;
        }
    }
    let b = Rect::new(
        r.x + 19.0,
        (r.y + 140.0).max(r.bottom() - 84.0),
        r.w - 38.0,
        44.0,
    );
    if action(
        &mut l.ui,
        "diagnostics:sessions",
        b,
        "OPEN SESSIONS",
        "Each game's log as it runs, and its Stop button.",
        true,
    ) {
        l.go(Page::Sessions);
    }
}

/// `text` for a URL's query (everything but the unreserved characters as `%XX`).
fn url_encode(text: &str) -> String {
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

#[cfg(test)]
mod tests {
    #[test]
    fn a_report_body_is_encoded_for_the_issue_link() {
        assert_eq!(super::url_encode("a b\n(c)"), "a%20b%0A%28c%29");
        assert_eq!(super::url_encode("Depot-0.2_x.~"), "Depot-0.2_x.~");
    }
}
