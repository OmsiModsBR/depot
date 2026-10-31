//! The Profile page: the driver chosen, their record and the drives they finished, and a
//! drive's details over it.

use super::super::domain::{frame::Frame, profile::ProfileView};
use super::super::interface::output::{core, kit, mobile, short_map, tr, Page};
use super::dialog::{dialog, dialog_foot};
use super::heading::{card_title, page_title};
use glam::Vec2;
use omsi_ui::paint::Align;
use omsi_ui::theme::{accent, EDGE, GAP, ROW, TEXT, TEXT_DIM};
use omsi_ui::widgets::{id_of, ButtonKind, Key};
use omsi_ui::{Color, Rect, Weight};

pub fn profile(f: &mut Frame, view: &mut ProfileView, area: Rect) {
    let body = page_title(f.ui, area, Page::Profile);
    if mobile() {
        // (a phone: the driver and the record on the left, the drives on the right)
        let left_w = (body.w * 0.56).min(700.0);
        let left = Rect::new(body.x, body.y, left_w, body.h);
        let right = Rect::new(
            body.x + left_w + GAP * 2.0,
            body.y,
            body.w - left_w - GAP * 2.0,
            body.h,
        );
        let top = Rect::new(left.x, left.y, left.w, 132.0);
        driver_card(f, view, top);
        record_card(
            f,
            Rect::new(
                left.x,
                top.bottom() + GAP * 1.5,
                left.w,
                left.h - top.h - GAP * 1.5,
            ),
        );
        recent_drives(f, view, right);
        return;
    }
    // (the driver and their record on the left, the drives finished on the right)
    let upper = body.h;
    let right_w = kit::fit(440.0, body.w);
    let left = Rect::new(body.x, body.y, body.w - right_w - GAP * 1.5, upper);
    let top = Rect::new(left.x, left.y, left.w, 132.0);
    driver_card(f, view, top);
    record_card(
        f,
        Rect::new(
            left.x,
            top.bottom() + GAP * 1.5,
            left.w,
            (left.h - top.h - GAP * 1.5).max(0.0),
        ),
    );
    recent_drives(
        f,
        view,
        Rect::new(left.right() + GAP * 1.5, body.y, right_w, upper),
    );
}

/// The driver: the one chosen, deleting them, and a new one.
fn driver_card(f: &mut Frame, view: &mut ProfileView, top: Rect) {
    f.ui.panel(top);
    let inner = card_title(
        f.ui,
        Rect::new(top.x + 18.0, top.y + 14.0, top.w - 36.0, top.h - 28.0),
        "Driver",
    );
    let names = f.state.profiles.clone();
    let mut sel = names
        .iter()
        .position(|n| *n == f.state.config.profile)
        .unwrap_or(0);
    let half = (inner.w - GAP) * 0.5;
    if !names.is_empty()
        && f.ui.select(
            "profile",
            Rect::new(inner.x, inner.y, half, ROW),
            &mut sel,
            &names,
        )
    {
        f.state.config.profile = names[sel].clone();
        let _ = core::save_config(&f.state.config);
        f.state.load_profile();
        f.state.touched();
        view.drive = 0;
    }
    let danger_armed = view
        .confirm_delete
        .map(|t| t.elapsed().as_secs() < 4)
        .unwrap_or(false);
    // (no driver yet: nothing to delete)
    if !names.is_empty()
        && f.ui.button(
            "profile-delete",
            Rect::new(inner.x + half + GAP, inner.y, half, ROW),
            if danger_armed {
                "Click again to delete"
            } else {
                "Delete this driver"
            },
            Some("delete"),
            ButtonKind::Danger,
        )
    {
        if danger_armed {
            let name = f.state.config.profile.clone();
            match core::delete_profile(&name) {
                Ok(()) => {
                    f.state.profile = None;
                    f.state.config.profile.clear();
                    let _ = core::save_config(&f.state.config);
                    f.state.set_status(
                        tr("Personnel file of %{name} deleted.").replace("%{name}", &name),
                        false,
                    );
                    f.state.load_profiles();
                }
                Err(e) => f.state.set_status(format!("{e:#}"), true),
            }
            view.confirm_delete = None;
        } else {
            view.confirm_delete = Some(std::time::Instant::now());
        }
    }
    // (with no driver yet the new one's line takes the first row)
    let y = if names.is_empty() {
        inner.y
    } else {
        inner.y + ROW + 10.0
    };
    f.ui.text_input(
        "new-driver",
        Rect::new(inner.x, y, half, ROW),
        &mut view.new_driver,
        "New driver's name",
        Some("person"),
    );
    if f.ui.button(
        "profile-create",
        Rect::new(inner.x + half + GAP, y, half, ROW),
        "Create",
        Some("add"),
        ButtonKind::Normal,
    ) {
        let name = view.new_driver.trim().to_string();
        if !name.is_empty() {
            match core::create_profile(&name, "M") {
                Ok(_) => {
                    f.state.config.profile = name.clone();
                    let _ = core::save_config(&f.state.config);
                    view.new_driver.clear();
                    f.state.load_profiles();
                    f.state.set_status(
                        tr("Driver %{name} created.").replace("%{name}", &name),
                        false,
                    );
                }
                Err(e) => f.state.set_status(format!("{e:#}"), true),
            }
        }
    }
}

