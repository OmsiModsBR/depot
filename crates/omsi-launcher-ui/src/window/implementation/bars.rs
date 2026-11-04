//! What lies around the pages: the sidebar or the top bar, the foot of the window with its
//! status line, and the banner over the window while a game runs.

use super::super::domain::frame::Frame;
use super::super::interface::output::mobile;
use crate::shell::Nav;
use crate::Page;
use glam::Vec2;
use omsi_ui::paint::Align;
use omsi_ui::theme::*;
use omsi_ui::widgets::{ButtonKind, Ui};
use omsi_ui::{Rect, Weight};

/// Over the launcher's last picture while a game runs: why the launcher does not move.
pub fn game_banner(ui: &mut Ui) {
    let size = ui.size;
    let full = Rect::new(0.0, 0.0, size.x, size.y);
    ui.solid(full);
    ui.p().rect(full, omsi_ui::Color::rgba(0, 0, 0, 0.62));
    let text = "The launcher rests while you drive, so that the game has the graphics card to itself. It is back as soon as the game ends.";
    let w = (size.x - 48.0).min(520.0);
    let th = ui.paragraph_height(text, w - 48.0, 13.0, Weight::Regular);
    let h = 80.0 + th;
    let r = Rect::new((size.x - w) * 0.5, (size.y - h) * 0.5, w, h);
    ui.panel(r);
    let inner = Rect::new(r.x + 24.0, r.y + 20.0, r.w - 48.0, r.h - 40.0);
    ui.icon(
        "directions_bus",
        Vec2::new(inner.x + 14.0, inner.y + 14.0),
        26.0,
        accent(),
    );
    ui.text_in(
        "The game is running",
        Rect::new(inner.x + 38.0, inner.y, inner.w - 38.0, 28.0),
        18.0,
        Weight::Bold,
        TEXT,
        Align::Left,
    );
    ui.paragraph(
        text,
        Vec2::new(inner.x, inner.y + 40.0),
        inner.w,
        13.0,
        Weight::Regular,
        TEXT_DIM,
    );
}

/// The sidebar over the page's left side (a scrolled page passes under it), or the top
/// bar over its top: the pages, and the driver at the sidebar's foot.
/// Returns what was clicked.
pub fn nav(
    f: &mut Frame,
    page: Page,
    logo: Option<(usize, f32)>,
    top: bool,
    compact: bool,
) -> Option<Nav> {
    let size = f.ui.size;
    let jobs = f.state.jobs.iter().filter(|j| j.finished.is_none()).count();
    let mut badges = Vec::new();
    if jobs > 0 {
        badges.push((Page::Mods, jobs.to_string(), accent()));
    }
    let (level, name) = match &f.state.profile {
        Some(p) => (p.level, p.name.clone()),
        None => (1, f.state.config.profile.clone()),
    };
    // (no driver yet: no level to show, and the foot asks for one - it opens Profile)
    let ask = name.is_empty();
    let name = if ask { "No driver".to_string() } else { name };
    let level = if ask {
        "Create".to_string()
    } else {
        omsi_ui::tr("Level %{level}").replace("%{level}", &level.to_string())
    };
    // the Depot | OMSI 2 switch: only with the flag on and OmsiLaunch installed
    let engine = f
        .state
        .original
        .switch_shown(&f.state.config.original_engine)
        .then(|| crate::shell::EngineSwitch {
            omsi2: f.state.original.engine(&f.state.config.original_engine)
                == omsi_launcher_lib::original_engine::Engine::Omsi2,
            locked: f.state.in_game(),
        });
    let nav = if top {
        crate::shell::TopBar {
            page,
            logo: logo,
            badges: &badges,
            engine,
        }
        .draw(f.ui, size.x)
    } else {
        crate::shell::Sidebar {
            page,
            logo: logo,
            badges: &badges,
            footer: (&name, &level),
            engine,
            footer_ask: ask,
            compact,
        }
        .draw(f.ui, size.y)
    };
    if let Some(Nav::Engine(omsi2)) = nav {
        f.state.engine_set_mode(if omsi2 {
            omsi_launcher_lib::original_engine::Engine::Omsi2
        } else {
            omsi_launcher_lib::original_engine::Engine::Depot
        });
        return None;
    }
    nav
}

