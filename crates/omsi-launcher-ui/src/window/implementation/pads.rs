//! The Controls page's game controllers tab: the devices set up and the ones connected, the one
//! shown with what each axis and button does, its set-up assistant and the force feedback test
//! - and, in OMSI 2 mode, the OMSI 2 folder's own file, only shown. The devices themselves are
//! the game's (`Pads`).

use super::super::domain::frame::Frame;
use super::super::domain::pads::*;
use super::super::interface::output::core;
use super::heading::card_title;
use super::keyboard::{labels, read_only_bar};
use core::controllers::Connected;
use glam::Vec2;
use omsi_content::input::gamectrler::{self, DeviceCfg, Func, AXIS_SHAPES};
use omsi_ui::paint::Align;
use omsi_ui::theme::*;
use omsi_ui::widgets::{ButtonKind, Ui};
use omsi_ui::{Color, Rect, Weight};
use serde_json::json;

/// OMSI 2 mode: the devices of the OMSI 2 folder's own `Inputs/gamectrler.cfg` and what each
/// axis and button does, only shown (changed inside OMSI 2; nothing is written from here).
pub fn read_only(f: &mut Frame, pv: &mut PadsView, pads: &mut dyn Pads, body: Rect) {
    let bar = read_only_bar(f.ui, Rect::new(body.x, body.y, body.w, 44.0));
    let body = Rect::new(
        body.x,
        body.y + bar + 12.0,
        body.w,
        (body.h - bar - 12.0).max(0.0),
    );
    // (a calibration left running by the Depot engine's page must not outlive it)
    pv.cancel_feedback_test(pads);
    let root = std::path::PathBuf::from(&f.state.config.root);
    let names = labels(f);
    let devices = pv.shown_devices.get_or_insert_with(|| {
        omsi_content::input::gamectrler::read(&root.join("Inputs").join("gamectrler.cfg"))
            .unwrap_or_default()
    });
    f.ui.panel(body);
    let inner = card_title(
        f.ui,
        Rect::new(body.x + 18.0, body.y + 14.0, body.w - 36.0, body.h - 28.0),
        "Devices",
    );
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
    // one line per thing a device does: its name, then its axes and buttons
    let mut lines: Vec<(String, bool)> = Vec::new();
    for d in devices.iter() {
        lines.push((d.name.clone(), true));
        for (i, a) in d.axes.iter().enumerate() {
            if let Some((f, _)) = a {
                let label = omsi_ui::tr(Func::LABELS[(Func::code(Some(*f)) + 1) as usize]);
                lines.push((format!("{}: {}", omsi_ui::tr(AXES[i]), label), false));
            }
        }
        for (b, (action, _)) in d.buttons.iter().enumerate() {
            if !action.is_empty() {
                lines.push((
                    format!(
                        "{}: {}",
                        omsi_ui::tr("Button %{n}").replace("%{n}", &(b + 1).to_string()),
                        names.text(action)
                    ),
                    false,
                ));
            }
        }
    }
    if lines.is_empty() {
        lines.push((omsi_ui::tr("none").into_owned(), false));
    }
    f.ui.scroll_area("pad-ro", inner, &mut |ui, v| {
        let mut y = v.y;
        for (text, head) in &lines {
            if *head {
                y += 10.0;
            }
            ui.text_in(
                text,
                Rect::new(v.x, y, v.w, 22.0),
                if *head { 14.0 } else { 13.0 },
                if *head { Weight::Bold } else { Weight::Regular },
                if *head { TEXT } else { TEXT_SOFT },
                Align::Left,
            );
            y += 24.0;
        }
        y - v.y
    });
}

