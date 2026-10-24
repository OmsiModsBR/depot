//! The launcher's other pages: the driver's profile, the settings, the key bindings, the
//! running games, the mods and where things are.

use super::state::{fmt_bytes, hhmm, short_map};
use super::theme::*;
use super::ui::{id_of, matches, ButtonKind, Ui};
use super::Launcher;
use glam::Vec2;
use omsi_launcher_lib as core;
use omsi_launcher_ui::Page as UiPage;
use omsi_ui::paint::Align;
use omsi_ui::{Color, Rect, Weight};
use serde_json::{json, Value};

#[derive(Default)]
pub struct PagesView {
    pub new_driver: String,
    pub confirm_delete: Option<std::time::Instant>,
    /// The drive chosen on Profile / Sessions (the driver's drives, most recent first).
    pub drive: usize,
    /// Its details window is open.
    pub drive_details: bool,
    /// The "reset every setting" dialog is open.
    pub confirm_reset: bool,
    pub kb_filter: [String; 2],
    /// The OMSI-style "Add event..." browser is open for a keyboard section.
    pub kb_events: [bool; 2],
    /// (section, index) of the binding waiting for a key.
    pub capturing: Option<(usize, usize)>,
    pub drop_hover: bool,
    /// The Setup page's folders as edited, and its Content card (`setup`).
    pub setup: super::setup::SetupView,
    /// The Controls page's tab: 0 the keyboard, 1 the game controllers.
    pub controls_tab: usize,
    /// The Settings page's section and search (`settings`).
    pub set: super::settings::SettingsView,
    pub pads: PadsView,
    pub tt: super::timetable::TimetableView,
    /// The keyboard tab on a computer (`keyboard`).
    pub kb: super::keyboard::KeyboardView,
}

/// The game controllers tab: the devices `gamectrler.cfg` sets up, the ones connected now,
/// the one shown, a button being waited for.
#[derive(Default)]
pub struct PadsView {
    pub io: Option<crate::controllers::Devices>,
    /// The set-up assistant, while it runs.
    pub wizard: Option<Wizard>,
    feedback_test: bool,
    pub devices: Option<Vec<crate::controllers::DeviceCfg>>,
    pub selected: usize,
    /// Waiting for a button of the shown device to be pressed (to add its binding).
    pub capturing: bool,
    /// A button found through "Add a button", kept visible even past the highlight.
    pub revealed_button: Option<usize>,
    pub dirty: bool,
    /// The button last pressed on the shown device and when: its line is lit, so that one
    /// sees which it is and what it does, and can give it an action there.
    pub last_pressed: Option<(usize, std::time::Instant)>,
    /// "Remove this device" clicked once, and when: a second click removes it.
    confirm_remove: Option<std::time::Instant>,
}

/// The set-up assistant of a device: the player lets go of everything, then turns the wheel
/// to the left and presses each pedal in turn; what moved most each time is that control
/// (and which way it runs), as OMSI's options dialog has the player choose by hand.
pub struct Wizard {
    pub step: usize,
    /// Where each axis rests, and where it stood at each step (left, throttle, brake,
    /// clutch).
    pub rest: [Option<f32>; 8],
    pub at: Vec<[Option<f32>; 8]>,
    pub error: Option<String>,
    calibration: Option<(std::time::Instant, crate::ffb_calibration::Calibration)>,
    ff_choice: Option<bool>,
    test_strength: f32,
}

impl PadsView {
    pub(super) fn cancel_feedback_test(&mut self) {
        release_feedback(&mut self.io, &mut self.feedback_test);
        if let Some((_, test)) = self.wizard.as_mut().and_then(|w| w.calibration.as_mut()) {
            if test.result.is_none() {
                test.fail("The test was interrupted. Please try again.");
            }
        }
    }

    /// Give every controller handle up before handing the hardware to the game. The normal
    /// controller list uses non-exclusive DirectInput too, not only the force-feedback test.
    pub(super) fn release_io(&mut self) {
        self.cancel_feedback_test();
        self.io = None;
    }
}

fn release_feedback(io: &mut Option<crate::controllers::Devices>, active: &mut bool) {
    if *active {
        *io = None;
        *active = false;
    }
}

// --- profile --------------------------------------------------------------------------------

pub fn profile(l: &mut Launcher, area: Rect) {
    let body = l.page_title(area, UiPage::Profile);
    if super::mobile::mobile() {
        // (a phone: the driver and the record on the left, the drives on the right; its games
        // running have a page of their own)
        let left_w = (body.w * 0.56).min(700.0);
        let left = Rect::new(body.x, body.y, left_w, body.h);
        let right = Rect::new(
            body.x + left_w + GAP * 2.0,
            body.y,
            body.w - left_w - GAP * 2.0,
            body.h,
        );
        let top = Rect::new(left.x, left.y, left.w, 132.0);
        driver_card(l, top);
        record_card(
            l,
            Rect::new(
                left.x,
                top.bottom() + GAP * 1.5,
                left.w,
                left.h - top.h - GAP * 1.5,
            ),
        );
        recent_drives(l, right);
        return;
    }
    // (as the concept: the driver and their record on the left, the drives finished on the
    // right, and the games open now along the foot - they belong to the driver too)
    let open = open_sessions_h(l, body.h);
    let upper = (body.h - open - GAP * 1.5).max(0.0);
    let right_w = omsi_launcher_ui::kit::fit(440.0, body.w);
    let left = Rect::new(body.x, body.y, body.w - right_w - GAP * 1.5, upper);
    let top = Rect::new(left.x, left.y, left.w, 132.0);
    driver_card(l, top);
    record_card(
        l,
        Rect::new(
            left.x,
            top.bottom() + GAP * 1.5,
            left.w,
            (left.h - top.h - GAP * 1.5).max(0.0),
        ),
    );
    recent_drives(
        l,
        Rect::new(left.right() + GAP * 1.5, body.y, right_w, upper),
    );
    open_sessions(
        l,
        Rect::new(body.x, body.y + upper + GAP * 1.5, body.w, open),
    );
}

/// The driver: the one chosen, deleting them, and a new one.
fn driver_card(l: &mut Launcher, top: Rect) {
    l.ui.panel(top);
    let inner = Launcher::card_title(
        &mut l.ui,
        Rect::new(top.x + 18.0, top.y + 14.0, top.w - 36.0, top.h - 28.0),
        "Driver",
    );
    let names = l.state.profiles.clone();
    let mut sel = names
        .iter()
        .position(|n| *n == l.state.config.profile)
        .unwrap_or(0);
    let half = (inner.w - GAP) * 0.5;
    if !names.is_empty()
        && l.ui.select(
            "profile",
            Rect::new(inner.x, inner.y, half, ROW),
            &mut sel,
            &names,
        )
    {
        l.state.config.profile = names[sel].clone();
        let _ = core::save_config(&l.state.config);
        l.state.load_profile();
        l.state.touched();
        l.pages.drive = 0;
    }
    let danger_armed = l
        .pages
        .confirm_delete
        .map(|t| t.elapsed().as_secs() < 4)
        .unwrap_or(false);
    // (no driver yet: nothing to delete)
    if !names.is_empty()
        && l.ui.button(
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
            let name = l.state.config.profile.clone();
            match core::delete_profile(&name) {
                Ok(()) => {
                    l.state.profile = None;
                    l.state.config.profile.clear();
                    let _ = core::save_config(&l.state.config);
                    l.state.set_status(
                        omsi_ui::tr("Personnel file of %{name} deleted.").replace("%{name}", &name),
                        false,
                    );
                    l.state.load_profiles();
                }
                Err(e) => l.state.set_status(format!("{e:#}"), true),
            }
            l.pages.confirm_delete = None;
        } else {
            l.pages.confirm_delete = Some(std::time::Instant::now());
        }
    }
    // (with no driver yet the new one's line takes the first row)
    let y = if names.is_empty() {
        inner.y
    } else {
        inner.y + ROW + 10.0
    };
    l.ui.text_input(
        "new-driver",
        Rect::new(inner.x, y, half, ROW),
        &mut l.pages.new_driver,
        "New driver's name",
        Some("person"),
    );
    if l.ui.button(
        "profile-create",
        Rect::new(inner.x + half + GAP, y, half, ROW),
        "Create",
        Some("add"),
        ButtonKind::Normal,
    ) {
        let name = l.pages.new_driver.trim().to_string();
        if !name.is_empty() {
            match core::create_profile(&name, "M") {
                Ok(_) => {
                    l.state.config.profile = name.clone();
                    let _ = core::save_config(&l.state.config);
                    l.pages.new_driver.clear();
                    l.state.load_profiles();
                    l.state.set_status(
                        omsi_ui::tr("Driver %{name} created.").replace("%{name}", &name),
                        false,
                    );
                }
                Err(e) => l.state.set_status(format!("{e:#}"), true),
            }
        }
    }
}