/// What the driver's record says, as the tiles of the Profile page show it: (icon, value, what).
fn record_tiles(p: &core::Profile) -> [(&'static str, String, &'static str); 12] {
    let hours = |h: f64| {
        tr("%{h} h %{m} min")
            .replace("%{h}", &(h.floor() as i64).to_string())
            .replace(
                "%{m}",
                &format!("{:02}", ((h - h.floor()) * 60.0).round() as i64),
            )
    };
    let on_time = (p.stops - p.early - p.late).max(0);
    [
        ("schedule", hours(p.hours), "hours driven"),
        ("route", format!("{:.1} km", p.km), "distance"),
        ("location_on", p.stops.to_string(), "stops served"),
        (
            "timer",
            format!("{on_time} / {} / {}", p.early, p.late),
            "on time / early / late",
        ),
        (
            "confirmation_number",
            format!("{:.0}", p.tickets),
            "tickets sold",
        ),
        ("payments", format!("{:.2}", p.cash), "takings"),
        ("warning", p.crashes.to_string(), "crashes"),
        ("person", p.hurt.to_string(), "pedestrians hurt"),
        ("speed", format!("{:.0} %", p.rating_driving), "driving"),
        (
            "airport_shuttle",
            format!("{:.0} %", p.rating_comfort),
            "comfort",
        ),
        (
            "receipt_long",
            format!("{:.0} %", p.rating_tickets),
            "ticket sales",
        ),
        ("history", p.drives.to_string(), "trips"),
    ]
}