/// The game controllers tab (see `PadsView`).
pub fn tab(f: &mut Frame, pv: &mut PadsView, pads: &mut dyn Pads, body: Rect) {
    let names = labels(f);
    if !pads.is_open() {
        pads.open(false);
    }
    if pv.devices.is_none() {
        pv.devices = Some(pads.read_cfg());
    }
    // what the devices do now (and a button pressed while one is awaited)
    let mut pressed: Vec<(String, usize)> = Vec::new();
    let mut connected: Vec<Connected> = Vec::new();
    if pads.is_open() {
        for (name, n, down) in pads.poll() {
            if down {
                pressed.push((name, n));
            }
        }
        connected = pads.connected();
    }
    let devices = pv.devices.get_or_insert_with(Vec::new);
    let mut body = body;
    if !f.state.omsi2_engine()
        && !f
            .state
            .settings
            .get("momentary_gears")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        && pads.held_gears(devices)
    {
        let height = f.ui.paragraph("H-pattern gears are assigned. Enable return to neutral under Settings → Driving → Game controllers if your shifter has no neutral button.", Vec2::new(body.x + 12.0, body.y + 8.0), body.w - 24.0, 12.5, Weight::Regular, TEXT_DIM);
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
    f.ui.panel(left);
    let inner = card_title(
        f.ui,
        Rect::new(left.x + 18.0, left.y + 14.0, left.w - 36.0, left.h - 28.0),
        "Devices",
    );
    let mut add: Option<String> = None;
    let mut sel = pv.selected;
    let list_r = Rect::new(inner.x - 6.0, inner.y, inner.w + 12.0, inner.h - 108.0);
    let offs: Vec<String> = f
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
        let ui = &mut *f.ui;
        let devices = &*devices;
        let connected = &connected;
        ui.scroll_area("pad-list", list_r, &mut |ui, v| {
            let mut y = v.y;
            for (i, d) in devices.iter().enumerate() {
                let on = connected.iter().any(|c| core::controllers::names_match(&d.name, &c.name));
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
            for c in connected.iter().filter(|c| !devices.iter().any(|d| core::controllers::names_match(&d.name, &c.name))) {
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
        release_feedback(pads, &mut pv.feedback_test);
        pv.selected = sel;
        pv.capturing = false;
        pv.revealed_button = None;
        pv.wizard = None;
        pv.confirm_remove = None;
    }
    if let Some(name) = add {
        // (a device of buttons only - a gear shifter, a button box - has no axes for the
        // assistant: its buttons are given their keys on its page)
        let axes = !(pads.is_open() && pads.buttons_only(&name));
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
                test_strength: core::controllers::PULSE_FORCE,
            });
        }
    }
    // the dead zone (a setting of the game's)
    let dz_r = Rect::new(inner.x, inner.bottom() - 98.0, inner.w, 34.0);
    let mut dz = f
        .state
        .settings
        .get("ctrl_deadzone")
        .and_then(|x| x.as_f64())
        .unwrap_or(0.0) as f32;
    if f.ui
        .slider("pad-dz", dz_r, &mut dz, 0.0, 0.3, 0.01, "Dead zone", &|v| {
            format!("{:.0} %", v * 100.0)
        })
    {
        f.state.settings["ctrl_deadzone"] = json!(dz);
        f.state.settings_dirty = 0.3;
    }
    let save_r = Rect::new(inner.x, inner.bottom() - 48.0, inner.w, 40.0);
    if f.ui.button(
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
        match save(devices) {
            Ok(p) => {
                pv.dirty = false;
                f.state.set_status(
                    omsi_ui::tr("Game controllers saved to %{path}")
                        .replace("%{path}", &p.display().to_string()),
                    false,
                );
            }
            Err(e) => f.state.set_status(
                omsi_ui::tr("Not saved: %{error}").replace("%{error}", &e.to_string()),
                true,
            ),
        }
    }
    // the device shown
    f.ui.panel(right);
    let Some(d) = devices.get_mut(pv.selected) else {
        return;
    };
    let inner = card_title(
        f.ui,
        Rect::new(
            right.x + 18.0,
            right.y + 14.0,
            right.w - 36.0,
            right.h - 28.0,
        ),
        &d.name.clone(),
    );
    let live_dev = core::controllers::find_connected(&connected, &d.name);
    let live: Vec<(usize, f32)> = live_dev.map(|c| c.axes.clone()).unwrap_or_default();
    // every button the device has gets its line (DirectInput says how many)
    if let Some(n) = live_dev.map(|c| c.buttons).filter(|n| *n > d.buttons.len()) {
        d.buttons.resize(n, (String::new(), "0".into()));
    }
    // the assistant, over the device's page
    if let Some(w) = pv.wizard.as_mut() {
        let done = if w.step == WIZARD_STEPS.len() {
            feedback_setup(
                f.ui,
                inner,
                w,
                d,
                &live,
                live_dev,
                pads,
                &mut pv.feedback_test,
                f.state
                    .settings
                    .get("ff_invert")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            )
        } else {
            assistant(
                f.ui,
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
                release_feedback(pads, &mut pv.feedback_test);
                pv.wizard = None;
                pv.dirty = true;
                f.state.set_status(
                    "Set up: press Save to keep it (the buttons can be given their keys below).",
                    false,
                );
            }
            Some(false) => {
                release_feedback(pads, &mut pv.feedback_test);
                pv.wizard = None;
            }
            None => {}
        }
        return;
    }
    // this device on or off (a second listing of the same wheel, a device not to be used)
    {
        let offs: Vec<String> = f
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
        if f.ui.toggle(
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
            f.state.settings["ctrl_off"] = json!(offs.join("|"));
            f.state.settings_dirty = 0.3;
        }
    }
    let buttons_only = pads.is_open() && pads.buttons_only(&d.name);
    if !buttons_only
        && f.ui.button(
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
            test_strength: core::controllers::PULSE_FORCE,
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
        f.state
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
    let omsi2 = f.state.omsi2_engine();
    for a in PAD_GAME_ACTIONS {
        if (!omsi2 || crate::omsi2_map::action_in_omsi2(a)) && !actions.iter().any(|x| x == a) {
            actions.insert(1, a.to_string());
        }
    }
    actions.dedup();
    let labels: Vec<String> = actions
        .iter()
        .enumerate()
        .map(|(i, a)| if i == 0 { a.clone() } else { names.text(a) })
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
    f.ui.scroll_area("pad-detail", list, &mut |ui, v| {
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
                    f.state
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
        let shapes: Vec<String> = AXIS_SHAPES.iter().map(|s| s.0.to_string()).collect();
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
                let mut shp = AXIS_SHAPES.iter().position(|s| s.1 == curve).unwrap_or(0);
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
                    d.axis_flags[a] = (d.axis_flags[a] & !(4 | 8 | 0x10)) | AXIS_SHAPES[shp].1;
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
            let label = match b.checked_sub(core::controllers::HAT_BUTTONS) {
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
        .find(|(name, _)| core::controllers::names_match(&d.name, name))
    {
        if core::controllers::names_match(&d.name, &name) && n < core::controllers::HAT_BUTTONS + 16
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
            f.ui.scroll_to(
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
            let label = match n.checked_sub(core::controllers::HAT_BUTTONS) {
                Some(h) => omsi_ui::tr("hat %{n} %{dir}")
                    .replace("%{n}", &(h / 4 + 1).to_string())
                    .replace(
                        "%{dir}",
                        &omsi_ui::tr(["up", "right", "down", "left"][h % 4]),
                    ),
                None => omsi_ui::tr("button %{n}").replace("%{n}", &(n + 1).to_string()),
            };
            f.state.set_status(
                match now {
                    Some(a) => omsi_ui::tr(
                        "%{name}: %{label} - %{action} (lit in the list: choose another there)",
                    )
                    .replace("%{name}", &name)
                    .replace("%{label}", &label)
                    .replace("%{action}", &names.text(&a)),
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
    if f.ui.button(
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
    if f.ui.button(
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
            release_feedback(pads, &mut pv.feedback_test);
            let name = remove_device(devices, &mut pv.selected);
            pv.capturing = false;
            pv.revealed_button = None;
            pv.last_pressed = None;
            pv.confirm_remove = None;
            pv.dirty = true;
            f.state.set_status(
                omsi_ui::tr("%{name} removed: press Save to keep it so.").replace("%{name}", &name),
                false,
            );
        } else {
            pv.confirm_remove = Some(std::time::Instant::now());
        }
    }
}

/// One frame of the assistant in `r`; Some(true) when it has set the device up, Some(false)
/// when the player gave up.
fn assistant(
    ui: &mut Ui,
    r: Rect,
    w: &mut Wizard,
    d: &mut DeviceCfg,
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
    if feedback && axes.iter().any(|a| matches!(a, Some((Func::Steering, _)))) {
        return None;
    }
    d.axes = axes;
    Some(true)
}

/// The assistant's last step on a wheel that pushes back: which way its force runs, found by
/// the test (`Calibration`) or chosen by hand. Some(true) when it is set up.
pub fn feedback_setup(
    ui: &mut Ui,
    r: Rect,
    w: &mut Wizard,
    d: &mut DeviceCfg,
    live: &[(usize, f32)],
    device: Option<&Connected>,
    pads: &mut dyn Pads,
    active: &mut bool,
    global_invert: bool,
) -> Option<bool> {
    let axes = wizard_result(&w.rest, &w.at);
    let axis = axes
        .iter()
        .position(|a| matches!(a, Some((Func::Steering, _))));
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
                    if !device.zip(axis).is_some_and(|(device, axis)| pads.calibration_pulse(&device.name, axis, force)) {
                        test.fail("Force feedback is unavailable. Choose the direction manually.");
                    }
                }
                if let Some(result) = test.result {
                    release_feedback(pads, active);
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
                      core::controllers::PULSE_FORCE, core::controllers::MAX_PULSE_FORCE, 0.01,
                      "Test strength", &|v| format!("{:.0}%", v * 100.0));
            y += ROW + 8.0;
            y += ui.paragraph("If the wheel barely moves, increase Test strength and retry. Keep your hands clear.", Vec2::new(r.x, y), r.w, 13.0, Weight::Regular, TEXT_DIM) + 10.0;
            if ui.button("wiz-ff-test", Rect::new(r.x, y, 180.0, 36.0), "Start test", Some("play_arrow"), ButtonKind::Primary) {
                if device.is_none() || axis.is_none() {
                    w.error = Some("The wheel is unavailable. Reconnect it and try again.".into());
                } else {
                    pads.open(true);
                    *active = true;
                    w.error = None;
                    log::info!("FFB calibration: device {}, raw steering axis {:?}, test strength {:.0}%", device.unwrap().name, axis, w.test_strength * 100.0);
                    w.calibration = Some((std::time::Instant::now(), core::controllers::Calibration::new(w.test_strength)));
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

/// Write the devices to the content folder's `Inputs/gamectrler.cfg` (OMSI 2's own is only
/// read; the game takes the content folder's first). Without a writable content folder there
/// is nowhere the game would read it from, so nothing is saved.
fn save(devices: &[DeviceCfg]) -> Result<std::path::PathBuf, String> {
    let dir = core::content_dir()
        .filter(|c| (c.exists() || std::fs::create_dir_all(c).is_ok()) && omsi_cfg::is_writable(c))
        .ok_or("No writable Project Depot content folder was found")?;
    let p = dir.join("Inputs").join("gamectrler.cfg");
    gamectrler::write(&p, devices)?;
    omsi_cfg::content_changed();
    Ok(p)
}