/// What the driver's record says, as the tiles of the Profile page show it: (icon, value, what).
fn record_tiles(p: &core::Profile) -> [(&'static str, String, &'static str); 12] {
    let hours = |h: f64| {
        omsi_ui::tr("%{h} h %{m} min")
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
fn record_card(l: &mut Launcher, card: Rect) {
    l.ui.panel(card);
    let Some(p) = l.state.profile.clone() else {
        l.ui.text_in(
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
    let shown = l.ui.anim(id_of("xp-ring"), frac, 0.6);
    l.ui.p().circle(c, 46.0, Color::rgba(28, 31, 37, 1.0));
    l.ui.p().arc(
        c,
        39.0,
        46.0,
        0.0,
        std::f32::consts::TAU,
        Color::WHITE.alpha(0.08),
    );
    let a0 = -std::f32::consts::FRAC_PI_2;
    l.ui.p().arc(
        c,
        39.0,
        46.0,
        a0,
        a0 + std::f32::consts::TAU * shown.max(0.01),
        accent(),
    );
    l.ui.text_in(
        &p.level.to_string(),
        Rect::new(c.x - 40.0, c.y - 24.0, 80.0, 36.0),
        30.0,
        Weight::Black,
        TEXT,
        Align::Center,
    );
    l.ui.text_in(
        "LEVEL",
        Rect::new(c.x - 40.0, c.y + 10.0, 80.0, 16.0),
        10.0,
        Weight::Black,
        TEXT_DIM,
        Align::Center,
    );
    let tx = card.x + 130.0;
    l.ui.text_in(
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
    l.ui.progress(
        Rect::new(tx, card.y + 62.0, card.w - 152.0, 8.0),
        shown,
        false,
    );
    l.ui.text_in(
        &omsi_ui::tr("%{xp} XP · %{left} to level %{level}")
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
        l.ui.p().rounded(r, 6.0, Color::WHITE.alpha(0.04));
        l.ui.p().rounded_border(r, 6.0, 1.0, EDGE);
        l.ui.icon(icon, Vec2::new(r.x + 22.0, top + 18.0), 15.0, accent());
        l.ui.text_in(
            v,
            Rect::new(r.x + 36.0, top + 5.0, r.w - 44.0, 26.0),
            16.5,
            Weight::Bold,
            TEXT,
            Align::Left,
        );
        l.ui.text_in(
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
fn drive_title(s: &core::Session) -> String {
    match &s.line {
        Some(line) => omsi_ui::tr("Line %{line} · %{map}")
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
        None => omsi_ui::tr("Free drive · %{map}").replace("%{map}", &short_map(&s.map)),
    }
}

/// The drives the driver finished, most recent first: one chosen, its details in a window.
fn recent_drives(l: &mut Launcher, right: Rect) {
    l.ui.panel(right);
    let inner = Launcher::card_title(
        &mut l.ui,
        Rect::new(
            right.x + 18.0,
            right.y + 14.0,
            right.w - 36.0,
            right.h - 28.0,
        ),
        "Recent drives",
    );
    let Some(p) = l.state.profile.clone() else {
        return;
    };
    if !super::mobile::mobile() {
        let note = omsi_launcher_ui::kit::fit_text(
            &mut l.ui,
            &omsi_ui::tr("%{name} · most recent first").replace("%{name}", &p.name),
            12.0,
            inner.w - 150.0,
        );
        l.ui.text_in(
            &note,
            Rect::new(inner.x, right.y + 14.0, inner.w, 18.0),
            12.0,
            Weight::Regular,
            TEXT_DIM,
            Align::Right,
        );
    }
    let sessions = p.sessions.clone();
    let foot = !sessions.is_empty() && !super::mobile::mobile();
    let list_h = if foot { inner.h - 58.0 } else { inner.h };
    let chosen = l.pages.drive.min(sessions.len().saturating_sub(1));
    let mut picked = None;
    let mut open = false;
    l.ui.scroll_area("runs", Rect::new(inner.x - 6.0, inner.y, inner.w + 12.0, list_h), &mut |ui, v| {
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
            ui.text_in(&omsi_ui::tr("%{bus} · %{km} km · %{stops} stops · %{tickets} tickets · %{crashes} crashes").replace("%{bus}", s.bus.rsplit('/').next().unwrap_or("")).replace("%{km}", &format!("{:.1}", s.metres / 1000.0)).replace("%{stops}", &s.stops.to_string()).replace("%{tickets}", &s.tickets.to_string()).replace("%{crashes}", &s.crashes.to_string()), Rect::new(r.x + 10.0, r.y + 26.0, r.w - 20.0, 18.0), 12.0, Weight::Regular, TEXT_DIM, Align::Left);
            ui.text_in(&chrono_like(s.time), Rect::new(r.x + 10.0, r.y + 44.0, r.w - 20.0, 18.0), 12.0, Weight::Regular, TEXT_DIM, Align::Left);
        }
        sessions.len() as f32 * rh
    });
    if let Some(k) = picked {
        l.pages.drive = k;
    }
    if foot {
        let line_y = inner.y + list_h + 8.0;
        l.ui.p()
            .rect(Rect::new(right.x + 1.0, line_y, right.w - 2.0, 1.0), EDGE);
        if l.ui.button(
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
        l.pages.drive_details = true;
    }
}

/// The open sessions card's height for a page `body_h` high: a row for each game, the log
/// under its row when open, a third of the page at most.
fn open_sessions_h(l: &Launcher, body_h: f32) -> f32 {
    let rows = l.state.instances.len().max(1) as f32;
    let logs = l
        .state
        .instances
        .iter()
        .filter(|i| l.state.open_logs.contains(&i.pid))
        .count() as f32;
    (52.0 + rows * 62.0 + logs * 190.0).min((body_h * 0.36).max(118.0))
}

/// The games open now (and those that ended since the launcher started): their log and stop.
fn open_sessions(l: &mut Launcher, r: Rect) {
    l.ui.panel(r);
    let inner = Launcher::card_title(
        &mut l.ui,
        Rect::new(r.x + 18.0, r.y + 14.0, r.w - 36.0, r.h - 28.0),
        "Open sessions",
    );
    l.ui.text_in(
        "games running now",
        Rect::new(inner.x, r.y + 14.0, inner.w, 18.0),
        12.0,
        Weight::Regular,
        TEXT_DIM,
        Align::Right,
    );
    let list = l.state.instances.clone();
    if list.is_empty() {
        l.ui.text_in(
            "No game started from here is running. Start one from Maps.",
            Rect::new(inner.x, inner.y, inner.w, 30.0),
            13.0,
            Weight::Regular,
            TEXT_DIM,
            Align::Left,
        );
        return;
    }
    let names: std::collections::HashMap<String, String> = l
        .state
        .vehicles
        .iter()
        .map(|v| (v.file.clone(), v.name.clone()))
        .collect();
    let short_bus = |b: &str| {
        names.get(b).cloned().unwrap_or_else(|| {
            b.rsplit('/')
                .next()
                .unwrap_or("")
                .trim_end_matches(".bus")
                .to_string()
        })
    };
    let mut actions: Vec<(u32, &str)> = Vec::new();
    let stopping = l.state.stopping.clone();
    let open_logs = l.state.open_logs.clone();
    let logs = l.state.logs.clone();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    l.ui.scroll_area(
        "open-sessions",
        Rect::new(inner.x - 4.0, inner.y - 4.0, inner.w + 8.0, inner.h + 8.0),
        &mut |ui, v| {
            let mut y = v.y + 4.0;
            for i in &list {
                let row = Rect::new(v.x + 4.0, y, v.w - 16.0, 52.0);
                ui.p().rounded(row, 6.0, Color::WHITE.alpha(0.04));
                let running = i.running;
                ui.p().circle(
                    Vec2::new(row.x + 16.0, row.center().y),
                    4.5,
                    if running { OK } else { TEXT_FAINT },
                );
                let on_line = i
                    .line
                    .as_ref()
                    .map(|ln| {
                        omsi_ui::tr(" on line %{line}").replace(
                            "%{line}",
                            &format!(
                                "{ln}{}",
                                i.tour
                                    .as_ref()
                                    .map(|t| format!(" / {t}"))
                                    .unwrap_or_default()
                            ),
                        )
                    })
                    .unwrap_or_default();
                let bus = if i.bus.is_empty() {
                    omsi_ui::tr("on foot").into_owned()
                } else {
                    short_bus(&i.bus)
                };
                let bw = 96.0;
                let buttons = if running { bw * 2.0 + 8.0 } else { bw };
                let tw = row.w - 40.0 - buttons - 16.0;
                ui.text_in(
                    &format!("{} · {bus}{on_line}", short_map(&i.map)),
                    Rect::new(row.x + 30.0, row.y + 7.0, tw, 20.0),
                    13.5,
                    Weight::Medium,
                    TEXT,
                    Align::Left,
                );
                let status = if running {
                    if stopping.contains(&i.pid) || i.stopping.is_some() {
                        omsi_ui::tr("stopping - saving the drive…").into_owned()
                    } else {
                        let t = now.saturating_sub(i.started);
                        omsi_ui::tr("running %{time}").replace(
                            "%{time}",
                            &format!("{:02}:{:02}:{:02}", t / 3600, (t / 60) % 60, t % 60),
                        )
                    }
                } else {
                    let how = if i.exit_code == Some(0) {
                        String::new()
                    } else if i.killed {
                        omsi_ui::tr(" (killed - it did not end by itself, the drive is not saved)")
                            .into_owned()
                    } else {
                        i.exit_code
                            .map(|c| {
                                omsi_ui::tr(" (exit code %{code})")
                                    .replace("%{code}", &c.to_string())
                            })
                            .unwrap_or_default()
                    };
                    omsi_ui::tr("ended%{how}").replace("%{how}", &how)
                };
                ui.text_in(
                    &format!("{} · {status}", i.profile),
                    Rect::new(row.x + 30.0, row.y + 27.0, tw, 18.0),
                    12.0,
                    Weight::Regular,
                    TEXT_DIM,
                    Align::Left,
                );
                let log_open = open_logs.contains(&i.pid);
                let mut bx = row.right() - 10.0 - bw;
                if running {
                    let busy = stopping.contains(&i.pid);
                    if ui.button(
                        &format!("stop-{}", i.pid),
                        Rect::new(bx, row.y + 10.0, bw, 32.0),
                        if busy { "Stopping…" } else { "Stop" },
                        None,
                        ButtonKind::Danger,
                    ) && !busy
                    {
                        actions.push((i.pid, "stop"));
                    }
                    bx -= bw + 8.0;
                }
                if ui.button(
                    &format!("log-{}", i.pid),
                    Rect::new(bx, row.y + 10.0, bw, 32.0),
                    if log_open { "Hide log" } else { "Open log" },
                    None,
                    ButtonKind::Normal,
                ) {
                    actions.push((i.pid, "log"));
                }
                y += 60.0;
                if log_open {
                    let lr = Rect::new(row.x, y - 2.0, row.w, 180.0);
                    ui.p().rounded(lr, 6.0, Color::rgba(6, 8, 10, 0.9));
                    let lines = logs.get(&i.pid).cloned().unwrap_or_default();
                    for (k, line) in lines.iter().rev().take(9).rev().enumerate() {
                        ui.text_in(
                            line,
                            Rect::new(lr.x + 10.0, lr.y + 6.0 + k as f32 * 18.0, lr.w - 20.0, 18.0),
                            11.0,
                            Weight::Regular,
                            if line.contains("ERROR") {
                                DANGER
                            } else if line.contains("WARN") {
                                WARN
                            } else {
                                TEXT_DIM
                            },
                            Align::Left,
                        );
                    }
                    y += 190.0;
                }
            }
            y - v.y
        },
    );
    for (pid, what) in actions {
        match what {
            "stop" => l.state.stop(pid),
            _ => {
                if !l.state.open_logs.remove(&pid) {
                    l.state.open_logs.insert(pid);
                    l.state.log_tail(pid);
                }
            }
        }
    }
}

/// What a drive's details window lists: (what, the value).
fn drive_rows(s: &core::Session) -> Vec<(&'static str, String)> {
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

/// The window over Profile / Sessions with the chosen drive's details.
pub fn drive_dialog(l: &mut Launcher) {
    let s = l
        .state
        .profile
        .as_ref()
        .and_then(|p| {
            p.sessions
                .get(l.pages.drive.min(p.sessions.len().saturating_sub(1)))
        })
        .cloned();
    let Some(s) = s else {
        l.pages.drive_details = false;
        return;
    };
    if l.ui.input.keys.contains(&super::ui::Key::Escape) {
        l.pages.drive_details = false;
        return;
    }
    let rows = drive_rows(&s);
    let inside = super::start::dialog(
        l,
        520.0,
        56.0 + rows.len() as f32 * 26.0 + 90.0,
        &drive_title(&s),
        "as saved after the drive",
    );
    let (buttons, body) = super::start::dialog_foot(l, inside);
    for (k, (what, value)) in rows.iter().enumerate() {
        let y = body.y + k as f32 * 26.0;
        if y + 22.0 > body.bottom() {
            break;
        }
        l.ui.text_in(
            what,
            Rect::new(body.x, y, 200.0, 22.0),
            13.0,
            Weight::Regular,
            TEXT_DIM,
            Align::Left,
        );
        l.ui.text_in(
            value,
            Rect::new(body.x + 200.0, y, body.w - 200.0, 22.0),
            13.0,
            Weight::Medium,
            TEXT,
            Align::Left,
        );
    }
    if l.ui.button(
        "drive-close",
        Rect::new(buttons.right() - 110.0, buttons.y, 110.0, 34.0),
        "Close",
        None,
        ButtonKind::Normal,
    ) {
        l.pages.drive_details = false;
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

// --- controls ---------------------------------------------------------------------------------

/// The game's own actions a controller's button can be given, besides the bus's: the doors
/// and gears of any bus, looking round while held, the bus radio while held, the cameras
/// and the views - both of OMSI's view resets, the one view's (C) and every view's (Space),
/// which a controller could not bring back to the first camera (#1167).
const PAD_GAME_ACTIONS: [&str; 26] = [
    "doors_all",
    "door_4",
    "door_3",
    "door_2",
    "door_1",
    "gear_up",
    "gear_down",
    "view_look_left",
    "view_look_right",
    "view_look_up",
    "view_look_down",
    "view_reset_direction",
    "view_reset_all_directions",
    "view_interiorcam_plus",
    "view_interiorcam_minus",
    "view_toggle_viewpoint",
    "view_toggle_interior",
    "view_set_driver",
    "view_set_passenger",
    "view_set_outside",
    "sim_pause",
    "screenshot",
    "quicksave",
    "toggel_mouse_ctrl",
    "toggel_ctrler",
    "voice_radio",
];

pub(super) fn action_text(names: &crate::describe::ControlNames, a: &str) -> String {
    known_action(a).unwrap_or_else(|| names.control(a))
}

pub(super) fn control_names(l: &Launcher) -> &'static crate::describe::ControlNames {
    crate::describe::names(
        std::path::Path::new(&l.state.config.root),
        l.state
            .settings
            .get("language")
            .and_then(|x| x.as_str())
            .unwrap_or("ENG"),
    )
}

fn known_action(a: &str) -> Option<String> {
    if let Some(gear) = a
        .strip_prefix("kw_s_")
        .and_then(|s| s.strip_suffix("_fest"))
    {
        return Some(omsi_ui::tr("Gear %{gear} (H-pattern)").replace("%{gear}", &gear.to_string()));
    }
    let known: &[(&str, &str)] = &[
        ("throttle", "Throttle"),
        ("brake", "Brake"),
        ("throttle_amplify", "Throttle (full, kickdown)"),
        ("clutch", "Clutch"),
        ("steering_left", "Steer left"),
        ("steering_right", "Steer right"),
        ("steering_neutral", "Steering to centre"),
        ("parking_brake_toggle", "Parking brake"),
        ("blinker_left_set", "Indicator left"),
        ("blinker_right_set", "Indicator right"),
        ("blinker_left_toggle", "Indicator left (toggle)"),
        ("blinker_right_toggle", "Indicator right (toggle)"),
        ("blinker_off", "Indicators off"),
        ("blinker_warn_toggle", "Hazard lights"),
        ("gear_up", "Gear up (manual gearbox)"),
        ("gear_down", "Gear down (manual gearbox)"),
        ("horn", "Horn"),
        ("kw_scheinwerfer_toggle", "Headlights"),
        ("kw_standlicht_toggle", "Sidelights"),
        ("kw_fernlicht_toggle", "High beam"),
        ("kw_m_enginestart", "Starter"),
        ("kw_wipermode_up", "Wipers (next mode)"),
        ("cp_batterietrennschalter_toggle", "Battery / ignition"),
        ("automatic_D", "Gear D"),
        ("automatic_N", "Gear N"),
        ("automatic_R", "Gear R"),
        ("bus_doorfront0", "Front door (leaf 1)"),
        ("bus_doorfront1", "Front door (leaf 2)"),
        ("bus_dooraft", "Release rear doors"),
        ("door_1", "Door 1 (front), any bus"),
        ("door_2", "Door 2, any bus"),
        ("door_3", "Door 3, any bus"),
        ("door_4", "Door 4, any bus"),
        ("doors_all", "All doors, any bus"),
        ("ticket_give", "Sell the requested ticket"),
        ("view_set_driver", "Driver's view"),
        ("view_set_passenger", "Passenger view"),
        ("view_set_outside", "Outside view"),
        ("view_toggle_viewpoint", "Next view"),
        ("view_toggle_interior", "Cabin and outside, one key"),
        ("vr_recenter", "VR: Reset view"),
        ("vr_toggle_desktop_mirror", "VR: Monitor preview"),
        ("vr_toggle_mode", "VR: Switch VR / desktop"),
        ("vr_toggle_navigator", "VR: Toggle navigator"),
        ("vr_position_navigator", "VR: Position navigator"),
        ("exit", "Quit"),
        ("chat_open", "Multiplayer: write in the chat"),
        ("chat_toggle", "Multiplayer: show / hide the chat"),
        ("voice_radio", "Multiplayer: bus radio (hold)"),
        ("sim_pause", "Pause"),
        ("screenshot", "Screenshot"),
        ("quicksave", "Quicksave"),
        ("toggel_mouse_ctrl", "Toggle mouse steering"),
        ("toggel_ctrler", "Toggle game controllers"),
    ];
    known.iter().find(|k| k.0 == a).map(|k| k.1.to_string())
}

pub fn controls(l: &mut Launcher, area: Rect) {
    // (where on the page is the tab shown, and its line says how it is used)
    let (_, title, keys_line) = UiPage::Controls.heading().unwrap_or_default();
    let (place, line) = if l.pages.controls_tab == 0 {
        ("Keyboard", keys_line)
    } else {
        ("Game controllers", "What each axis and button of a wheel, pedals or joystick does: OMSI 2's gamectrler.cfg, in the content folder.")
    };
    if !super::mobile::mobile() {
        // (a computer, as the Windows 11 concept: the tabs under the heading, the keyboard in
        // groups with the chosen action beside it)
        let body = l.page_title_with(area, UiPage::Controls.title(), (place, title, line));
        let mut tab = l.pages.controls_tab;
        if l.ui.segmented(
            "controls-tab",
            Rect::new(body.x, body.y, 330.0, 36.0),
            &mut tab,
            &["Keyboard", "Game controllers"],
        ) {
            l.pages.controls_tab = tab;
        }
        let body = Rect::new(body.x, body.y + 50.0, body.w, (body.h - 50.0).max(0.0));
        if l.pages.controls_tab == 1 {
            game_controllers(l, body);
        } else {
            l.pages.pads.cancel_feedback_test();
            super::keyboard::draw(l, body);
        }
        return;
    }
    let body = l.page_title_beside(area, UiPage::Controls.title(), (place, title, line), 320.0);
    let mut tab = l.pages.controls_tab;
    if l.ui.segmented(
        "controls-tab",
        l.title_tabs(body, 320.0),
        &mut tab,
        &["Keyboard", "Game controllers"],
    ) {
        l.pages.controls_tab = tab;
    }
    if l.pages.controls_tab == 1 {
        game_controllers(l, body);
        return;
    }
    l.pages.pads.cancel_feedback_test();
    // a key pressed while one binding waits for it
    if let (Some((sec, idx)), Some(code)) = (l.pages.capturing, l.ui.input.raw_key) {
        use winit::keyboard::KeyCode as K;
        if code == K::Escape {
            l.pages.capturing = None;
        } else if !matches!(
            code,
            K::ShiftLeft
                | K::ShiftRight
                | K::ControlLeft
                | K::ControlRight
                | K::AltLeft
                | K::AltRight
                | K::SuperLeft
                | K::SuperRight
        ) {
            match crate::keys::dik_code(code) {
                Some(scan) => {
                    let m = omsi_content::input::chord(
                        l.ui.input.shift,
                        l.ui.input.ctrl,
                        l.ui.input.alt,
                    ) as i64;
                    let section = ["vehicles", "game"][sec];
                    let vr_binding = l
                        .state
                        .keybindings
                        .get(section)
                        .and_then(|a| a.as_array())
                        .and_then(|a| a.get(idx))
                        .and_then(|b| b.get("action"))
                        .and_then(|a| a.as_str())
                        .is_some_and(|a| a.starts_with("vr_"));
                    if let Some(b) = l
                        .state
                        .keybindings
                        .get_mut(section)
                        .and_then(|a| a.as_array_mut())
                        .and_then(|a| a.get_mut(idx))
                    {
                        // (the entry's "held" bit is the action's, not the key's: it stays)
                        let hold = b.get("modifier").and_then(|x| x.as_i64()).unwrap_or(0)
                            & omsi_content::input::KEY_HOLD as i64;
                        b["scan_code"] = json!(scan);
                        b["modifier"] = json!(m | hold);
                    }
                    save_keys(l, vr_binding);
                }
                None => l.state.set_status(
                    format!("{code:?} has no DirectInput scan code the game understands."),
                    true,
                ),
            }
            l.pages.capturing = None;
        }
        l.ui.input.raw_key = None;
    }
    // The keys below are the ones the game uses only with "Custom controls" (Settings →
    // Driving keys); the ready-made layouts keep W A S D / the arrows for driving. Say so,
    // with the switch right here - and changing a key switches by itself (see `save_keys`).
    let preset = l
        .state
        .settings
        .get("drive_keys")
        .and_then(|v| v.as_str())
        .unwrap_or("simple")
        .to_string();
    let body = if preset != "omsi" {
        let name = match preset.as_str() {
            "wasd" => "W A S D only",
            "arrows" => "Arrow keys only",
            _ => "W A S D + arrows",
        };
        let bw = if body.w < 700.0 { 150.0 } else { 200.0 };
        let text = omsi_ui::tr("Driving keys: %{name} (Settings). Those keys drive the bus and win over the list below. Change any key here and your own layout (Custom controls) is used from then on.").replace("%{name}", &omsi_ui::tr(name));
        let tw = body.w - bw - 70.0;
        let th = l.ui.paragraph_height(&text, tw, 12.5, Weight::Medium);
        let bar = Rect::new(body.x, body.y, body.w, (th + 22.0).max(54.0));
        l.ui.p().rounded(bar, 8.0, accent().alpha(0.1));
        l.ui.p().rounded_border(bar, 8.0, 1.0, accent().alpha(0.45));
        l.ui.icon(
            "info",
            Vec2::new(bar.x + 22.0, bar.center().y),
            20.0,
            accent(),
        );
        l.ui.paragraph(
            &text,
            Vec2::new(bar.x + 42.0, bar.center().y - th * 0.5),
            tw,
            12.5,
            Weight::Medium,
            TEXT_SOFT,
        );
        if l.ui.button(
            "kb-use-custom",
            Rect::new(bar.right() - bw - 10.0, bar.center().y - 18.0, bw, 36.0),
            "Use these keys",
            Some("keyboard"),
            ButtonKind::Primary,
        ) {
            use_custom_keys(l);
        }
        let used = bar.h + 12.0;
        Rect::new(body.x, body.y + used, body.w, body.h - used)
    } else {
        body
    };
    let half = (body.w - GAP * 2.0) * 0.5;
    let names = control_names(l);
    for (sec, (title, sub, key)) in [
        ("Driving & the bus", "The bus's own keys", "vehicles"),
        ("The game", "Menus, views, pausing", "game"),
    ]
    .iter()
    .enumerate()
    {
        let r = Rect::new(
            body.x + sec as f32 * (half + GAP * 2.0),
            body.y,
            half,
            body.h,
        );
        l.ui.panel(r);
        let inner = Launcher::card_title(
            &mut l.ui,
            Rect::new(r.x + 18.0, r.y + 14.0, r.w - 36.0, r.h - 28.0),
            title,
        );
        l.ui.text_in(
            sub,
            Rect::new(inner.x, inner.y - 6.0, inner.w, 18.0),
            12.0,
            Weight::Regular,
            TEXT_DIM,
            Align::Left,
        );
        let mut filter = std::mem::take(&mut l.pages.kb_filter[sec]);
        let event_w = if sec == 0 { 138.0 } else { 0.0 };
        let filter_w = if sec == 0 {
            (inner.w - event_w - GAP).max(120.0)
        } else {
            inner.w
        };
        l.ui.text_input(
            &format!("kb-filter-{sec}"),
            Rect::new(inner.x, inner.y + 18.0, filter_w, 34.0),
            &mut filter,
            if l.pages.kb_events[sec] {
                "Filter events…"
            } else {
                "Filter…"
            },
            Some("search"),
        );
        if sec == 0
            && l.ui.button(
                "kb-events",
                Rect::new(inner.x + filter_w + GAP, inner.y + 18.0, event_w, 34.0),
                if l.pages.kb_events[sec] {
                    "Back to keys"
                } else {
                    "Add event…"
                },
                Some(if l.pages.kb_events[sec] {
                    "arrow_back"
                } else {
                    "add"
                }),
                ButtonKind::Normal,
            )
        {
            l.pages.kb_events[sec] = !l.pages.kb_events[sec];
            filter.clear();
        }
        l.pages.kb_filter[sec] = filter.clone();
        let q = filter.to_lowercase();

        // OMSI's Add event dialog: all KY_ events from the language files, including
        // event tables supplied by installed mods. Picking one adds an unbound vehicle
        // entry and immediately waits for its key.
        if sec == 0 && l.pages.kb_events[sec] {
            let mut events = names.events();
            if !q.is_empty() {
                events.retain(|(action, label)| {
                    action.to_lowercase().contains(&q) || matches(label, &q)
                });
            }
            let mut picked: Option<String> = None;
            l.ui.scroll_area(
                &format!("kb-events-{sec}"),
                Rect::new(
                    inner.x - 6.0,
                    inner.y + 62.0,
                    inner.w + 12.0,
                    inner.bottom() - (inner.y + 62.0),
                ),
                &mut |ui, v| {
                    let rh = 40.0;
                    for (row, (action, label)) in events.iter().enumerate() {
                        let rr = Rect::new(v.x + 6.0, v.y + row as f32 * rh, v.w - 16.0, rh - 4.0);
                        let shown = format!("{label}  ·  KY_{action}");
                        if ui.button(
                            &format!("kb-event-{row}"),
                            rr,
                            &shown,
                            Some("add"),
                            ButtonKind::Ghost,
                        ) {
                            picked = Some(action.clone());
                        }
                    }
                    events.len() as f32 * rh
                },
            );
            if let Some(action) = picked {
                if let Some(a) = l
                    .state
                    .keybindings
                    .get_mut("vehicles")
                    .and_then(|a| a.as_array_mut())
                {
                    a.push(json!({ "action": action.clone(), "scan_code": 0, "modifier": 0 }));
                    l.pages.capturing = Some((0, a.len() - 1));
                    l.pages.kb_events[0] = false;
                    l.pages.kb_filter[0] = action;
                    l.state.set_status(
                        "Event added. Press the key you want to use (Escape cancels).",
                        false,
                    );
                }
            }
            continue;
        }

        let list: Vec<(usize, String, i64, i64)> = l
            .state
            .keybindings
            .get(*key)
            .and_then(|a| a.as_array())
            .map(|a| {
                a.iter()
                    .enumerate()
                    .map(|(i, b)| {
                        (
                            i,
                            b.get("action")
                                .and_then(|x| x.as_str())
                                .unwrap_or("")
                                .to_string(),
                            b.get("scan_code").and_then(|x| x.as_i64()).unwrap_or(0),
                            b.get("modifier").and_then(|x| x.as_i64()).unwrap_or(0),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut shown: Vec<(usize, String, String, bool)> = list
            .iter()
            .filter(|(_, a, s, m)| {
                q.is_empty()
                    || matches(&action_text(names, a), &q)
                    || a.to_lowercase().contains(&q)
                    || crate::keys::key_name(*s, *m).to_lowercase().contains(&q)
            })
            .map(|(i, a, s, m)| {
                let clash = *s != 0
                    && list
                        .iter()
                        .any(|(j, _, s2, m2)| j != i && s2 == s && m2 == m);
                (
                    *i,
                    action_text(names, a),
                    crate::keys::key_name(*s, *m),
                    clash,
                )
            })
            .collect();
        if sec == 1 {
            shown.sort_by_key(|(_, label, _, _)| !label.starts_with("VR:"));
        }
        let capturing = l.pages.capturing;
        // what the row's buttons asked: (entry, cleared) a key cleared or to be pressed,
        // `more` another key for an entry's action (#854)
        let mut clicked: Option<(usize, bool)> = None;
        let mut more: Option<usize> = None;
        let time = l.ui.time;
        // a name the list does not have (a bus's own trigger a mod's readme gives a key, the
        // Urbanway's `ASS_toggle`): added to the list as a key of its own, as an [entry]
        // added to OMSI's keyboard.cfg by hand is (#854)
        let new_action = filter.trim();
        let mut list_top = inner.y + 62.0;
        if shown.is_empty()
            && new_action.len() > 1
            && new_action
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            if l.ui.button(
                &format!("kb-add-{sec}"),
                Rect::new(inner.x, list_top, inner.w, 36.0),
                &omsi_ui::tr("Add \"%{action}\" and give it a key")
                    .replace("%{action}", new_action),
                Some("add"),
                ButtonKind::Normal,
            ) {
                if let Some(a) = l
                    .state
                    .keybindings
                    .get_mut(*key)
                    .and_then(|a| a.as_array_mut())
                {
                    a.push(json!({ "action": new_action, "scan_code": 0, "modifier": 0 }));
                    l.pages.capturing = Some((sec, a.len() - 1));
                }
                // (the filter stays: the new row is the one it shows, waiting for its key)
            }
            list_top += 44.0;
        }
        l.ui.scroll_area(
            &format!("kb-{sec}"),
            Rect::new(
                inner.x - 6.0,
                list_top,
                inner.w + 12.0,
                inner.bottom() - list_top,
            ),
            &mut |ui, v| {
                let rh = 40.0;
                for (row, (i, label, keyn, clash)) in shown.iter().enumerate() {
                    let rr = Rect::new(v.x + 6.0, v.y + row as f32 * rh, v.w - 16.0, rh - 4.0);
                    if rr.bottom() < v.y - 50.0 {
                        continue;
                    }
                    ui.p().rounded(rr, 8.0, Color::WHITE.alpha(0.03));
                    ui.text_in(
                        label,
                        Rect::new(rr.x + 12.0, rr.y, rr.w - 240.0, rr.h),
                        13.0,
                        Weight::Medium,
                        TEXT_SOFT,
                        Align::Left,
                    );
                    // another key for the same action (OMSI's file may give one action
                    // several [entry]s; several actions on one key need nothing more than the
                    // same key pressed for each)
                    let pr = Rect::new(rr.right() - 222.0, rr.y + 5.0, 26.0, rr.h - 10.0);
                    let (hp, _, cp) = ui.interact(id_of(&format!("kb-{sec}-{i}-more")), pr);
                    ui.icon(
                        "add",
                        pr.center(),
                        16.0,
                        if hp { accent() } else { TEXT_FAINT },
                    );
                    if cp {
                        more = Some(*i);
                    }
                    let kr = Rect::new(rr.right() - 190.0, rr.y + 5.0, 150.0, rr.h - 10.0);
                    let waiting = capturing == Some((sec, *i));
                    let id = id_of(&format!("kb-{sec}-{i}"));
                    let (h, _, c) = ui.interact(id, kr);
                    if c {
                        clicked = Some((*i, false));
                    }
                    let base = if waiting {
                        accent().alpha(0.25 + 0.15 * (time * 6.0).sin().abs())
                    } else if *clash {
                        DANGER.alpha(0.22)
                    } else {
                        Color::WHITE.alpha(if h { 0.12 } else { 0.07 })
                    };
                    ui.p().rounded(kr, 6.0, base);
                    ui.p().rounded_border(
                        kr,
                        6.0,
                        1.0,
                        if waiting {
                            accent()
                        } else if *clash {
                            DANGER
                        } else {
                            Color::WHITE.alpha(0.1)
                        },
                    );
                    ui.text_in(
                        if waiting { "press a key…" } else { keyn },
                        kr,
                        12.0,
                        Weight::Bold,
                        if *clash { DANGER.lighten(0.3) } else { TEXT },
                        Align::Center,
                    );
                    let xr = Rect::new(rr.right() - 32.0, rr.y + 5.0, 26.0, rr.h - 10.0);
                    let (hx, _, cx) = ui.interact(id ^ 1, xr);
                    ui.icon(
                        "close",
                        xr.center(),
                        16.0,
                        if hx { DANGER } else { TEXT_FAINT },
                    );
                    if cx {
                        clicked = Some((*i, true));
                    }
                }
                shown.len() as f32 * rh
            },
        );
        match clicked {
            Some((i, true)) => {
                let vr_binding = l
                    .state
                    .keybindings
                    .get(*key)
                    .and_then(|a| a.as_array())
                    .and_then(|a| a.get(i))
                    .and_then(|b| b.get("action"))
                    .and_then(|a| a.as_str())
                    .is_some_and(|a| a.starts_with("vr_"));
                if let Some(b) = l
                    .state
                    .keybindings
                    .get_mut(*key)
                    .and_then(|a| a.as_array_mut())
                    .and_then(|a| a.get_mut(i))
                {
                    b["scan_code"] = json!(0);
                    b["modifier"] = json!(0);
                }
                save_keys(l, vr_binding);
            }
            Some((i, false)) => l.pages.capturing = Some((sec, i)),
            None => {}
        }
        if let Some(i) = more {
            if let Some(a) = l
                .state
                .keybindings
                .get_mut(*key)
                .and_then(|a| a.as_array_mut())
            {
                if let Some(b) = a.get(i).cloned() {
                    // (the held bit is the action's: it goes with it)
                    let hold = b.get("modifier").and_then(|x| x.as_i64()).unwrap_or(0)
                        & omsi_content::input::KEY_HOLD as i64;
                    a.insert(i + 1, json!({ "action": b.get("action").cloned().unwrap_or(json!("")), "scan_code": 0, "modifier": hold }));
                    l.pages.capturing = Some((sec, i + 1));
                }
            }
        }
    }
    if !l.state.keybindings_error.is_empty() {
        let e = l.state.keybindings_error.clone();
        l.ui.text_in(
            &e,
            Rect::new(body.x, body.bottom() + 4.0, body.w, 18.0),
            12.0,
            Weight::Medium,
            DANGER,
            Align::Left,
        );
    }
}

/// Hide empty slots beyond the physical buttons without changing the saved controller file.
fn shown_button_count(
    buttons: &[(String, String)],
    physical: usize,
    revealed: Option<usize>,
) -> usize {
    physical
        .max(
            buttons
                .iter()
                .rposition(|(action, _)| !action.trim().is_empty())
                .map(|i| i + 1)
                .unwrap_or(0),
        )
        .max(revealed.map(|i| i + 1).unwrap_or(0))
}

/// The game controllers tab (see `PadsView`).
fn game_controllers(l: &mut Launcher, body: Rect) {
    use crate::controllers::{DeviceCfg, Func};
    let hwnd = l
        .window
        .as_deref()
        .and_then(crate::controllers::window_handle);
    let names = control_names(l);
    let pv = &mut l.pages.pads;
    if pv.io.is_none() {
        pv.io = Some(crate::controllers::Devices::new(hwnd, false));
    }
    if pv.devices.is_none() {
        let root = std::path::PathBuf::from(&l.state.config.root);
        pv.devices = Some(crate::controllers::read_cfg(&root));
    }
    // what the devices do now (and a button pressed while one is awaited)
    let mut pressed: Vec<(String, usize)> = Vec::new();
    let mut connected: Vec<crate::controllers::Connected> = Vec::new();
    if let Some(io) = pv.io.as_mut() {
        for (name, n, down) in io.poll() {
            if down {
                pressed.push((name, n));
            }
        }
        connected = io.connected();
    }
    let devices = pv.devices.get_or_insert_with(Vec::new);
    let mut body = body;
    if !l
        .state
        .settings
        .get("momentary_gears")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
        && crate::hpattern::has_held_bindings(devices)
    {
        let height = l.ui.paragraph("H-pattern gears are assigned. Enable return to neutral under Settings → Driving → Game controllers if your shifter has no neutral button.", Vec2::new(body.x + 12.0, body.y + 8.0), body.w - 24.0, 12.5, Weight::Regular, TEXT_DIM);
        body.y += height + 20.0;
        body.h -= height + 20.0;
    }
    // devices connected but not set up yet can be added
    let list_w = (body.w * 0.32).min(360.0);
    let left = Rect::new(body.x, body.y, list_w, body.h);
    let right = Rect::new(
        body.x + list_w + GAP * 2.0,
        body.y,
        body.w - list_w - GAP * 2.0,
        body.h,
    );
    l.ui.panel(left);
    let inner = Launcher::card_title(
        &mut l.ui,
        Rect::new(left.x + 18.0, left.y + 14.0, left.w - 36.0, left.h - 28.0),
        "Devices",
    );
    let mut add: Option<String> = None;
    let mut sel = pv.selected;
    let list_r = Rect::new(inner.x - 6.0, inner.y, inner.w + 12.0, inner.h - 108.0);
    let offs: Vec<String> = l
        .state
        .settings
        .get("ctrl_off")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .split('|')
        .map(str::to_string)
        .filter(|s| !s.is_empty())
        .collect();
    {
        let ui = &mut l.ui;
        let devices = &*devices;
        let connected = &connected;
        ui.scroll_area("pad-list", list_r, &mut |ui, v| {
            let mut y = v.y;
            for (i, d) in devices.iter().enumerate() {
                let on = connected.iter().any(|c| crate::controllers::names_match(&d.name, &c.name));
                let switched_off = offs.iter().any(|o| o.eq_ignore_ascii_case(&d.name));
                let r = Rect::new(v.x + 6.0, y, v.w - 12.0, 44.0);
                if ui.row(&format!("pad-{i}"), r, sel == i) {
                    sel = i;
                }
                ui.text_in(&d.name, Rect::new(r.x + 12.0, r.y, r.w - 40.0, r.h), 13.0, Weight::Medium, if on { TEXT } else { TEXT_DIM }, Align::Left);
                if switched_off {
                    ui.text_in("off", Rect::new(r.right() - 40.0, r.y, 30.0, r.h), 11.5, Weight::Bold, TEXT_FAINT, Align::Right);
                } else {
                    ui.icon(if on { "check_circle" } else { "remove" }, Vec2::new(r.right() - 18.0, r.center().y), 16.0, if on { OK } else { TEXT_FAINT });
                }
                y += 48.0;
            }
            for c in connected.iter().filter(|c| !devices.iter().any(|d| crate::controllers::names_match(&d.name, &c.name))) {
                let r = Rect::new(v.x + 6.0, y, v.w - 12.0, 38.0);
                if ui.button(&format!("pad-add-{}", c.name), r, &omsi_ui::tr("Set up %{name}").replace("%{name}", &c.name), Some("add"), ButtonKind::Primary) {
                    add = Some(c.name.clone());
                }
                y += 44.0;
            }
            if devices.is_empty() && connected.is_empty() {
                y += 4.0 + ui.paragraph("No game controller is connected, and none is set up. Connect a wheel, pedals or a joystick; a gamepad works without setting up (left stick steers, the triggers are the pedals).", Vec2::new(v.x + 6.0, y + 4.0), v.w - 12.0, 13.0, Weight::Regular, TEXT_DIM);
            }
            y - v.y
        });
    }
    if sel != pv.selected {
        release_feedback(&mut pv.io, &mut pv.feedback_test);
        pv.selected = sel;
        pv.capturing = false;
        pv.revealed_button = None;
        pv.wizard = None;
        pv.confirm_remove = None;
    }
    if let Some(name) = add {
        // (a device of buttons only - a gear shifter, a button box - has no axes for the
        // assistant: its buttons are given their keys on its page)
        let axes = !pv.io.as_ref().is_some_and(|io| io.buttons_only(&name));
        devices.push(DeviceCfg {
            name,
            second: "0".into(),
            ..Default::default()
        });
        pv.selected = devices.len() - 1;
        pv.revealed_button = None;
        pv.dirty = true;
        // a new device starts with the assistant
        if axes {
            pv.wizard = Some(Wizard {
                step: 0,
                rest: [None; 8],
                at: Vec::new(),
                error: None,
                calibration: None,
                ff_choice: None,
                test_strength: crate::ffb_calibration::PULSE_FORCE,
            });
        }
    }
    // the dead zone (a setting of the game's)
    let dz_r = Rect::new(inner.x, inner.bottom() - 98.0, inner.w, 34.0);
    let mut dz = l
        .state
        .settings
        .get("ctrl_deadzone")
        .and_then(|x| x.as_f64())
        .unwrap_or(0.0) as f32;
    if l.ui
        .slider("pad-dz", dz_r, &mut dz, 0.0, 0.3, 0.01, "Dead zone", &|v| {
            format!("{:.0} %", v * 100.0)
        })
    {
        l.state.settings["ctrl_deadzone"] = json!(dz);
        l.state.settings_dirty = 0.3;
    }
    let save_r = Rect::new(inner.x, inner.bottom() - 48.0, inner.w, 40.0);
    if l.ui.button(
        "pad-save",
        save_r,
        if pv.dirty { "Save" } else { "Saved" },
        Some("save"),
        if pv.dirty {
            ButtonKind::Primary
        } else {
            ButtonKind::Normal
        },
    ) && pv.dirty
    {
        match save_gamectrler(devices) {
            Ok(p) => {
                pv.dirty = false;
                l.state.set_status(
                    omsi_ui::tr("Game controllers saved to %{path}")
                        .replace("%{path}", &p.display().to_string()),
                    false,
                );
            }
            Err(e) => l.state.set_status(
                omsi_ui::tr("Not saved: %{error}").replace("%{error}", &e.to_string()),
                true,
            ),
        }
    }
    // the device shown
    l.ui.panel(right);
    let Some(d) = devices.get_mut(pv.selected) else {
        return;
    };
    let inner = Launcher::card_title(
        &mut l.ui,
        Rect::new(
            right.x + 18.0,
            right.y + 14.0,
            right.w - 36.0,
            right.h - 28.0,
        ),
        &d.name.clone(),
    );
    let live_dev = crate::controllers::find_connected(&connected, &d.name);
    let live: Vec<(usize, f32)> = live_dev.map(|c| c.axes.clone()).unwrap_or_default();
    // every button the device has gets its line (DirectInput says how many)
    if let Some(n) = live_dev.map(|c| c.buttons).filter(|n| *n > d.buttons.len()) {
        d.buttons.resize(n, (String::new(), "0".into()));
    }
    // the assistant, over the device's page
    if let Some(w) = pv.wizard.as_mut() {
        let done = if w.step == WIZARD_STEPS.len() {
            feedback_setup(
                &mut l.ui,
                inner,
                w,
                d,
                &live,
                live_dev,
                &mut pv.io,
                &mut pv.feedback_test,
                hwnd,
                l.state
                    .settings
                    .get("ff_invert")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            )
        } else {
            wizard(
                &mut l.ui,
                inner,
                w,
                d,
                &live,
                live_dev.is_some(),
                live_dev.is_some_and(|c| c.ff_capable && !c.gamepad),
            )
        };
        match done {
            Some(true) => {
                release_feedback(&mut pv.io, &mut pv.feedback_test);
                pv.wizard = None;
                pv.dirty = true;
                l.state.set_status(
                    "Set up: press Save to keep it (the buttons can be given their keys below).",
                    false,
                );
            }
            Some(false) => {
                release_feedback(&mut pv.io, &mut pv.feedback_test);
                pv.wizard = None;
            }
            None => {}
        }
        return;
    }
    // this device on or off (a second listing of the same wheel, a device not to be used)
    {
        let offs: Vec<String> = l
            .state
            .settings
            .get("ctrl_off")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .split('|')
            .map(str::to_string)
            .filter(|s| !s.is_empty())
            .collect();
        let mut on = !offs.iter().any(|o| o.eq_ignore_ascii_case(&d.name));
        if l.ui.toggle(
            "pad-on",
            Rect::new(inner.right() - 400.0, inner.y - 36.0, 170.0, 30.0),
            &mut on,
            "Use this device",
        ) {
            let mut offs: Vec<String> = offs
                .into_iter()
                .filter(|o| !o.eq_ignore_ascii_case(&d.name))
                .collect();
            if !on {
                offs.push(d.name.clone());
            }
            l.state.settings["ctrl_off"] = json!(offs.join("|"));
            l.state.settings_dirty = 0.3;
        }
    }
    let buttons_only = pv.io.as_ref().is_some_and(|io| io.buttons_only(&d.name));
    if !buttons_only
        && l.ui.button(
            "pad-wizard",
            Rect::new(inner.right() - 220.0, inner.y - 36.0, 220.0, 30.0),
            "Set up step by step",
            Some("touch_app"),
            ButtonKind::Normal,
        )
    {
        pv.wizard = Some(Wizard {
            step: 0,
            rest: [None; 8],
            at: Vec::new(),
            error: None,
            calibration: None,
            ff_choice: None,
            test_strength: crate::ffb_calibration::PULSE_FORCE,
        });
    }
    const AXES: [&str; 8] = [
        "X axis",
        "Y axis",
        "Z axis",
        "X rotation",
        "Y rotation",
        "Z rotation",
        "Slider 1",
        "Slider 2",
    ];
    let funcs: Vec<String> = Func::LABELS.iter().map(|s| s.to_string()).collect();
    let mut actions: Vec<String> = vec!["<none>".into()];
    actions.extend(
        l.state
            .keybindings
            .get("vehicles")
            .and_then(|a| a.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|b| b.get("action").and_then(|x| x.as_str()).map(String::from))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
    );
    // H-pattern shifters use OMSI's "_fest" actions: pressing the gate selects the gear,
    // releasing it fires "_fest_off", which lets the bus script return to neutral.
    for a in [
        "kw_s_R_fest",
        "kw_s_1_fest",
        "kw_s_2_fest",
        "kw_s_3_fest",
        "kw_s_4_fest",
        "kw_s_5_fest",
        "kw_s_6_fest",
        "kw_s_7_fest",
        "kw_s_8_fest",
        "kw_s_9_fest",
        "kw_s_10_fest",
    ] {
        if !actions.iter().any(|x| x.eq_ignore_ascii_case(a)) {
            actions.push(a.to_string());
        }
    }
    // the game's own view actions (looking around while held, the cameras, the views)
    for a in PAD_GAME_ACTIONS {
        if !actions.iter().any(|x| x == a) {
            actions.insert(1, a.to_string());
        }
    }
    actions.dedup();
    let labels: Vec<String> = actions
        .iter()
        .enumerate()
        .map(|(i, a)| {
            if i == 0 {
                a.clone()
            } else {
                action_text(names, a)
            }
        })
        .collect();
    let mut dirty = false;
    let lit = pv
        .last_pressed
        .filter(|(_, t)| t.elapsed().as_secs_f32() < 4.0)
        .map(|(b, _)| b);
    // Some OMSI configs contain hundreds of empty trailing slots (the G920 report had
    // 131 entries for 18 physical buttons). Keep them on disk, but do not fill the UI with them.
    let shown_buttons = shown_button_count(
        &d.buttons,
        live_dev.map(|c| c.buttons).unwrap_or(0),
        pv.revealed_button,
    );
    // (the axes, then every button of the device: the list scrolls - it stopped at the ten
    // buttons that fitted)
    let list = Rect::new(inner.x - 6.0, inner.y, inner.w + 12.0, inner.h - 50.0);
    let mut buttons_start_y = 0.0;
    l.ui.scroll_area("pad-detail", list, &mut |ui, v| {
        let x0 = v.x + 6.0;
        let w = v.w - 16.0;
        let mut y = v.y;
        if live_dev.is_some_and(|c| c.ff_capable) || d.ff_invert.is_some() {
            let (mut steering_force, mut vibration) = d.ff_scale.unwrap_or((1.0, 1.0));
            if ui.slider(
                "pad-ff-steering",
                Rect::new(x0, y, w, ROW),
                &mut steering_force,
                0.0,
                2.0,
                0.05,
                "Steering force",
                &|v| format!("{:.0}%", v * 100.0),
            ) {
                d.ff_scale = Some((steering_force, vibration));
                dirty = true;
            }
            y += ROW + 6.0;
            if ui.slider(
                "pad-ff-vibration",
                Rect::new(x0, y, w, ROW),
                &mut vibration,
                0.0,
                2.0,
                0.05,
                "Vibration",
                &|v| format!("{:.0}%", v * 100.0),
            ) {
                d.ff_scale = Some((steering_force, vibration));
                dirty = true;
            }
            y += ROW + 20.0;
            if !live_dev.is_some_and(|c| c.gamepad) {
                let mut invert = d.ff_invert.unwrap_or_else(|| {
                    l.state
                        .settings
                        .get("ff_invert")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                });
                if ui.toggle(
                    "pad-ff-invert",
                    Rect::new(x0, y, w, ROW),
                    &mut invert,
                    "Invert force feedback",
                ) {
                    d.ff_invert = Some(invert);
                    dirty = true;
                }
                y += ROW + 20.0;
            }
        }
        let lab_w = if w < 520.0 { 84.0 } else { 110.0 };
        let inv_w = 110.0;
        let shp_w = 140.0;
        let sel_w = (w - lab_w - inv_w - shp_w - 60.0 - 4.0 * GAP).clamp(120.0, 200.0);
        let bar_w = (w - lab_w - sel_w - inv_w - shp_w - 4.0 * GAP).max(30.0);
        let shapes: Vec<String> = crate::controllers::AXIS_SHAPES
            .iter()
            .map(|s| s.0.to_string())
            .collect();
        // (a device of buttons only has no axes to give a function)
        for a in (0..8).filter(|_| !buttons_only) {
            let r = Rect::new(x0, y, w, ROW);
            ui.label(Rect::new(r.x, r.y, lab_w, r.h), AXES[a]);
            let bar = Rect::new(r.x + lab_w + GAP, r.y + 12.0, bar_w, r.h - 24.0);
            ui.p().rounded(bar, 4.0, Color::WHITE.alpha(0.06));
            if let Some((_, v)) = live.iter().find(|(k, _)| *k == a) {
                let x = bar.x + (v.clamp(-1.0, 1.0) + 1.0) * 0.5 * bar.w;
                ui.p().rounded(
                    Rect::new(x - 2.0, bar.y - 3.0, 4.0, bar.h + 6.0),
                    2.0,
                    accent(),
                );
            }
            let mut sel = (Func::code(d.axes[a].map(|x| x.0)) + 1) as usize;
            if ui.select(
                &format!("pad-axis-{a}"),
                Rect::new(bar.right() + GAP, r.y, sel_w, r.h),
                &mut sel,
                &funcs,
            ) {
                let inv = d.axes[a].map(|x| x.1).unwrap_or(false);
                d.axes[a] = Func::from_code(sel as i32 - 1).map(|f| (f, inv));
                dirty = true;
            }
            let mut inv = d.axes[a].map(|x| x.1).unwrap_or(false);
            if d.axes[a].is_some()
                && ui.toggle(
                    &format!("pad-inv-{a}"),
                    Rect::new(bar.right() + GAP + sel_w + GAP, r.y, inv_w, r.h),
                    &mut inv,
                    "Reversed",
                )
            {
                if let Some(x) = d.axes[a].as_mut() {
                    x.1 = inv;
                }
                dirty = true;
            }
            if d.axes[a].is_some() {
                // the characteristic: the curve bits of the flags, the range extension kept
                let curve = d.axis_flags[a] & (4 | 8 | 0x10);
                let mut shp = crate::controllers::AXIS_SHAPES
                    .iter()
                    .position(|s| s.1 == curve)
                    .unwrap_or(0);
                if ui.select(
                    &format!("pad-shape-{a}"),
                    Rect::new(
                        bar.right() + GAP + sel_w + GAP + inv_w + GAP,
                        r.y,
                        shp_w,
                        r.h,
                    ),
                    &mut shp,
                    &shapes,
                ) {
                    d.axis_flags[a] = (d.axis_flags[a] & !(4 | 8 | 0x10))
                        | crate::controllers::AXIS_SHAPES[shp].1;
                    dirty = true;
                }
            }
            y += ROW + 6.0;
        }
        // buttons: their key actions
        y += 10.0;
        ui.text_in(
            "Buttons",
            Rect::new(x0, y, w, 20.0),
            14.0,
            Weight::Bold,
            TEXT,
            Align::Left,
        );
        y += 26.0;
        buttons_start_y = y - v.y;
        let cols = if w < 560.0 { 1usize } else { 2 };
        let cw = (w - GAP * (cols - 1) as f32) / cols as f32;
        let per_row = ROW + 4.0;
        let rows = shown_buttons.div_ceil(cols);
        let latching = &mut d.latching;
        for (b, (act, _)) in d.buttons.iter_mut().take(shown_buttons).enumerate() {
            let (col, row) = (b / rows.max(1), b % rows.max(1));
            let r = Rect::new(
                x0 + col as f32 * (cw + GAP),
                y + row as f32 * per_row,
                cw,
                ROW,
            );
            let label = match b.checked_sub(crate::controllers::HAT_BUTTONS) {
                Some(h) => omsi_ui::tr("Hat %{n} %{dir}")
                    .replace("%{n}", &(h / 4 + 1).to_string())
                    .replace(
                        "%{dir}",
                        &omsi_ui::tr(["up", "right", "down", "left"][h % 4]),
                    ),
                None => omsi_ui::tr("Button %{n}").replace("%{n}", &(b + 1).to_string()),
            };
            if lit == Some(b) {
                ui.p().rounded(
                    Rect::new(r.x - 4.0, r.y - 2.0, r.w + 8.0, r.h + 4.0),
                    6.0,
                    accent().alpha(0.28),
                );
            }
            ui.label(Rect::new(r.x, r.y, 90.0, r.h), &label);
            // (a latching switch - a turn signal lever, a lit hazard button - also switches
            // when it comes out)
            let latch_w = 104.0;
            let mut sel = actions
                .iter()
                .position(|a| a.eq_ignore_ascii_case(act))
                .unwrap_or(0);
            if ui.select(
                &format!("pad-btn-{b}"),
                Rect::new(r.x + 90.0, r.y, r.w - 90.0 - latch_w - GAP, r.h),
                &mut sel,
                &labels,
            ) {
                *act = if sel == 0 {
                    String::new()
                } else {
                    actions[sel].clone()
                };
                dirty = true;
            }
            let mut latched = latching.contains(&b);
            if ui.toggle(
                &format!("pad-latch-{b}"),
                Rect::new(r.right() - latch_w, r.y, latch_w, r.h),
                &mut latched,
                "Latching",
            ) {
                latching.retain(|x| *x != b);
                if latched {
                    latching.push(b);
                    latching.sort_unstable();
                }
                dirty = true;
            }
        }
        y += rows as f32 * per_row;
        y - v.y + 8.0
    });
    if dirty {
        pv.dirty = true;
    }
    // a button pressed on the device: its line (added up to it)
    if let Some((name, n)) = pressed
        .into_iter()
        .find(|(name, _)| crate::controllers::names_match(&d.name, name))
    {
        if crate::controllers::names_match(&d.name, &name)
            && n < crate::controllers::HAT_BUTTONS + 16
        {
            while d.buttons.len() <= n {
                d.buttons.push((String::new(), "0".into()));
                pv.dirty = true;
            }
            pv.capturing = false;
            pv.revealed_button = Some(n);
            let cols = if list.w - 16.0 < 560.0 { 1usize } else { 2 };
            let rows = shown_buttons.max(n + 1).div_ceil(cols).max(1);
            let row = n % rows;
            l.ui.scroll_to(
                "pad-detail",
                buttons_start_y + row as f32 * (ROW + 4.0),
                ROW,
                list.h,
            );
            pv.last_pressed = Some((n, std::time::Instant::now()));
            let now = d
                .buttons
                .get(n)
                .map(|b| b.0.clone())
                .filter(|a| !a.is_empty());
            let label = match n.checked_sub(crate::controllers::HAT_BUTTONS) {
                Some(h) => omsi_ui::tr("hat %{n} %{dir}")
                    .replace("%{n}", &(h / 4 + 1).to_string())
                    .replace(
                        "%{dir}",
                        &omsi_ui::tr(["up", "right", "down", "left"][h % 4]),
                    ),
                None => omsi_ui::tr("button %{n}").replace("%{n}", &(n + 1).to_string()),
            };
            l.state.set_status(
                match now {
                    Some(a) => omsi_ui::tr(
                        "%{name}: %{label} - %{action} (lit in the list: choose another there)",
                    )
                    .replace("%{name}", &name)
                    .replace("%{label}", &label)
                    .replace("%{action}", &action_text(names, &a)),
                    None => omsi_ui::tr(
                        "%{name}: %{label} - nothing yet (lit in the list: choose what it does)",
                    )
                    .replace("%{name}", &name)
                    .replace("%{label}", &label),
                },
                false,
            );
        }
    }
    let add_r = Rect::new(inner.x, inner.bottom() - 40.0, 260.0, 36.0);
    if l.ui.button(
        "pad-add-button",
        add_r,
        if pv.capturing {
            "Press a button on the device…"
        } else {
            "Add a button"
        },
        Some("add"),
        ButtonKind::Normal,
    ) {
        pv.capturing = !pv.capturing;
    }
    // a device no longer used (a wheel sold, one that came along in OMSI's own file) leaves
    // the list on a second click; Save keeps it so, and a connected one can be set up again
    // from the list (#636)
    let armed = pv.confirm_remove.is_some_and(|t| t.elapsed().as_secs() < 4);
    let remove_r = Rect::new(inner.right() - 260.0, inner.bottom() - 40.0, 260.0, 36.0);
    if l.ui.button(
        "pad-remove",
        remove_r,
        if armed {
            "Click again to remove"
        } else {
            "Remove this device"
        },
        Some("delete"),
        ButtonKind::Danger,
    ) {
        if armed {
            release_feedback(&mut pv.io, &mut pv.feedback_test);
            let name = remove_device(devices, &mut pv.selected);
            pv.capturing = false;
            pv.revealed_button = None;
            pv.last_pressed = None;
            pv.confirm_remove = None;
            pv.dirty = true;
            l.state.set_status(
                omsi_ui::tr("%{name} removed: press Save to keep it so.").replace("%{name}", &name),
                false,
            );
        } else {
            pv.confirm_remove = Some(std::time::Instant::now());
        }
    }
}

/// Take the device shown (`selected`) out of the list; the one below it (or the last) is
/// shown next. Its name.
fn remove_device(devices: &mut Vec<crate::controllers::DeviceCfg>, selected: &mut usize) -> String {
    let name = devices.remove(*selected).name;
    *selected = (*selected).min(devices.len().saturating_sub(1));
    name
}

/// The steps of the set-up assistant (see `Wizard`): what the player is asked each time.
const WIZARD_STEPS: [(&str, &str); 5] = [
    ("Let go of everything", "Take your hands off the wheel and your feet off the pedals (the wheel in the middle), then press Next."),
    ("Steering", "Turn the wheel (or move the stick) all the way to the LEFT and hold it there, then press Next."),
    ("Throttle", "Press the throttle pedal all the way down and hold it, then press Next. No pedals: Skip."),
    ("Brake", "Press the brake pedal all the way down and hold it, then press Next. No brake pedal: Skip."),
    ("Clutch", "Press the clutch pedal all the way down and hold it, then press Next. No clutch: Skip."),
];

/// One frame of the assistant in `r`; Some(true) when it has set the device up, Some(false)
/// when the player gave up.
fn wizard(
    ui: &mut Ui,
    r: Rect,
    w: &mut Wizard,
    d: &mut crate::controllers::DeviceCfg,
    live: &[(usize, f32)],
    connected: bool,
    feedback: bool,
) -> Option<bool> {
    let (title, text) = WIZARD_STEPS[w.step];
    ui.text_in(
        &omsi_ui::tr("Step %{n} of %{total}: %{title}")
            .replace("%{n}", &(w.step + 1).to_string())
            .replace("%{total}", &WIZARD_STEPS.len().to_string())
            .replace("%{title}", &omsi_ui::tr(title)),
        Rect::new(r.x, r.y, r.w, 26.0),
        17.0,
        Weight::Bold,
        TEXT,
        Align::Left,
    );
    let mut y = r.y + 34.0;
    y += ui.paragraph(
        text,
        Vec2::new(r.x, y),
        r.w,
        13.5,
        Weight::Regular,
        TEXT_SOFT,
    ) + 10.0;
    if !connected {
        y += ui.paragraph(
            "The device is not connected: plug it in (the list on the left marks it green).",
            Vec2::new(r.x, y),
            r.w,
            13.0,
            Weight::Medium,
            DANGER,
        );
    }
    if let Some(e) = &w.error {
        y += ui.paragraph(e, Vec2::new(r.x, y), r.w, 13.0, Weight::Medium, DANGER) + 6.0;
    }
    // the axes as they stand, so that the player sees the device answer
    for (k, v) in live {
        let bar = Rect::new(r.x + 90.0, y + 8.0, (r.w - 100.0).max(40.0), 8.0);
        ui.text_in(
            ["X", "Y", "Z", "Rx", "Ry", "Rz", "Slider 1", "Slider 2"][*k],
            Rect::new(r.x, y, 84.0, 24.0),
            12.0,
            Weight::Medium,
            TEXT_DIM,
            Align::Left,
        );
        ui.p().rounded(bar, 4.0, Color::WHITE.alpha(0.06));
        let x = bar.x + (v.clamp(-1.0, 1.0) + 1.0) * 0.5 * bar.w;
        ui.p().rounded(
            Rect::new(x - 2.0, bar.y - 4.0, 4.0, bar.h + 8.0),
            2.0,
            accent(),
        );
        y += 26.0;
    }
    let now = |live: &[(usize, f32)]| {
        let mut a = [None; 8];
        for (k, v) in live {
            a[*k] = Some(*v);
        }
        a
    };
    let by = r.bottom() - 40.0;
    if ui.button(
        "wiz-cancel",
        Rect::new(r.x, by, 120.0, 36.0),
        "Cancel",
        None,
        ButtonKind::Ghost,
    ) {
        return Some(false);
    }
    let skip = w.step >= 2
        && ui.button(
            "wiz-skip",
            Rect::new(r.right() - 260.0, by, 110.0, 36.0),
            "Skip",
            None,
            ButtonKind::Normal,
        );
    let next = ui.button(
        "wiz-next",
        Rect::new(r.right() - 140.0, by, 140.0, 36.0),
        if w.step + 1 == WIZARD_STEPS.len() && !feedback {
            "Finish"
        } else {
            "Next"
        },
        Some("chevron_right"),
        ButtonKind::Primary,
    );
    if !(next || skip) {
        return None;
    }
    let cur = now(live);
    w.error = None;
    if w.step == 0 {
        if live.is_empty() {
            w.error = Some("The device shows no axis yet: move the wheel and the pedals a little, let go, and press Next again.".into());
            return None;
        }
        w.rest = cur;
    } else if skip {
        w.at.push([None; 8]);
    } else {
        // the axis that moved most since everything was let go (one taken before is not
        // taken again - but the throttle's axis may turn out to be the brake's too)
        let used: Vec<usize> =
            w.at.iter()
                .filter_map(|a| moved_most(&w.rest, a, &[]).map(|m| m.0))
                .collect();
        let exclude: Vec<usize> = if w.step == 3 {
            used.iter().copied().take(1).collect()
        } else {
            used.clone()
        };
        match moved_most(&w.rest, &cur, &exclude) {
            Some(_) => w.at.push(cur),
            None => {
                w.error = Some(
                    "Nothing moved far enough. Hold it all the way, then press Next (or Skip)."
                        .into(),
                );
                return None;
            }
        }
    }
    w.step += 1;
    if w.step < WIZARD_STEPS.len() {
        return None;
    }
    let axes = wizard_result(&w.rest, &w.at);
    if feedback
        && axes
            .iter()
            .any(|a| matches!(a, Some((crate::controllers::Func::Steering, _))))
    {
        return None;
    }
    d.axes = axes;
    Some(true)
}

fn feedback_setup(
    ui: &mut Ui,
    r: Rect,
    w: &mut Wizard,
    d: &mut crate::controllers::DeviceCfg,
    live: &[(usize, f32)],
    device: Option<&crate::controllers::Connected>,
    io: &mut Option<crate::controllers::Devices>,
    active: &mut bool,
    hwnd: Option<isize>,
    global_invert: bool,
) -> Option<bool> {
    let axes = wizard_result(&w.rest, &w.at);
    let axis = axes
        .iter()
        .position(|a| matches!(a, Some((crate::controllers::Func::Steering, _))));
    ui.text_in(
        "Force feedback direction",
        Rect::new(r.x, r.y, r.w, 26.0),
        17.0,
        Weight::Bold,
        TEXT,
        Align::Left,
    );
    let warning = "INJURY RISK: TAKE YOUR HANDS OFF THE WHEEL. Keep hands and fingers clear before starting and throughout the test.";
    let warning_height = ui.paragraph_height(warning, r.w - 58.0, 14.5, Weight::Bold) + 20.0;
    let warning_rect = Rect::new(r.x, r.y + 34.0, r.w, warning_height);
    ui.p().rounded(warning_rect, 6.0, DANGER.alpha(0.15));
    ui.p().rounded_border(warning_rect, 6.0, 1.5, DANGER);
    ui.icon(
        "warning",
        Vec2::new(warning_rect.x + 22.0, warning_rect.center().y),
        26.0,
        DANGER,
    );
    ui.paragraph(
        warning,
        Vec2::new(warning_rect.x + 44.0, warning_rect.y + 10.0),
        warning_rect.w - 58.0,
        14.5,
        Weight::Bold,
        DANGER,
    );
    let body_y = warning_rect.bottom() + 12.0;
    ui.scroll_area("wiz-ff-body", Rect::new(r.x, body_y, r.w, r.bottom() - 56.0 - body_y), &mut |ui, r| {
        let mut y = r.y;
        y += ui.paragraph("The test applies two short forces in opposite directions. Finish and press Save to keep the detected direction for this wheel.", Vec2::new(r.x, y), r.w, 13.5, Weight::Regular, TEXT_SOFT) + 16.0;
        if let Some((started, test)) = w.calibration.as_mut() {
            if *active {
                let position = axis.and_then(|a| live.iter().find(|(k, _)| *k == a).map(|(_, x)| *x));
                if let Some(force) = test.update(started.elapsed().as_secs_f32(), position) {
                    if !device.zip(axis).zip(io.as_mut()).is_some_and(|((device, axis), io)| io.calibration_pulse(&device.name, axis, force)) {
                        test.fail("Force feedback is unavailable. Choose the direction manually.");
                    }
                }
                if let Some(result) = test.result {
                    release_feedback(io, active);
                    if let Ok(invert) = result {
                        w.ff_choice = Some(invert);
                    }
                }
            }
            let (message, color) = match test.result {
                Some(Ok(false)) => ("Direction detected: normal", OK),
                Some(Ok(true)) => ("Direction detected: inverted", OK),
                Some(Err(message)) => (message, DANGER),
                None => ("Testing: keep your hands off the wheel…", TEXT_SOFT),
            };
            y += ui.paragraph(message, Vec2::new(r.x, y), r.w, 13.0, Weight::Medium, color) + 12.0;
        }
        if !*active {
            ui.slider("wiz-ff-strength", Rect::new(r.x, y, r.w, ROW), &mut w.test_strength,
                      crate::ffb_calibration::PULSE_FORCE, crate::ffb_calibration::MAX_PULSE_FORCE, 0.01,
                      "Test strength", &|v| format!("{:.0}%", v * 100.0));
            y += ROW + 8.0;
            y += ui.paragraph("If the wheel barely moves, increase Test strength and retry. Keep your hands clear.", Vec2::new(r.x, y), r.w, 13.0, Weight::Regular, TEXT_DIM) + 10.0;
            if ui.button("wiz-ff-test", Rect::new(r.x, y, 180.0, 36.0), "Start test", Some("play_arrow"), ButtonKind::Primary) {
                if device.is_none() || axis.is_none() {
                    w.error = Some("The wheel is unavailable. Reconnect it and try again.".into());
                } else {
                    *io = None;
                    *io = Some(crate::controllers::Devices::new(hwnd, true));
                    *active = true;
                    w.error = None;
                    log::info!("FFB calibration: device {}, raw steering axis {:?}, test strength {:.0}%", device.unwrap().name, axis, w.test_strength * 100.0);
                    w.calibration = Some((std::time::Instant::now(), crate::ffb_calibration::Calibration::new(w.test_strength)));
                }
            }
            y += 48.0;
            let mut invert = w.ff_choice.or(d.ff_invert).unwrap_or(global_invert);
            if ui.toggle("wiz-ff-manual", Rect::new(r.x, y, r.w, ROW), &mut invert, "Invert force feedback") {
                w.ff_choice = Some(invert);
            }
            y += ROW + 8.0;
            y += ui.paragraph("If detection is inconclusive, retry or choose the direction manually. You can change it later on this device's page.", Vec2::new(r.x, y), r.w, 13.0, Weight::Regular, TEXT_DIM) + 8.0;
        }
        if let Some(error) = &w.error {
            y += ui.paragraph(error, Vec2::new(r.x, y), r.w, 13.0, Weight::Medium, DANGER) + 8.0;
        }
        y - r.y
    });
    let by = r.bottom() - 40.0;
    if ui.button(
        "wiz-cancel",
        Rect::new(r.x, by, 120.0, 36.0),
        "Cancel",
        None,
        ButtonKind::Ghost,
    ) {
        return Some(false);
    }
    if !*active
        && ui.button(
            "wiz-ff-finish",
            Rect::new(r.right() - 140.0, by, 140.0, 36.0),
            "Finish",
            Some("check"),
            ButtonKind::Primary,
        )
    {
        d.axes = axes;
        d.ff_invert = Some(w.ff_choice.or(d.ff_invert).unwrap_or(global_invert));
        return Some(true);
    }
    None
}

/// The axes the assistant found: `rest` where everything rested, `at` where the axes stood
/// with the wheel turned left, the throttle, the brake and the clutch pressed (all None: that
/// step skipped).
fn wizard_result(
    rest: &[Option<f32>; 8],
    at: &[[Option<f32>; 8]],
) -> [Option<(crate::controllers::Func, bool)>; 8] {
    use crate::controllers::Func;
    let mut axes: [Option<(Func, bool)>; 8] = [None; 8];
    let steer = at.first().and_then(|a| moved_most(rest, a, &[]));
    if let Some((k, delta)) = steer {
        // turned left the value falls: else the axis runs the other way
        axes[k] = Some((Func::Steering, delta > 0.0));
    }
    let taken: Vec<usize> = steer.map(|s| vec![s.0]).unwrap_or_default();
    let pedal = |i: usize, ex: &[usize]| at.get(i).and_then(|a| moved_most(rest, a, ex));
    let throttle = pedal(1, &taken);
    let brake = pedal(2, &taken);
    match (throttle, brake) {
        // one axis for both (pedals on a single axis): the throttle towards the raw maximum
        // (as in OMSI), else the axis is reversed
        (Some((kt, dt)), Some((kb, db))) if kt == kb && dt * db < 0.0 => {
            axes[kt] = Some((Func::ThrottleBrake, dt < 0.0))
        }
        _ => {
            // a pedal pressed goes towards 1
            if let Some((k, dl)) = throttle {
                axes[k] = Some((Func::Throttle, dl < 0.0));
            }
            if let Some((k, dl)) = brake.filter(|b| Some(b.0) != throttle.map(|t| t.0)) {
                axes[k] = Some((Func::Brake, dl < 0.0));
            }
        }
    }
    let mut ex = taken.clone();
    ex.extend(throttle.map(|t| t.0));
    ex.extend(brake.map(|t| t.0));
    if let Some((k, dl)) = pedal(3, &ex) {
        axes[k] = Some((Func::Clutch, dl < 0.0));
    }
    axes
}

/// The axis that moved most from `rest` to `now` (at least a sixth of its travel), not one of
/// `exclude`: (slot, how far, signed).
fn moved_most(
    rest: &[Option<f32>; 8],
    now: &[Option<f32>; 8],
    exclude: &[usize],
) -> Option<(usize, f32)> {
    (0..8)
        .filter(|k| !exclude.contains(k))
        .filter_map(|k| Some((k, now[k]? - rest[k].unwrap_or(0.0))))
        .filter(|(_, d)| d.abs() > 0.33)
        .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
}

/// Write the devices to the content folder's `Inputs/gamectrler.cfg` (OMSI 2's own is only
/// read; the game takes the content folder's first). Without a writable content folder there
/// is nowhere the game would read it from, so nothing is saved.
fn save_gamectrler(
    devices: &[crate::controllers::DeviceCfg],
) -> Result<std::path::PathBuf, String> {
    let dir = core::content_dir()
        .filter(|c| (c.exists() || std::fs::create_dir_all(c).is_ok()) && omsi_cfg::is_writable(c))
        .ok_or("No writable Project Depot content folder was found")?;
    let p = dir.join("Inputs").join("gamectrler.cfg");
    crate::controllers::save_cfg_to(&p, devices)?;
    omsi_cfg::content_changed();
    Ok(p)
}

/// Settings → Driving keys: "Custom controls", the keys of the Controls page.
pub(super) fn use_custom_keys(l: &mut Launcher) -> bool {
    if l.state.settings.get("drive_keys").and_then(|v| v.as_str()) == Some("omsi") {
        return false;
    }
    l.state.settings["drive_keys"] = json!("omsi");
    l.state.settings_dirty = 0.3;
    l.state.set_status(
        "Driving keys: Custom controls - the game uses the keys of this page.",
        false,
    );
    true
}

pub(super) fn save_keys(l: &mut Launcher, vr_binding: bool) {
    match core::save_keybindings(&l.state.keybindings) {
        Ok(()) => {
            l.state.keybindings_error.clear();
            if let Ok(k) = core::get_keybindings() {
                l.state.keybindings = k;
            }
            // a key changed is a key the player wants to use: with a ready-made layout it
            // would be ignored wherever that layout has a key of its own
            if !vr_binding && use_custom_keys(l) {
                l.state.set_status("Key bindings saved; Driving keys switched to Custom controls so the game uses them.", false);
            } else {
                l.state.set_status("Key bindings saved.", false);
            }
        }
        Err(e) => {
            l.state.keybindings_error = format!("{e:#}");
            l.state.set_status(format!("{e:#}"), true);
        }
    }
}

// --- sessions ---------------------------------------------------------------------------------

fn ago(t: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(t);
    let s = now.saturating_sub(t);
    if s < 60 {
        format!("{s} s")
    } else if s < 3600 {
        omsi_ui::tr("%{m} min").replace("%{m}", &(s / 60).to_string())
    } else {
        omsi_ui::tr("%{h} h %{m} min")
            .replace("%{h}", &(s / 3600).to_string())
            .replace("%{m}", &((s / 60) % 60).to_string())
    }
}

pub fn sessions(l: &mut Launcher, area: Rect) {
    let body = l.page_title(area, UiPage::Sessions);
    let list = l.state.instances.clone();
    if list.is_empty() {
        let r = Rect::new(body.x, body.y, body.w.min(720.0), 120.0);
        l.ui.panel(r);
        l.ui.icon(
            "sports_esports",
            Vec2::new(r.x + 40.0, r.center().y),
            36.0,
            TEXT_FAINT,
        );
        l.ui.paragraph("No game is running. Start a duty on the Drive page; to drive with friends, turn on hosting on the Multiplayer page and give them the code shown here.", Vec2::new(r.x + 76.0, r.y + 30.0), r.w - 100.0, 13.5, Weight::Regular, TEXT_DIM);
        return;
    }
    let names: std::collections::HashMap<String, String> = l
        .state
        .vehicles
        .iter()
        .map(|v| (v.file.clone(), v.name.clone()))
        .collect();
    let short_bus = |b: &str| {
        names.get(b).cloned().unwrap_or_else(|| {
            b.rsplit('/')
                .next()
                .unwrap_or("")
                .trim_end_matches(".bus")
                .to_string()
        })
    };
    let mut y = body.y - l.ui.scroll.get(&id_of("sessions")).copied().unwrap_or(0.0);
    // (the page's whole width, as the other pages: at 900 a wide window had the cards in its
    // left half and the buttons in the middle of nowhere)
    let view = Rect::new(body.x, body.y, body.w, body.h);
    let mut actions: Vec<(u32, &str)> = Vec::new();
    let mut copy: Option<String> = None;
    l.ui.push_clip(view, 0.0);
    for i in &list {
        let lan = i.lan_status.clone().unwrap_or(Value::Null);
        let role = lan.get("role").and_then(|x| x.as_str()).unwrap_or("");
        let players: Vec<Value> = lan
            .get("players")
            .and_then(|x| x.as_array())
            .cloned()
            .unwrap_or_default();
        let chat: Vec<String> = lan
            .get("chat")
            .and_then(|x| x.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|c| c.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let warnings: Vec<String> = lan
            .get("warnings")
            .and_then(|x| x.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|c| c.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let log_open = l.state.open_logs.contains(&i.pid);
        let log_lines = l.state.logs.get(&i.pid).cloned().unwrap_or_default();
        let mut h = 96.0;
        if role == "host" {
            h += 96.0;
        } else if !role.is_empty() {
            h += 24.0;
        }
        h += if players.is_empty() {
            0.0
        } else {
            26.0 + players.len() as f32 * 20.0
        };
        h += if chat.is_empty() {
            0.0
        } else {
            26.0 + chat.len().min(6) as f32 * 18.0
        };
        h += warnings.len() as f32 * 20.0;
        if log_open {
            h += 220.0;
        }
        let r = Rect::new(view.x, y, view.w, h);
        l.ui.panel(r);
        let running = i.running;
        let c = Vec2::new(r.x + 24.0, r.y + 28.0);
        if running {
            let pulse = (l.ui.time * 3.0).sin() * 0.5 + 0.5;
            l.ui.p().circle(c, 6.0 + 3.0 * pulse, OK.alpha(0.25));
        }
        l.ui.p()
            .circle(c, 6.0, if running { OK } else { TEXT_FAINT });
        let duty = i
            .line
            .as_ref()
            .map(|ln| {
                omsi_ui::tr(" · line %{line}").replace(
                    "%{line}",
                    &format!(
                        "{ln}{}",
                        i.tour
                            .as_ref()
                            .map(|t| format!(" / {t}"))
                            .unwrap_or_default()
                    ),
                )
            })
            .unwrap_or_default();
        l.ui.text_in(
            &format!("{} · {}{duty}", short_map(&i.map), short_bus(&i.bus)),
            Rect::new(r.x + 42.0, r.y + 16.0, r.w - 260.0, 24.0),
            16.0,
            Weight::Black,
            TEXT,
            Align::Left,
        );
        let status = if running {
            if l.state.stopping.contains(&i.pid) || i.stopping.is_some() {
                omsi_ui::tr("stopping - saving the run…").into_owned()
            } else {
                omsi_ui::tr("running for %{time}").replace("%{time}", &ago(i.started))
            }
        } else {
            let how = if i.exit_code == Some(0) {
                String::new()
            } else if i.killed {
                omsi_ui::tr(" (killed - it did not end by itself, the run is not saved)")
                    .into_owned()
            } else {
                i.exit_code
                    .map(|c| omsi_ui::tr(" (exit code %{code})").replace("%{code}", &c.to_string()))
                    .unwrap_or_default()
            };
            omsi_ui::tr("ended%{how}").replace("%{how}", &how)
        };
        l.ui.text_in(
            &omsi_ui::tr("%{status} · driver %{name}")
                .replace("%{status}", &status)
                .replace("%{name}", &i.profile),
            Rect::new(r.x + 42.0, r.y + 42.0, r.w - 60.0, 18.0),
            12.0,
            Weight::Regular,
            TEXT_DIM,
            Align::Left,
        );
        l.ui.text_in(
            &i.last_line,
            Rect::new(r.x + 42.0, r.y + 62.0, r.w - 60.0, 18.0),
            11.5,
            Weight::Regular,
            TEXT_FAINT,
            Align::Left,
        );
        // buttons
        let bw = 110.0;
        if running {
            let stopping = l.state.stopping.contains(&i.pid);
            if l.ui.button(
                &format!("stop-{}", i.pid),
                Rect::new(r.right() - 18.0 - bw, r.y + 14.0, bw, 34.0),
                if stopping { "Stopping…" } else { "Stop" },
                Some("close"),
                ButtonKind::Danger,
            ) && !stopping
            {
                actions.push((i.pid, "stop"));
            }
        }
        if l.ui.button(
            &format!("log-{}", i.pid),
            Rect::new(
                r.right() - 18.0 - bw - if running { bw + 8.0 } else { 0.0 },
                r.y + 14.0,
                bw,
                34.0,
            ),
            if log_open { "Hide log" } else { "Show log" },
            Some("receipt_long"),
            ButtonKind::Normal,
        ) {
            actions.push((i.pid, "log"));
        }
        let mut yy = r.y + 86.0;
        if role == "host" {
            let code = lan
                .get("code")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let box_r = Rect::new(r.x + 20.0, yy, r.w - 40.0, 84.0);
            l.ui.p().rounded(box_r, 10.0, accent().alpha(0.10));
            l.ui.p()
                .rounded_border(box_r, 10.0, 1.0, accent().alpha(0.5));
            l.ui.text_in(
                "SESSION CODE",
                Rect::new(box_r.x + 16.0, box_r.y + 8.0, 200.0, 16.0),
                10.5,
                Weight::Black,
                accent(),
                Align::Left,
            );
            l.ui.text_in(
                &code,
                Rect::new(box_r.x + 16.0, box_r.y + 26.0, box_r.w - 170.0, 30.0),
                20.0,
                Weight::Condensed,
                TEXT,
                Align::Left,
            );
            l.ui.text_in(
                "Your friends paste it into Multiplayer → Connect by Code.",
                Rect::new(box_r.x + 16.0, box_r.y + 58.0, box_r.w - 170.0, 18.0),
                11.5,
                Weight::Regular,
                TEXT_DIM,
                Align::Left,
            );
            if l.ui.button(
                &format!("copy-{}", i.pid),
                Rect::new(box_r.right() - 146.0, box_r.y + 24.0, 130.0, 36.0),
                "Copy code",
                Some("content_copy"),
                ButtonKind::Primary,
            ) {
                copy = Some(code.clone());
            }
            yy += 96.0;
        } else if role == "client" {
            let connected = lan
                .get("connected")
                .and_then(|x| x.as_bool())
                .unwrap_or(false);
            let text = if let Some(rej) = lan.get("rejected").and_then(|x| x.as_str()) {
                omsi_ui::tr("not connected: %{reason}").replace("%{reason}", &rej.to_string())
            } else if connected {
                omsi_ui::tr("connected to %{host}").replace(
                    "%{host}",
                    lan.get("host_name").and_then(|x| x.as_str()).unwrap_or(""),
                )
            } else {
                "connecting…".to_string()
            };
            l.ui.text_in(
                &omsi_ui::tr("Multiplayer: %{text}").replace("%{text}", &text),
                Rect::new(r.x + 42.0, yy, r.w - 60.0, 20.0),
                12.5,
                Weight::Medium,
                if connected { OK } else { WARN },
                Align::Left,
            );
            yy += 24.0;
        }
        if !players.is_empty() {
            l.ui.text_in(
                "PLAYERS",
                Rect::new(r.x + 42.0, yy, 200.0, 18.0),
                10.5,
                Weight::Black,
                TEXT_FAINT,
                Align::Left,
            );
            yy += 22.0;
            for p in &players {
                let s = |k: &str| p.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
                let pax = p.get("passengers").and_then(|x| x.as_i64()).unwrap_or(0);
                let dest = if s("destination").is_empty() {
                    String::new()
                } else {
                    format!(" · {} → {}", s("line"), s("destination"))
                };
                l.ui.text_in(
                    &format!(
                        "{} · {}{dest}{} · {}",
                        s("name"),
                        short_bus(&s("bus")),
                        if pax > 0 {
                            omsi_ui::tr(" · %{n} passengers").replace("%{n}", &pax.to_string())
                        } else {
                            String::new()
                        },
                        s("where")
                    ),
                    Rect::new(r.x + 42.0, yy, r.w - 60.0, 18.0),
                    12.5,
                    Weight::Medium,
                    TEXT_SOFT,
                    Align::Left,
                );
                yy += 20.0;
            }
        }
        if !chat.is_empty() {
            l.ui.text_in(
                "CHAT  (V in the game to write)",
                Rect::new(r.x + 42.0, yy + 4.0, 300.0, 18.0),
                10.5,
                Weight::Black,
                TEXT_FAINT,
                Align::Left,
            );
            yy += 26.0;
            for c in chat.iter().rev().take(6).rev() {
                l.ui.text_in(
                    c,
                    Rect::new(r.x + 42.0, yy, r.w - 60.0, 18.0),
                    12.0,
                    Weight::Regular,
                    TEXT_SOFT,
                    Align::Left,
                );
                yy += 18.0;
            }
        }
        for w in &warnings {
            l.ui.icon("warning", Vec2::new(r.x + 50.0, yy + 9.0), 15.0, WARN);
            l.ui.text_in(
                w,
                Rect::new(r.x + 64.0, yy, r.w - 80.0, 18.0),
                12.0,
                Weight::Medium,
                WARN,
                Align::Left,
            );
            yy += 20.0;
        }
        if log_open {
            let lr = Rect::new(r.x + 20.0, yy + 6.0, r.w - 40.0, 204.0);
            l.ui.p().rounded(lr, 8.0, Color::rgba(6, 8, 10, 0.9));
            let text: Vec<String> = log_lines.iter().rev().take(11).rev().cloned().collect();
            for (k, line) in text.iter().enumerate() {
                l.ui.text_in(
                    line,
                    Rect::new(lr.x + 10.0, lr.y + 6.0 + k as f32 * 18.0, lr.w - 20.0, 18.0),
                    11.0,
                    Weight::Regular,
                    if line.contains("ERROR") {
                        DANGER
                    } else if line.contains("WARN") {
                        WARN
                    } else {
                        TEXT_DIM
                    },
                    Align::Left,
                );
            }
        }
        y += h + GAP;
    }
    l.ui.pop_clip();
    // scrolling the list
    let content = y + l.ui.scroll.get(&id_of("sessions")).copied().unwrap_or(0.0) - body.y;
    if l.ui.hover(view) && l.ui.input.wheel.y.abs() > 0.0 {
        let s = l.ui.scroll.entry(id_of("sessions")).or_insert(0.0);
        *s = (*s - l.ui.input.wheel.y * 40.0).clamp(0.0, (content - view.h).max(0.0));
    }
    for (pid, what) in actions {
        match what {
            "stop" => l.state.stop(pid),
            _ => {
                if !l.state.open_logs.remove(&pid) {
                    l.state.open_logs.insert(pid);
                    l.state.log_tail(pid);
                }
            }
        }
    }
    if let Some(c) = copy {
        l.ui.clipboard_out = Some(c);
        l.state.set_status("Session code copied.", false);
    }
    let _ = hhmm;
}

// --- mods ----------------------------------------------------------------------------------------

pub fn mods(l: &mut Launcher, area: Rect) {
    if !l.state.mods_asked {
        l.state.load_mods();
    }
    let body = l.page_title(area, UiPage::Mods);
    let phone = super::mobile::mobile();
    // (three columns: installing, the installs, the content folder - on a phone of one width)
    let (w0, w2) = if phone {
        let cw = (body.w - GAP * 4.0) / 3.0;
        (cw, cw)
    } else {
        (420.0f32.min(body.w * 0.36), 340.0f32.min(body.w * 0.28))
    };
    let c0 = Rect::new(body.x, body.y, w0, body.h);
    let c2 = Rect::new(body.right() - w2, body.y, w2, body.h);
    let c1 = Rect::new(
        c0.right() + GAP * 2.0,
        body.y,
        c2.x - GAP * 2.0 - (c0.right() + GAP * 2.0),
        body.h,
    );
    // install
    l.ui.panel(c0);
    let inner = Launcher::card_title(
        &mut l.ui,
        Rect::new(c0.x + 18.0, c0.y + 14.0, c0.w - 36.0, c0.h - 28.0),
        "Install a mod",
    );
    let mut y = inner.y;
    // (a .zip only, everywhere: loose files are copied by hand, see below)
    if l.ui.button(
        "mod-zip",
        Rect::new(inner.x, y, inner.w, 42.0),
        "Choose a .zip file",
        Some("o-package"),
        ButtonKind::Primary,
    ) {
        if let Some(p) = core::pick_mod() {
            l.state.install(p.to_string_lossy().to_string());
        }
    }
    y += 54.0;
    let drop = Rect::new(inner.x, y, inner.w, 130.0);
    let hot = l.pages.drop_hover;
    let t = l.ui.anim(id_of("drop"), if hot { 1.0 } else { 0.0 }, 0.1);
    l.ui.p().rounded(drop, 8.0, accent().alpha(0.08 + 0.12 * t));
    // a dashed edge
    let per = 2.0 * (drop.w + drop.h);
    let n = (per / 9.0) as usize;
    for k in 0..n {
        let s = k as f32 * per / n as f32;
        let (p, horizontal) = if s < drop.w {
            (Vec2::new(drop.x + s, drop.y), true)
        } else if s < drop.w + drop.h {
            (Vec2::new(drop.right(), drop.y + s - drop.w), false)
        } else if s < 2.0 * drop.w + drop.h {
            (
                Vec2::new(drop.right() - (s - drop.w - drop.h), drop.bottom()),
                true,
            )
        } else {
            (
                Vec2::new(drop.x, drop.bottom() - (s - 2.0 * drop.w - drop.h)),
                false,
            )
        };
        let dash = if horizontal {
            Rect::new(p.x - 2.5, p.y - 0.75, 5.0, 1.5)
        } else {
            Rect::new(p.x - 0.75, p.y - 2.5, 1.5, 5.0)
        };
        l.ui.p().rect(dash, accent().alpha(0.45 + 0.45 * t));
    }
    l.ui.icon(
        "o-plus",
        Vec2::new(drop.center().x, drop.center().y - 12.0),
        20.0,
        TEXT_SOFT,
    );
    l.ui.text_in(
        "…or drop a .zip onto this window",
        Rect::new(drop.x, drop.center().y + 4.0, drop.w, 24.0),
        13.0,
        Weight::Regular,
        TEXT_SOFT,
        Align::Center,
    );
    y += 142.0;
    if !l.state.mod_path.is_empty() {
        let p = l.state.mod_path.clone();
        y += l.ui.paragraph(
            &p,
            Vec2::new(inner.x, y),
            inner.w,
            11.5,
            Weight::Regular,
            TEXT_FAINT,
        );
        match l.state.mod_info.clone() {
            Some(Ok(i)) if i.is_archive => {
                let place = if i.in_place_ok {
                    omsi_ui::tr("can be used in place").into_owned()
                } else {
                    i.in_place.clone()
                };
                y += l.ui.paragraph(
                    &omsi_ui::tr("%{size} archive, %{n} files - %{place}")
                        .replace("%{size}", &fmt_bytes(i.archive_bytes))
                        .replace("%{n}", &i.files.to_string())
                        .replace("%{place}", &place),
                    Vec2::new(inner.x, y),
                    inner.w,
                    12.0,
                    Weight::Regular,
                    if i.in_place_ok { TEXT_DIM } else { WARN },
                );
            }
            Some(Err(e)) => {
                y += l.ui.paragraph(
                    &e,
                    Vec2::new(inner.x, y),
                    inner.w,
                    12.0,
                    Weight::Regular,
                    DANGER,
                );
            }
            _ => {}
        }
        y += 6.0;
    }
    y += l.ui.paragraph(
        "The .zip is kept as it is in the Archives folder and read in place; nothing is unpacked.",
        Vec2::new(inner.x, y),
        inner.w,
        13.0,
        Weight::Regular,
        TEXT_DIM,
    );
    let mods = l.state.mods.clone();
    if let Some(m) = &mods {
        y += 4.0 + l.ui.paragraph(
            &omsi_ui::tr(
                "A .zip put into %{folder} is installed by itself once it has finished copying.",
            )
            .replace("%{folder}", &m.inbox.to_string()),
            Vec2::new(inner.x, y + 4.0),
            inner.w,
            12.0,
            Weight::Regular,
            TEXT_FAINT,
        );
        if !m.inbox_items.is_empty() {
            // (a long name with no spaces is cut to the card, not run past it)
            let now: Vec<String> = m
                .inbox_items
                .iter()
                .map(|n| omsi_launcher_ui::kit::fit_text(&mut l.ui, n, 12.0, inner.w - 80.0))
                .collect();
            let now = now.join(", ");
            y += 4.0
                + l.ui.paragraph(
                    &omsi_ui::tr("In it now: %{list}").replace("%{list}", &now),
                    Vec2::new(inner.x, y + 4.0),
                    inner.w,
                    12.0,
                    Weight::Regular,
                    TEXT_SOFT,
                );
        }
    }
    y += 18.0;
    Launcher::part_title(
        &mut l.ui,
        Rect::new(inner.x, y, inner.w, 28.0),
        "Loose files (not a .zip)",
    );
    y += 32.0;
    y += l.ui.paragraph("The launcher does not install loose folders. If you want to install them, copy them yourself:", Vec2::new(inner.x, y), inner.w, 13.0, Weight::Regular, TEXT_SOFT) + 10.0;
    let content = mods
        .as_ref()
        .map(|m| m.content_dir.clone())
        .unwrap_or_default();
    let places = [
        (
            "mods-open-root",
            "In your original OMSI 2 folder",
            "The mod works in OMSI 2 and in the Depot.",
            TEXT_DIM,
            l.state.config.root.clone(),
        ),
        (
            "mods-open-content",
            "In the Depot content folder",
            "The mod will not be available in OMSI 2.",
            WARN,
            content,
        ),
    ];
    for (id, title, note, note_c, folder) in places {
        let r = Rect::new(inner.x, y, inner.w, 50.0);
        l.ui.p().rounded(r, 6.0, Color::WHITE.alpha(0.04));
        l.ui.icon(
            "o-folder",
            Vec2::new(r.x + 20.0, r.center().y),
            18.0,
            TEXT_SOFT,
        );
        let bw = 104.0;
        let tw = r.w - 40.0 - bw - 20.0;
        l.ui.text_in_tip(
            title,
            Rect::new(r.x + 40.0, r.y + 7.0, tw, 20.0),
            13.5,
            Weight::Bold,
            TEXT,
            Align::Left,
        );
        l.ui.text_in_tip(
            note,
            Rect::new(r.x + 40.0, r.y + 26.0, tw, 18.0),
            12.0,
            Weight::Regular,
            note_c,
            Align::Left,
        );
        let b = Rect::new(r.right() - 10.0 - bw, r.y + 11.0, bw, 28.0);
        if l.ui.button(id, b, "Open folder", None, ButtonKind::Normal) {
            if folder.trim().is_empty() || !core::open_folder(std::path::Path::new(&folder)) {
                let msg = if id == "mods-open-root" {
                    "Choose the OMSI 2 folder under Setup first."
                } else {
                    "The content folder is not there yet."
                };
                l.state.set_status(msg, true);
            }
        }
        y += 60.0;
    }
    // installs
    l.ui.panel(c1);
    let inner = Launcher::card_title(
        &mut l.ui,
        Rect::new(c1.x + 18.0, c1.y + 14.0, c1.w - 36.0, c1.h - 28.0),
        "Installs",
    );
    if l.ui.button(
        "jobs-clear",
        Rect::new(c1.right() - 18.0 - 120.0, c1.y + 12.0, 120.0, 28.0),
        "Clear finished",
        None,
        ButtonKind::Normal,
    ) {
        core::install::clear_finished();
        l.state.poll_now();
    }
    let jobs = l.state.jobs.clone();
    let mut cancel = None;
    l.ui.scroll_area("jobs", Rect::new(inner.x - 6.0, inner.y, inner.w + 12.0, inner.h), &mut |ui, v| {
        if jobs.is_empty() {
            ui.paragraph("Nothing installed since the launcher started. A .zip is checked before it is used; a cancelled or failed install leaves nothing behind.", Vec2::new(v.x + 6.0, v.y), v.w - 12.0, 12.5, Weight::Regular, TEXT_DIM);
            return 60.0;
        }
        let mut y = v.y;
        for j in &jobs {
            let running = j.finished.is_none();
            let msg_h = ui.paragraph_height(&j.message, v.w - 40.0, 12.0, Weight::Regular);
            let h = 50.0 + msg_h + if running { 44.0 } else { 0.0 } + j.warnings.len() as f32 * 18.0;
            let r = Rect::new(v.x + 6.0, y, v.w - 16.0, h);
            ui.p().rounded(r, 10.0, Color::WHITE.alpha(0.04));
            ui.text_in(&j.name, Rect::new(r.x + 12.0, r.y + 8.0, r.w - 120.0, 20.0), 13.5, Weight::Bold, TEXT, Align::Left);
            let sc = match j.state.as_str() {
                "done" => OK,
                "failed" => DANGER,
                "cancelled" => TEXT_DIM,
                _ => accent(),
            };
            ui.badge(Vec2::new(r.right() - 90.0, r.y + 10.0), &j.state.to_uppercase(), sc);
            let mut yy = r.y + 34.0;
            if running {
                let frac = if j.bytes_total > 0 { j.bytes_done as f32 / j.bytes_total as f32 } else if j.files_total > 0 { j.files_done as f32 / j.files_total as f32 } else { 0.0 };
                ui.progress(Rect::new(r.x + 12.0, yy, r.w - 24.0, 8.0), frac, true);
                ui.text_in(&omsi_ui::tr("%{done} / %{total} files · %{bytes} / %{size}").replace("%{done}", &j.files_done.to_string()).replace("%{total}", &j.files_total.to_string()).replace("%{bytes}", &fmt_bytes(j.bytes_done)).replace("%{size}", &fmt_bytes(j.bytes_total)), Rect::new(r.x + 12.0, yy + 10.0, r.w - 24.0, 16.0), 11.0, Weight::Regular, TEXT_DIM, Align::Left);
                yy += 30.0;
            }
            yy += ui.paragraph(&j.message, Vec2::new(r.x + 12.0, yy), r.w - 24.0, 12.0, Weight::Regular, if j.state == "failed" { DANGER } else { TEXT_SOFT });
            for w in &j.warnings {
                ui.text_in(&format!("⚠ {w}"), Rect::new(r.x + 12.0, yy, r.w - 24.0, 16.0), 11.0, Weight::Regular, WARN, Align::Left);
                yy += 18.0;
            }
            if running && ui.button(&format!("cancel-{}", j.id), Rect::new(r.x + 12.0, r.bottom() - 34.0, 100.0, 28.0), "Cancel", None, ButtonKind::Danger) {
                cancel = Some(j.id);
            }
            y += h + 10.0;
        }
        y - v.y
    });
    if let Some(id) = cancel {
        core::install::cancel(id);
        l.state.poll_now();
    }
    // content folder
    l.ui.panel(c2);
    let inner = Launcher::card_title(
        &mut l.ui,
        Rect::new(c2.x + 18.0, c2.y + 14.0, c2.w - 36.0, c2.h - 28.0),
        "Content folder",
    );
    let Some(m) = mods else {
        l.ui.text_in(
            "Reading…",
            Rect::new(inner.x, inner.y, inner.w, 20.0),
            12.5,
            Weight::Regular,
            TEXT_DIM,
            Align::Left,
        );
        return;
    };
    // (the free space beside the title, warm: what installs are checked against)
    l.ui.text_in(
        &omsi_ui::tr("%{size} free").replace("%{size}", &fmt_bytes(m.free_bytes)),
        Rect::new(c2.x + 18.0, c2.y + 14.0, c2.w - 36.0, 24.0),
        13.0,
        Weight::Medium,
        WARN,
        Align::Right,
    );
    // (scrolled: a long list of archives or buses waiting would run past the card)
    l.ui.scroll_area(
        "mods-content",
        Rect::new(inner.x - 6.0, inner.y, inner.w + 12.0, inner.h),
        &mut |ui, v| {
            let (x, w) = (v.x + 6.0, v.w - 16.0);
            let mut y = v.y;
            y += ui.paragraph(
                &m.content_dir,
                Vec2::new(x, y),
                w,
                11.5,
                Weight::Regular,
                TEXT_FAINT,
            );
            y += 8.0;
            for (f, n) in &m.folders {
                ui.p().rect(Rect::new(x, y, w, 1.0), EDGE);
                ui.icon("o-folder", Vec2::new(x + 9.0, y + 16.0), 16.0, TEXT_SOFT);
                ui.text_in(
                    f,
                    Rect::new(x + 26.0, y + 5.0, w * 0.6, 22.0),
                    13.0,
                    Weight::Regular,
                    TEXT,
                    Align::Left,
                );
                ui.text_in(
                    &format!("{n} {}", if *n == 1 { "entry" } else { "entries" }),
                    Rect::new(x, y + 5.0, w, 22.0),
                    13.0,
                    Weight::Regular,
                    TEXT_DIM,
                    Align::Right,
                );
                y += 31.0;
            }
            if !m.archives.is_empty() {
                y += 8.0;
                Launcher::part_title(ui, Rect::new(x, y, w, 28.0), "Archives used in place");
                y += 30.0;
                for (n, b) in &m.archives {
                    ui.text_in(
                        &format!("{n}  ({})", fmt_bytes(*b)),
                        Rect::new(x, y, w, 20.0),
                        12.0,
                        Weight::Regular,
                        TEXT_SOFT,
                        Align::Left,
                    );
                    y += 22.0;
                }
            }
            if !m.waiting.is_empty() {
                y += 8.0;
                Launcher::part_title(
                    ui,
                    Rect::new(x, y, w, 28.0),
                    "Kept aside by an older version (copy by hand)",
                );
                y += 30.0;
                for item in &m.waiting {
                    ui.text_in(
                        item,
                        Rect::new(x, y, w, 20.0),
                        12.0,
                        Weight::Regular,
                        TEXT_DIM,
                        Align::Left,
                    );
                    y += 22.0;
                }
            }
            y - v.y
        },
    );
}

// --- tutorials --------------------------------------------------------------------------------

/// OMSI's four tutorials: what each teaches, and a button to start it.
pub fn tutorials(l: &mut Launcher, area: Rect) {
    let body = l.page_title(area, UiPage::Tutorials);
    static LIST: std::sync::OnceLock<Vec<(usize, String, String)>> = std::sync::OnceLock::new();
    let list = LIST.get_or_init(omsi_launcher_lib::tutorials);
    if list.is_empty() {
        // (a card, as Sessions says it has none)
        let r = Rect::new(body.x, body.y, body.w.min(720.0), 120.0);
        l.ui.panel(r);
        l.ui.icon(
            "help",
            Vec2::new(r.x + 40.0, r.center().y),
            36.0,
            TEXT_FAINT,
        );
        l.ui.paragraph(
            "No tutorials were found in the OMSI 2 folder (Tutorials).",
            Vec2::new(r.x + 76.0, r.y + 52.0),
            r.w - 100.0,
            13.5,
            Weight::Regular,
            TEXT_DIM,
        );
        return;
    }
    let cols = 2;
    let cw = (body.w - GAP * 2.0) / cols as f32;
    let ch = ((body.h - GAP * 2.0) / 2.0).min(330.0);
    let mut start = None;
    for (k, (n, title, text)) in list.iter().enumerate() {
        let r = Rect::new(
            body.x + (k % cols) as f32 * (cw + GAP * 2.0),
            body.y + (k / cols) as f32 * (ch + GAP * 2.0),
            cw,
            ch,
        );
        l.ui.panel(r);
        l.ui.text_in(
            title,
            Rect::new(r.x + 18.0, r.y + 14.0, r.w - 36.0, 26.0),
            17.0,
            Weight::Bold,
            TEXT,
            Align::Left,
        );
        l.ui.push_clip(
            Rect::new(r.x + 18.0, r.y + 46.0, r.w - 36.0, r.h - 110.0),
            0.0,
        );
        l.ui.paragraph(
            text,
            Vec2::new(r.x + 18.0, r.y + 46.0),
            r.w - 36.0,
            12.5,
            Weight::Regular,
            TEXT_DIM,
        );
        l.ui.pop_clip();
        if l.ui.button(
            &format!("tut-{n}"),
            Rect::new(r.x + 18.0, r.bottom() - 58.0, 200.0, 40.0),
            "Start the lesson",
            Some("play_arrow"),
            ButtonKind::Primary,
        ) {
            start = Some(*n);
        }
    }
    if let Some(n) = start {
        l.state.launch_tutorial(n);
    }
}

#[cfg(test)]
mod wizard_tests {
    fn feedback_wizard() -> super::Wizard {
        super::Wizard {
            step: super::WIZARD_STEPS.len(),
            rest: [Some(0.0); 8],
            at: vec![
                [Some(-1.0), None, None, None, None, None, None, None],
                [None; 8],
                [None; 8],
                [None; 8],
            ],
            error: None,
            calibration: None,
            ff_choice: None,
            test_strength: crate::ffb_calibration::PULSE_FORCE,
        }
    }

    fn click_feedback(
        name: &str,
        w: &mut super::Wizard,
        d: &mut crate::controllers::DeviceCfg,
    ) -> Option<bool> {
        use super::*;
        let mut ui = Ui::new();
        let size = Vec2::new(900.0, 700.0);
        let area = Rect::new(20.0, 20.0, 700.0, 600.0);
        let mut io = None;
        let mut active = false;
        ui.begin(size, 1.0, 0.016);
        feedback_setup(
            &mut ui,
            area,
            w,
            d,
            &[],
            None,
            &mut io,
            &mut active,
            None,
            false,
        );
        let rect = ui.drawn[&id_of(name)];
        ui.input.mouse = rect.center();
        ui.input.pressed = true;
        ui.input.down = true;
        ui.begin(size, 1.0, 0.016);
        feedback_setup(
            &mut ui,
            area,
            w,
            d,
            &[],
            None,
            &mut io,
            &mut active,
            None,
            false,
        );
        ui.input.pressed = false;
        ui.input.down = false;
        ui.input.released = true;
        ui.begin(size, 1.0, 0.016);
        let done = feedback_setup(
            &mut ui,
            area,
            w,
            d,
            &[],
            None,
            &mut io,
            &mut active,
            None,
            false,
        );
        assert!(!active);
        assert!(io.is_none());
        done
    }

    #[test]
    fn manual_direction_is_only_applied_on_finish_and_cancel_preserves_the_device() {
        let original = crate::controllers::DeviceCfg {
            ff_invert: Some(true),
            ..Default::default()
        };
        let mut device = original.clone();
        let mut w = feedback_wizard();
        assert_eq!(click_feedback("wiz-ff-manual", &mut w, &mut device), None);
        assert_eq!(w.ff_choice, Some(false));
        assert_eq!(device, original);
        assert_eq!(
            click_feedback("wiz-cancel", &mut w, &mut device),
            Some(false)
        );
        assert_eq!(device, original);
        assert_eq!(
            click_feedback("wiz-ff-finish", &mut w, &mut device),
            Some(true)
        );
        assert_eq!(device.ff_invert, Some(false));
        assert_eq!(device.axes[0], Some((Func::Steering, false)));
    }

    #[test]
    fn disconnected_wheel_cannot_start_a_hardware_test() {
        let mut device = crate::controllers::DeviceCfg::default();
        let mut w = feedback_wizard();
        assert_eq!(click_feedback("wiz-ff-test", &mut w, &mut device), None);
        assert!(w.error.is_some());
        assert!(w.calibration.is_none());
    }

    #[test]
    fn cancelling_feedback_releases_io_and_invalidates_the_test() {
        let mut pads = super::PadsView::default();
        pads.feedback_test = true;
        pads.wizard = Some(super::Wizard {
            step: super::WIZARD_STEPS.len(),
            rest: [None; 8],
            at: Vec::new(),
            error: None,
            calibration: Some((
                std::time::Instant::now(),
                crate::ffb_calibration::Calibration::new(crate::ffb_calibration::PULSE_FORCE),
            )),
            ff_choice: None,
            test_strength: crate::ffb_calibration::PULSE_FORCE,
        });
        pads.cancel_feedback_test();
        assert!(!pads.feedback_test);
        assert!(pads.io.is_none());
        let test = &pads
            .wizard
            .as_ref()
            .unwrap()
            .calibration
            .as_ref()
            .unwrap()
            .1;
        assert!(test.result.unwrap().is_err());
        assert_eq!(pads.wizard.as_ref().unwrap().ff_choice, None);
    }

    use crate::controllers::Func;

    #[test]
    fn h_pattern_gears_have_a_clear_name() {
        assert_eq!(
            super::known_action("kw_s_1_fest").as_deref(),
            Some("Gear 1 (H-pattern)")
        );
        assert_eq!(
            super::known_action("kw_s_R_fest").as_deref(),
            Some("Gear R (H-pattern)")
        );
    }

    #[test]
    fn empty_saved_button_slots_do_not_fill_the_controller_list() {
        let mut buttons = vec![(String::new(), "0".to_string()); 131];
        buttons[10].0 = "horn".to_string();
        assert_eq!(super::shown_button_count(&buttons, 18, None), 18);
        assert_eq!(super::shown_button_count(&buttons, 18, Some(128)), 129);
    }

    #[test]
    fn a_wheel_with_three_pedals() {
        // X the wheel; Y throttle, Z brake, Rz clutch - pedals reading 1 up, -1 down (as the
        // G25's run, "reversed")
        let rest = [
            Some(0.0),
            Some(1.0),
            Some(1.0),
            None,
            None,
            Some(1.0),
            None,
            None,
        ];
        let mut left = rest;
        left[0] = Some(-1.0);
        let mut thr = rest;
        thr[1] = Some(-1.0);
        let mut brk = rest;
        brk[2] = Some(-1.0);
        let mut clu = rest;
        clu[5] = Some(-1.0);
        let a = super::wizard_result(&rest, &[left, thr, brk, clu]);
        assert_eq!(a[0], Some((Func::Steering, false)));
        assert_eq!(a[1], Some((Func::Throttle, true)));
        assert_eq!(a[2], Some((Func::Brake, true)));
        assert_eq!(a[5], Some((Func::Clutch, true)));
    }

    #[test]
    fn pedals_on_one_axis_and_a_wheel_the_other_way() {
        let rest = [Some(0.0), Some(0.0), None, None, None, None, None, None];
        let a = super::wizard_result(
            &rest,
            &[
                [Some(0.9), Some(0.0), None, None, None, None, None, None],
                [Some(0.0), Some(-1.0), None, None, None, None, None, None],
                [Some(0.0), Some(1.0), None, None, None, None, None, None],
                [None; 8],
            ],
        );
        assert_eq!(a[0], Some((Func::Steering, true)));
        assert_eq!(a[1], Some((Func::ThrottleBrake, true)));
    }
}

#[cfg(test)]
mod pad_action_tests {
    /// Every game action a button can be given is one the game carries out from a
    /// controller (app_events: `view_look_*` / `voice_radio` while held, the gears by name,
    /// the rest through `is_game_action`, the doors through `Player::action`) - Space's
    /// reset of every view among them (#1167).
    #[test]
    fn a_button_can_reset_every_view() {
        assert!(super::PAD_GAME_ACTIONS.contains(&"view_reset_all_directions"));
        assert!(super::PAD_GAME_ACTIONS.contains(&"voice_radio"));
        assert_eq!(
            super::known_action("voice_radio").as_deref(),
            Some("Multiplayer: bus radio (hold)")
        );
        for a in super::PAD_GAME_ACTIONS {
            let held = a.starts_with("view_look_") || a == "voice_radio";
            let handled = held
                || crate::actions::is_game_action(a)
                || a.starts_with("gear_")
                || crate::player::door_action(a).is_some();
            assert!(handled, "{a}");
        }
        // held pad actions are game actions so they never reach the bus script
        assert!(crate::actions::is_game_action("voice_radio"));
        assert!(crate::actions::is_game_action("view_look_left"));
    }
}

#[cfg(test)]
mod pad_remove_tests {
    use crate::controllers::{cfg_text, parse_cfg};

    /// A device taken out of the list is gone from the file Save writes, and the next one is
    /// shown - the one below it, or above it when it was the last (#636).
    #[test]
    fn a_removed_device_leaves_the_file() {
        let mut devices = parse_cfg("[ctrl]\r\nSideWinder Joystick\r\n0\r\n\r\n[ctrl]\r\nLogitech G25 Racing Wheel USB\r\n1\r\n\r\n[ctrl]\r\nMOZA R3 Base\r\n0\r\n");
        let mut sel = 1;
        assert_eq!(
            super::remove_device(&mut devices, &mut sel),
            "Logitech G25 Racing Wheel USB"
        );
        assert_eq!(sel, 1);
        let names = |text: &str| {
            parse_cfg(text)
                .into_iter()
                .map(|d| d.name)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            names(&cfg_text(&devices)),
            ["SideWinder Joystick", "MOZA R3 Base"]
        );
        assert_eq!(super::remove_device(&mut devices, &mut sel), "MOZA R3 Base");
        assert_eq!(sel, 0);
        assert_eq!(
            super::remove_device(&mut devices, &mut sel),
            "SideWinder Joystick"
        );
        assert_eq!(sel, 0);
        assert!(names(&cfg_text(&devices)).is_empty());
    }
}

#[cfg(test)]
mod profile_tests {
    fn drive() -> super::core::Session {
        serde_json::from_value(serde_json::json!({"time":0,"driver":"T","map":"maps/Spandau/global.cfg","bus":"Vehicles/X/a.bus","line":"136","tour":"2","seconds":1500.0,"metres":8400.0,"stops":14,"early":1,"late":2,"tickets":4,"cash":7.5,"crashes":0,"hurt":0,"jolts":2,"driving":96.0,"comfort":88.0,"ticketing":100.0})).unwrap()
    }

    /// The stops on time are the ones served neither early nor late (the personnel file
    /// counts all of them, and of those the early and the late ones).
    #[test]
    fn a_drives_details_count_the_stops_on_time() {
        let rows = super::drive_rows(&drive());
        let get = |k: &str| rows.iter().find(|r| r.0 == k).map(|r| r.1.clone()).unwrap();
        assert_eq!(get("On time / early / late"), "11 / 1 / 2");
        assert_eq!(get("Line"), "136 / 2");
        assert_eq!(get("Distance"), "8.4 km");
        assert_eq!(super::drive_title(&drive()), "Line 136 / 2 · Spandau");
    }
}