/// The driver's level and record.
fn record_card(f: &mut Frame, card: Rect) {
    f.ui.panel(card);
    let Some(p) = f.state.profile.clone() else {
        f.ui.text_in(
            "Create a driver to start a personnel file.",
            card.pad(20.0, 20.0),
            14.0,
            Weight::Medium,
            TEXT_DIM,
            Align::Left,
        );
        return;
    };
    let c = Vec2::new(card.x + 64.0, card.y + 64.0);
    let prev = ((p.level - 1) * (p.level - 1) * 250) as f64;
    let frac =
        ((p.xp as f64 - prev) / (p.next_level_xp as f64 - prev).max(1.0)).clamp(0.0, 1.0) as f32;
    let shown = f.ui.anim(id_of("xp-ring"), frac, 0.6);
    f.ui.p().circle(c, 46.0, Color::rgba(28, 31, 37, 1.0));
    f.ui.p().arc(
        c,
        39.0,
        46.0,
        0.0,
        std::f32::consts::TAU,
        Color::WHITE.alpha(0.08),
    );
    let a0 = -std::f32::consts::FRAC_PI_2;
    f.ui.p().arc(
        c,
        39.0,
        46.0,
        a0,
        a0 + std::f32::consts::TAU * shown.max(0.01),
        accent(),
    );
    f.ui.text_in(
        &p.level.to_string(),
        Rect::new(c.x - 40.0, c.y - 24.0, 80.0, 36.0),
        30.0,
        Weight::Black,
        TEXT,
        Align::Center,
    );
    f.ui.text_in(
        "LEVEL",
        Rect::new(c.x - 40.0, c.y + 10.0, 80.0, 16.0),
        10.0,
        Weight::Black,
        TEXT_DIM,
        Align::Center,
    );
    let tx = card.x + 130.0;
    f.ui.text_in(
        &format!(
            "{}{}",
            p.name,
            if p.exists {
                ""
            } else {
                " (no personnel file yet)"
            }
        ),
        Rect::new(tx, card.y + 26.0, card.w - 148.0, 30.0),
        20.0,
        Weight::Bold,
        TEXT,
        Align::Left,
    );
    f.ui.progress(
        Rect::new(tx, card.y + 62.0, card.w - 152.0, 8.0),
        shown,
        false,
    );
    f.ui.text_in(
        &tr("%{xp} XP · %{left} to level %{level}")
            .replace("%{xp}", &p.xp.to_string())
            .replace("%{left}", &(p.next_level_xp - p.xp).max(0).to_string())
            .replace("%{level}", &(p.level + 1).to_string()),
        Rect::new(tx, card.y + 78.0, card.w - 148.0, 18.0),
        12.0,
        Weight::Regular,
        TEXT_DIM,
        Align::Left,
    );
    let grid = Rect::new(card.x + 18.0, card.y + 126.0, card.w - 36.0, card.h - 140.0);
    let gap = 10.0;
    // (three across as the concept has them; four across, three rows, where four rows do not
    // fit - a 720 p window - and the tiles a little lower still if need be)
    let cols = if grid.h >= 4.0 * 62.0 + 3.0 * gap || grid.w < 560.0 {
        3
    } else {
        4
    };
    let rows = 12_usize.div_ceil(cols);
    let cw = (grid.w - gap * (cols as f32 - 1.0)) / cols as f32;
    let ch = ((grid.h - gap * (rows as f32 - 1.0)) / rows as f32).clamp(48.0, 62.0);
    for (k, (icon, v, label)) in record_tiles(&p).iter().enumerate() {
        let (cx, cy) = ((k % cols) as f32, (k / cols) as f32);
        let r = Rect::new(grid.x + cx * (cw + gap), grid.y + cy * (ch + gap), cw, ch);
        if r.bottom() > card.bottom() - 6.0 {
            break;
        }
        let top = r.y + (ch - 48.0) * 0.4;
        f.ui.p().rounded(r, 6.0, Color::WHITE.alpha(0.04));
        f.ui.p().rounded_border(r, 6.0, 1.0, EDGE);
        f.ui.icon(icon, Vec2::new(r.x + 22.0, top + 18.0), 15.0, accent());
        f.ui.text_in(
            v,
            Rect::new(r.x + 36.0, top + 5.0, r.w - 44.0, 26.0),
            16.5,
            Weight::Bold,
            TEXT,
            Align::Left,
        );
        f.ui.text_in(
            label,
            Rect::new(r.x + 14.0, r.bottom() - 24.0, r.w - 20.0, 18.0),
            12.0,
            Weight::Regular,
            TEXT_DIM,
            Align::Left,
        );
    }
}

/// The drive's title in the lists: its line (and tour) or a free drive, and the map.
pub fn drive_title(s: &core::Session) -> String {
    match &s.line {
        Some(line) => tr("Line %{line} · %{map}")
            .replace(
                "%{line}",
                &format!(
                    "{line}{}",
                    s.tour
                        .as_ref()
                        .map(|t| format!(" / {t}"))
                        .unwrap_or_default()
                ),
            )
            .replace("%{map}", &short_map(&s.map)),
        None => tr("Free drive · %{map}").replace("%{map}", &short_map(&s.map)),
    }
}