/// The foot of the page (on a computer): nothing while all is well. A problem or a game
/// running shows as one line, Windows 11's InfoBar in small: the OMSI 2 folder not chosen
/// (with the button that fixes it) or the game running. The preview helper is no status:
/// the picture it draws says so itself (`preview_badge`). Returns where the message line
/// can start.
fn chips(f: &mut Frame, x: f32, cy: f32, setup: &mut bool) -> f32 {
    let games = f.state.instances.iter().filter(|i| i.running).count();
    let (tone, icon, text, action) = if !f.root.ok(&f.state.config.root) {
        (
            WARN,
            "warning",
            "OMSI 2 folder: not chosen",
            Some("Open Setup"),
        )
    } else if games > 0 {
        (accent(), "info", "Game: running", None)
    } else {
        return x;
    };
    let button_w = action.map_or(0.0, |a| f.ui.width(a, 12.0, Weight::Medium) + 24.0);
    let w = 36.0 + f.ui.width(text, 12.5, Weight::Regular) + 14.0 + button_w + 6.0;
    let r = Rect::new(x, cy - 16.0, w, 32.0);
    let a = f.ui.appear(
        omsi_ui::widgets::id_of("status:bar"),
        omsi_ui::theme::MOTION_NORMAL,
    );
    f.ui.solid(r);
    f.ui.p().rounded(
        r,
        omsi_ui::theme::corner(RADIUS),
        omsi_ui::theme::surface(PANEL.mix(tone, 0.12)),
    );
    f.ui.p()
        .rounded_border(r, omsi_ui::theme::corner(RADIUS), 1.0, tone.alpha(0.35));
    f.ui.push_fade(a);
    f.ui.icon(icon, Vec2::new(r.x + 18.0, cy), 16.0, tone);
    f.ui.text_in(
        text,
        Rect::new(r.x + 36.0, r.y, w - 36.0 - button_w, r.h),
        12.5,
        Weight::Medium,
        TEXT,
        Align::Left,
    );
    if let Some(label) = action {
        let b = Rect::new(r.right() - button_w - 4.0, r.y + 4.0, button_w, 24.0);
        if f.ui
            .button("status:action", b, label, None, ButtonKind::Normal)
        {
            *setup = true;
        }
    }
    f.ui.pop_fade();
    r.right() + 12.0
}

/// The foot of the window; `rail_w` is what the rail takes of it. Returns whether Setup was
/// asked for.
pub fn status_bar(f: &mut Frame, rail_w: f32) -> bool {
    let mut setup = false;
    let size = f.ui.size;
    let left = if mobile() {
        rail_w + 20.0
    } else {
        chips(f, rail_w + 28.0, size.y - 22.0, &mut setup)
    };
    let (text, err, at) = f.state.status.clone();
    if text.is_empty() {
        return setup;
    }
    let fade = if err {
        1.0
    } else {
        (1.0 - (at.elapsed().as_secs_f32() - 6.0) / 1.5).clamp(0.0, 1.0)
    };
    if fade <= 0.0 {
        return setup;
    }
    let first = text.lines().next().unwrap_or("").to_string();
    let r = Rect::new(left, size.y - 34.0, (size.x - left - 20.0).max(0.0), 24.0);
    if mobile() {
        // (readable over a page scrolled under it)
        f.ui.p().rect(
            Rect::new(rail_w, size.y - 34.0, size.x - rail_w, 34.0),
            RAIL.alpha(0.92),
        );
    }
    let c = if err { DANGER } else { TEXT_DIM };
    f.ui.text_in(&first, r, 12.0, Weight::Regular, c.alpha(fade), Align::Left);
    f.ui.tooltip(r, &text);
    setup
}
