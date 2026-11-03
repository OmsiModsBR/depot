//! The launcher's dialogs over the page that tell how a game started from here ended: the
//! game's crash report and the server's disconnect message.

use super::super::domain::frame::Frame;
use glam::Vec2;
use omsi_ui::paint::Align;
use omsi_ui::theme::*;
use omsi_ui::widgets::ButtonKind;
use omsi_ui::{Rect, Weight};

/// The run went down while a Vulkan driver compiled the shaders: the LAST it said was a stage
/// of that, and it drew with Vulkan (as the phone's shell decides it, `android.rs`). Any
/// compile stage anywhere in the log said so of every silent end - a phone run out of memory
/// 75 % into loading a map on OpenGL was told its Vulkan driver had failed (#848).
pub fn died_compiling_on_vulkan(log: &str) -> bool {
    let last = log
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("");
    let compiling = last.contains("renderer: compiling")
        || last.contains("cloud noise made")
        || last.contains("opening graphics device")
        || last.contains("compiling renderer pipelines");
    let vulkan = log.lines().any(|l| {
        (l.contains("renderer: ") || l.contains("opening graphics device")) && l.contains("(Vulkan")
    }) || log
        .lines()
        .any(|l| l.contains("graphics: ") && l.to_ascii_uppercase().contains("VULKAN"));
    compiling && vulkan
}

/// A game started from here was sent away by its server (kicked, banned) or turned away at
/// the door: the game is over, and this says so with the server's own message.
pub fn disconnect_dialog(f: &mut Frame) {
    let Some(why) = f.state.disconnected.clone() else {
        return;
    };
    let size = f.ui.size;
    let full = Rect::new(0.0, 0.0, size.x, size.y);
    f.ui.solid(full);
    f.ui.p().rect(full, super::dialog::SCRIM);
    let w = (size.x - 48.0).min(560.0);
    let lead = omsi_ui::tr("The server ended your game. Its message:");
    let th =
        f.ui.paragraph_height(&why, w - 48.0, 14.0, Weight::Regular)
            .min(size.y * 0.4);
    let h = (176.0 + th).min(size.y - 24.0);
    let r = Rect::new((size.x - w) * 0.5, (size.y - h) * 0.5, w, h);
    f.ui.panel(r);
    let inner = Rect::new(r.x + 24.0, r.y + 20.0, r.w - 48.0, r.h - 40.0);
    f.ui.icon(
        "error",
        Vec2::new(inner.x + 14.0, inner.y + 14.0),
        26.0,
        DANGER,
    );
    f.ui.text_in(
        "Disconnected from the server",
        Rect::new(inner.x + 38.0, inner.y, inner.w - 38.0, 28.0),
        18.0,
        Weight::Bold,
        TEXT,
        Align::Left,
    );
    f.ui.paragraph(
        &lead,
        Vec2::new(inner.x, inner.y + 40.0),
        inner.w,
        13.0,
        Weight::Regular,
        TEXT_DIM,
    );
    f.ui.push_clip(Rect::new(inner.x, inner.y + 66.0, inner.w, th + 4.0), 0.0);
    f.ui.paragraph(
        &why,
        Vec2::new(inner.x, inner.y + 66.0),
        inner.w,
        14.0,
        Weight::Bold,
        TEXT,
    );
    f.ui.pop_clip();
    let by = inner.bottom() - 38.0;
    if f.ui.button(
        "disconnect-close",
        Rect::new(inner.right() - 110.0, by, 110.0, 38.0),
        "Close",
        None,
        ButtonKind::Primary,
    ) {
        f.state.disconnected = None;
    }
}