/// The drives the driver finished, most recent first: one chosen, its details in a window.
fn recent_drives(f: &mut Frame, view: &mut ProfileView, right: Rect) {
    f.ui.panel(right);
    let inner = card_title(
        f.ui,
        Rect::new(
            right.x + 18.0,
            right.y + 14.0,
            right.w - 36.0,
            right.h - 28.0,
        ),
        "Recent drives",
    );
    let Some(p) = f.state.profile.clone() else {
        return;
    };
    if !mobile() {
        let note = kit::fit_text(
            f.ui,
            &tr("%{name} · most recent first").replace("%{name}", &p.name),
            12.0,
            inner.w - 150.0,
        );
        f.ui.text_in(
            &note,
            Rect::new(inner.x, right.y + 14.0, inner.w, 18.0),
            12.0,
            Weight::Regular,
            TEXT_DIM,
            Align::Right,
        );
    }
    let sessions = p.sessions.clone();
    let foot = !sessions.is_empty() && !mobile();
    let list_h = if foot { inner.h - 58.0 } else { inner.h };
    let chosen = view.drive.min(sessions.len().saturating_sub(1));
    let mut picked = None;
    let mut open = false;
    f.ui.scroll_area("runs", Rect::new(inner.x - 6.0, inner.y, inner.w + 12.0, list_h), &mut |ui, v| {
        if sessions.is_empty() {
            ui.text_in("No drives yet. Start one from Maps and it shows up here.", Rect::new(v.x + 8.0, v.y, v.w, 30.0), 13.0, Weight::Regular, TEXT_DIM, Align::Left);
            return 30.0;
        }
        let rh = 74.0;
        for (k, s) in sessions.iter().enumerate() {
            let r = Rect::new(v.x + 6.0, v.y + k as f32 * rh, v.w - 16.0, rh - 8.0);
            let (hover, _, clicked) = ui.interact_key(id_of(&format!("drive-{k}")), r);
            let on = k == chosen;
            ui.p().rounded(r, 6.0, if on { accent().alpha(0.12) } else if hover { Color::WHITE.alpha(0.06) } else { Color::WHITE.alpha(0.0) });
            if on {
                ui.p().rounded_border(r, 6.0, 1.0, accent().alpha(0.6));
            }
            if clicked {
                // (a click on the one chosen opens its details)
                open = on;
                picked = Some(k);
            }
            ui.text_in(&drive_title(s), Rect::new(r.x + 10.0, r.y + 6.0, r.w - 80.0, 20.0), 13.5, Weight::Bold, TEXT, Align::Left);
            ui.text_in(&hours_short(s.seconds / 3600.0), Rect::new(r.right() - 80.0, r.y + 6.0, 70.0, 20.0), 12.0, Weight::Bold, accent(), Align::Right);
            ui.text_in(&tr("%{bus} · %{km} km · %{stops} stops · %{tickets} tickets · %{crashes} crashes").replace("%{bus}", s.bus.rsplit('/').next().unwrap_or("")).replace("%{km}", &format!("{:.1}", s.metres / 1000.0)).replace("%{stops}", &s.stops.to_string()).replace("%{tickets}", &s.tickets.to_string()).replace("%{crashes}", &s.crashes.to_string()), Rect::new(r.x + 10.0, r.y + 26.0, r.w - 20.0, 18.0), 12.0, Weight::Regular, TEXT_DIM, Align::Left);
            ui.text_in(&chrono_like(s.time), Rect::new(r.x + 10.0, r.y + 44.0, r.w - 20.0, 18.0), 12.0, Weight::Regular, TEXT_DIM, Align::Left);
        }
        sessions.len() as f32 * rh
    });
    if let Some(k) = picked {
        view.drive = k;
    }
    if foot {
        let line_y = inner.y + list_h + 8.0;
        f.ui.p()
            .rect(Rect::new(right.x + 1.0, line_y, right.w - 2.0, 1.0), EDGE);
        if f.ui.button(
            "drive-details",
            Rect::new(inner.x, line_y + 12.0, inner.w, 34.0),
            "Open details",
            None,
            ButtonKind::Normal,
        ) {
            open = true;
        }
    }
    if open {
        view.drive_details = true;
    }
}

