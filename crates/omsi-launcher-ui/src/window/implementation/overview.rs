//! The Diagnostics page's Overview tab: this build, the OMSI 2 folder and whether its essential
//! files are there, the logs, and what the launcher watches (in the bar of the tabs). Nothing
//! on it changes a setting, but the detailed log.

use super::super::domain::{frame::Frame, overview::Overview};
use super::super::interface::output::{core, root_problem, Page, Ui};
use crate::kit::{action, card, clipped, dot, text, Status};
use crate::look::*;
use glam::Vec2;
use omsi_ui::{Rect, Weight};

/// The first tab: this build, the folder, the log and the games running.
pub fn diagnostics_overview(f: &mut Frame, area: Rect, seen: &Overview) -> Option<Page> {
    let y = area.y;
    let col = (area.w - 16.0) * 0.5;
    let left = Rect::new(area.x, y, col, 300.0);
    let right = Rect::new(area.x + col + 16.0, y, col, 300.0);
    build_card(f, left, seen);
    let open = folder_card(f, right);
    let y2 = y + 316.0;
    let rest = (area.bottom() - y2 - 24.0).max(200.0);
    log_card(f, Rect::new(area.x, y2, area.w, rest));
    open
}

/// What the launcher watches, side by side in the bar of the tabs: the game, the OMSI 2 folder and the preview
/// helper (the pills that were at the foot of every page). The dot is green when the thing is
/// as it should be, amber when the folder is missing, and none while there is nothing to say.
pub fn diagnostics_status(f: &mut Frame, r: Rect, seen: &Overview) {
    if r.w < 360.0 {
        return;
    }
    let games = f.state.instances.iter().any(|i| i.running);
    let root_ok = f.root.ok(&f.state.config.root);
    let helper = seen.preview_live;
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
        f.ui.icon(icon, Vec2::new(x + 10.0, cy), 16.0, MUTED);
        if let Some(c) = dot {
            f.ui.p().circle(Vec2::new(x + 30.0, cy), 3.0, c);
        }
        f.ui.text_in_tip(
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
fn line(ui: &mut Ui, r: Rect, y: f32, label: &str, value: &str) {
    ui.p()
        .rect(Rect::new(r.x + 1.0, y - 20.0, r.w - 2.0, 1.0), LINE);
    text(ui, label, r.x + 19.0, y, 13.0, Weight::Regular, MUTED);
    // (the value starts after the label when a translation is longer than the usual column)
    let label_w = ui
        .fonts
        .width(&omsi_ui::tr(label), 13.0 * TEXT_SCALE, Weight::Regular);
    let x = (r.x + 170.0).max(r.x + 19.0 + label_w + 14.0);
    clipped(
        ui,
        value,
        x,
        y,
        (r.right() - 20.0 - x).max(40.0),
        13.0,
        Weight::Regular,
        FG,
    );
}

fn build_card(f: &mut Frame, r: Rect, seen: &Overview) {
    let (version, build) = f.host.version();
    card(f.ui, r, "This build", None);
    line(f.ui, r, r.y + 72.0, "Version", version);
    line(f.ui, r, r.y + 112.0, "Build", build);
    line(f.ui, r, r.y + 152.0, "System", std::env::consts::OS);
    let adapter = if seen.adapter.is_empty() {
        "not known yet".to_string()
    } else {
        seen.adapter.to_string()
    };
    line(f.ui, r, r.y + 192.0, "Graphics", &adapter);
    let b = Rect::new(r.x + 19.0, r.bottom() - 84.0, r.w - 38.0, 44.0);
    if action(
        f.ui,
        "diagnostics:report",
        b,
        "REPORT A PROBLEM",
        "Opens a new issue on the project's page, with this build filled in.",
        true,
    ) {
        let body = format!(
            "Project Depot {} (build {}, {})\n\nWhat happened:\n",
            version,
            build,
            std::env::consts::OS
        );
        f.host.open_url(&format!(
            "{}/issues/new?body={}",
            f.host.repo_url(),
            url_encode(&body)
        ));
    }
}

/// Returns the page to open (Setup) when its button is pressed.
fn folder_card(f: &mut Frame, r: Rect) -> Option<Page> {
    let root = f.state.config.root.clone();
    let missing = omsi_cfg::missing_original_essentials(std::path::Path::new(&root));
    card(f.ui, r, "OMSI 2 folder", None);
    line(
        f.ui,
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
        (Status::Missing, root_problem(&root))
    };
    // (the dot at the middle of the paragraph's first line)
    dot(f.ui, r.x + 24.0, r.y + 129.0, status);
    f.ui.paragraph(
        &said,
        Vec2::new(r.x + 42.0, r.y + 117.0),
        r.w - 61.0,
        13.0,
        Weight::Regular,
        status.color(),
    );
    let b = Rect::new(r.x + 19.0, r.bottom() - 84.0, r.w - 38.0, 44.0);
    if action(
        f.ui,
        "diagnostics:setup",
        b,
        "OPEN SETUP",
        "Choose the folder or look at what is installed.",
        true,
    ) {
        return Some(Page::Setup);
    }
    None
}

fn log_card(f: &mut Frame, r: Rect) {
    let log = core::data_dir().join("game.log");
    card(f.ui, r, "The game's log", None);
    line(f.ui, r, r.y + 72.0, "File", &log.to_string_lossy());
    line(
        f.ui,
        r,
        r.y + 108.0,
        "Launcher",
        &core::data_dir().join("launcher.log").to_string_lossy(),
    );
    line(
        f.ui,
        r,
        r.y + 144.0,
        "Preview helper",
        &f.host.helper_log().to_string_lossy(),
    );
    // (the next games started from here write what they load and every failure)
    let mut detailed = f
        .state
        .settings
        .get("detailed_log")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let row = Rect::new(r.x + 19.0, r.y + 166.0, r.w - 38.0, 32.0);
    if f.ui.toggle(
        "diagnostics:detailed-log",
        row,
        &mut detailed,
        "Detailed log",
    ) {
        f.state.settings["detailed_log"] = serde_json::json!(detailed);
        f.state.settings_dirty = 0.3;
    }
    f.ui.tooltip(
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
        f.ui,
        "diagnostics:copy-log",
        b,
        "COPY ITS PATH",
        "The path goes on the clipboard, to paste where you send it from.",
        true,
    ) {
        f.ui.clipboard_out = Some(log.to_string_lossy().to_string());
        f.state
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