/// A game started from here ended on an error: what it said, and the ways to report it
/// (the end of its log copied, or a GitHub issue opened with it).
/// Returns whether the player asked for Settings (a game the system closed).
pub fn crash_dialog(f: &mut Frame) -> bool {
    let Some((what, tail)) = f.state.crash.clone() else {
        return false;
    };
    let size = f.ui.size;
    let full = Rect::new(0.0, 0.0, size.x, size.y);
    f.ui.solid(full);
    f.ui.p().rect(full, super::dialog::SCRIM);
    let w = (size.x - 48.0).min(640.0);
    let lost = what.contains("graphics device was lost");
    let silent = what.contains("closed without a word");
    let compiling = silent && died_compiling_on_vulkan(&tail);
    let hint = if lost {
        if cfg!(windows) {
            "The graphics driver stopped the game. Updating the graphics driver usually helps; you can also let the game draw with DirectX 12 instead of Vulkan (the button below, or Settings → Graphics API)."
        } else {
            "The graphics driver stopped the game. Updating the graphics driver usually helps; Settings → Graphics API can switch to OpenGL."
        }
    } else if compiling {
        "The Vulkan graphics driver stopped while compiling shaders. Starting the game again will switch to OpenGL (or change it in Settings → Graphics API)."
    } else if silent {
        "The system closed the game while it was running, typically because the device ran out of memory (RAM). Lowering texture resolution or reducing AI traffic in Settings helps prevent memory exhaustion."
    } else {
        "Copy the report (the end of the game's log), or open a GitHub issue with it: it tells what went wrong on this computer."
    };
    let text = format!("{what}\n\n{hint}");
    let th =
        f.ui.paragraph_height(&text, w - 48.0, 13.0, Weight::Regular)
            .min(size.y * 0.5);
    let h = (140.0 + th).min(size.y - 24.0);
    let r = Rect::new((size.x - w) * 0.5, (size.y - h) * 0.5, w, h);
    f.ui.panel(r);
    let inner = Rect::new(r.x + 24.0, r.y + 20.0, r.w - 48.0, r.h - 40.0);
    f.ui.icon(
        if silent { "info" } else { "error" },
        Vec2::new(inner.x + 14.0, inner.y + 14.0),
        26.0,
        if silent { WARN } else { DANGER },
    );
    let title_text = if silent {
        "The game was closed by the system"
    } else {
        "The game closed on an error"
    };
    f.ui.text_in(
        title_text,
        Rect::new(inner.x + 38.0, inner.y, inner.w - 38.0, 28.0),
        18.0,
        Weight::Bold,
        TEXT,
        Align::Left,
    );
    f.ui.push_clip(Rect::new(inner.x, inner.y + 40.0, inner.w, th + 4.0), 0.0);
    f.ui.paragraph(
        &text,
        Vec2::new(inner.x, inner.y + 40.0),
        inner.w,
        13.0,
        Weight::Regular,
        TEXT_DIM,
    );
    f.ui.pop_clip();
    let by = inner.bottom() - 38.0;
    if f.ui.button(
        "crash-close",
        Rect::new(inner.right() - 110.0, by, 110.0, 38.0),
        "Close",
        None,
        ButtonKind::Normal,
    ) {
        f.state.crash = None;
    }
    if f.ui.button(
        "crash-copy",
        Rect::new(inner.right() - 270.0, by, 150.0, 38.0),
        "Copy report",
        Some("content_copy"),
        ButtonKind::Primary,
    ) {
        f.ui.clipboard_out = Some(format!(
            "Project Depot {} ({})\n{what}\n\n{tail}",
            f.host.version().0,
            std::env::consts::OS
        ));
        f.state.set_status(
            "The report is copied: paste it into a GitHub issue or a message.",
            false,
        );
    }
    let api = f
        .state
        .settings
        .get("graphics_api")
        .and_then(|v| v.as_str())
        .unwrap_or("auto")
        .to_string();
    if lost
        && cfg!(windows)
        && api != "dx12"
        && f.ui.button(
            "crash-dx12",
            Rect::new(inner.x + 200.0, by, 170.0, 38.0),
            "Use DirectX 12",
            Some("monitor"),
            ButtonKind::Normal,
        )
    {
        f.state.settings["graphics_api"] = serde_json::json!("dx12");
        f.state.settings_dirty = 0.3;
        f.state.crash = None;
        f.state.set_status("The game draws with DirectX 12 from the next start (Settings → Graphics API to change it back).", false);
    }
    if silent {
        if f.ui.button(
            "crash-settings",
            Rect::new(inner.x, by, 150.0, 38.0),
            "Settings",
            Some("tune"),
            ButtonKind::Ghost,
        ) {
            f.state.crash = None;
            return true;
        }
    } else if f.ui.button(
        "crash-issue",
        Rect::new(inner.x, by, 190.0, 38.0),
        "Report on GitHub",
        Some("open_in_new"),
        ButtonKind::Ghost,
    ) {
        let title = format!("Crash: {}", what.chars().take(80).collect::<String>());
        let enc = |t: &str| {
            t.bytes()
                .map(|b| {
                    if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                        (b as char).to_string()
                    } else {
                        format!("%{b:02X}")
                    }
                })
                .collect::<String>()
        };
        // the end of the log goes with it, as much as a link holds (a report of the
        // last line alone said where the game stopped, never what led there); the whole
        // report is on the clipboard as well
        // (the computer and the map always, see `crash_of`)
        let (machine, end) = tail
            .split_once(&format!("\n{}\n", crate::state::CRASH_TAIL_GAP))
            .unwrap_or(("", &tail));
        let machine = if machine.is_empty() {
            String::new()
        } else {
            format!("The computer:\n```\n{machine}\n```\n\n")
        };
        let body_with = |end: &str| {
            format!("Project Depot {} on {}\n\n```\n{what}\n```\n\n{machine}The end of the log:\n```\n{end}\n```\n", f.host.version().0, std::env::consts::OS)
        };
        let lines: Vec<&str> = end.lines().collect();
        let mut shown = 0;
        let body = loop {
            let body = body_with(&lines[lines.len() - shown..].join("\n"));
            if shown >= lines.len() || enc(&body).len() > 6500 {
                break if shown == 0 {
                    body
                } else {
                    body_with(&lines[lines.len() - shown.saturating_sub(1)..].join("\n"))
                };
            }
            shown += 1;
        };
        f.ui.clipboard_out = Some(format!(
            "Project Depot {} ({})\n{what}\n\n{tail}",
            f.host.version().0,
            std::env::consts::OS
        ));
        f.host.open_url(&format!(
            "{}/issues/new?title={}&body={}",
            f.host.repo_url(),
            enc(&title),
            enc(&body)
        ));
    }
    false
}