/// What a drive's details window lists: (what, the value).
pub fn drive_rows(s: &core::Session) -> Vec<(&'static str, String)> {
    let on_time = (s.stops - s.early - s.late).max(0);
    vec![
        ("Map", short_map(&s.map)),
        ("Bus", s.bus.rsplit('/').next().unwrap_or("").to_string()),
        (
            "Line",
            s.line
                .as_ref()
                .map(|l| {
                    format!(
                        "{l}{}",
                        s.tour
                            .as_ref()
                            .map(|t| format!(" / {t}"))
                            .unwrap_or_default()
                    )
                })
                .unwrap_or_else(|| "Free drive".into()),
        ),
        ("Started", chrono_like(s.time)),
        ("Time driven", hours_short(s.seconds / 3600.0)),
        ("Distance", format!("{:.1} km", s.metres / 1000.0)),
        ("Stops served", s.stops.to_string()),
        (
            "On time / early / late",
            format!("{on_time} / {} / {}", s.early, s.late),
        ),
        ("Tickets sold", s.tickets.to_string()),
        ("Takings", format!("{:.2}", s.cash)),
        ("Crashes", s.crashes.to_string()),
        ("Pedestrians hurt", s.hurt.to_string()),
        ("Jolts", s.jolts.to_string()),
        ("Driving", format!("{:.0} %", s.driving)),
        ("Comfort", format!("{:.0} %", s.comfort)),
        ("Ticket sales", format!("{:.0} %", s.ticketing)),
    ]
}

/// The window over Profile with the chosen drive's details.
pub fn drive_dialog(f: &mut Frame, view: &mut ProfileView) {
    let s = f
        .state
        .profile
        .as_ref()
        .and_then(|p| {
            p.sessions
                .get(view.drive.min(p.sessions.len().saturating_sub(1)))
        })
        .cloned();
    let Some(s) = s else {
        view.drive_details = false;
        return;
    };
    if f.ui.input.keys.contains(&Key::Escape) {
        view.drive_details = false;
        return;
    }
    let rows = drive_rows(&s);
    let inside = dialog(
        f.ui,
        520.0,
        56.0 + rows.len() as f32 * 26.0 + 90.0,
        &drive_title(&s),
        "as saved after the drive",
    );
    let (buttons, body) = dialog_foot(f.ui, inside);
    for (k, (what, value)) in rows.iter().enumerate() {
        let y = body.y + k as f32 * 26.0;
        if y + 22.0 > body.bottom() {
            break;
        }
        f.ui.text_in(
            what,
            Rect::new(body.x, y, 200.0, 22.0),
            13.0,
            Weight::Regular,
            TEXT_DIM,
            Align::Left,
        );
        f.ui.text_in(
            value,
            Rect::new(body.x + 200.0, y, body.w - 200.0, 22.0),
            13.0,
            Weight::Medium,
            TEXT,
            Align::Left,
        );
    }
    if f.ui.button(
        "drive-close",
        Rect::new(buttons.right() - 110.0, buttons.y, 110.0, 34.0),
        "Close",
        None,
        ButtonKind::Normal,
    ) {
        view.drive_details = false;
    }
}

fn hours_short(h: f64) -> String {
    format!(
        "{}:{:02} h",
        h.floor() as i64,
        ((h - h.floor()) * 60.0).round() as i64
    )
}

/// A Unix time as "YYYY-MM-DD HH:MM" in the machine's time zone.
fn chrono_like(t: u64) -> String {
    #[cfg(unix)]
    {
        let tt = t as libc::time_t;
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        if !unsafe { libc::localtime_r(&tt, &mut tm) }.is_null() {
            return format!(
                "{:04}-{:02}-{:02} {:02}:{:02}",
                tm.tm_year + 1900,
                tm.tm_mon + 1,
                tm.tm_mday,
                tm.tm_hour,
                tm.tm_min
            );
        }
    }
    let days = (t / 86400) as i64;
    let secs = t % 86400;
    // civil from days (Howard Hinnant), UTC
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02} UTC",
        secs / 3600,
        (secs % 3600) / 60
    )
}
