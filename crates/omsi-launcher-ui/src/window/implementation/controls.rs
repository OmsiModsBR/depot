//! The Controls page: its tabs, the keyboard (`keyboard_tab` on a computer, a list of two
//! sections on a phone) and the game controllers (`pads`), whose devices are the game's
//! (`Pads`).

use super::super::domain::controls::ControlsView;
use super::super::domain::frame::Frame;
use super::super::domain::keyboard::KeyboardView;
use super::super::domain::pads::{Pads, PadsView};
use super::super::interface::output::{mobile, Page as UiPage};
use super::heading::{card_title, page_title_beside, page_title_with, title_tabs};
use super::keyboard::{keyboard_tab, labels, save_keys, use_custom_keys};
use super::pads::{read_only, tab as pads_tab};
use glam::Vec2;
use omsi_ui::paint::Align;
use omsi_ui::theme::*;
use omsi_ui::widgets::{id_of, matches, ButtonKind};
use omsi_ui::{Color, Rect, Weight};
use serde_json::json;

/// The Controls page in `area`.
pub fn controls(
    f: &mut Frame,
    cv: &mut ControlsView,
    kb: &mut KeyboardView,
    pv: &mut PadsView,
    pads: &mut dyn Pads,
    area: Rect,
) {
    // (the shown file is read again the next time OMSI 2 is chosen)
    // (the keys and controllers are changed inside OMSI 2: coming back to the page, or back
    // from a game, shows the files as they are now)
    let now = std::time::Instant::now();
    let away = pv
        .controls_seen
        .is_none_or(|t| now.duration_since(t) > std::time::Duration::from_millis(750));
    pv.controls_seen = Some(now);
    if !f.state.omsi2_engine() || away {
        pv.shown_devices = None;
    }
    if away && f.state.omsi2_engine() {
        kb.original = None;
    }
    // (where on the page is the tab shown, and its line says how it is used)
    let (_, title, keys_line) = UiPage::Controls.heading().unwrap_or_default();
    let (place, line) = if cv.tab == 0 {
        ("Keyboard", keys_line)
    } else {
        ("Game controllers", "What each axis and button of a wheel, pedals or joystick does: OMSI 2's gamectrler.cfg, in the content folder.")
    };
    // (OMSI 2 mode has only the read-only page, on a phone too)
    if !mobile() || f.state.omsi2_engine() {
        // (a computer, as the Windows 11 concept: the tabs under the heading, the keyboard in
        // groups with the chosen action beside it)
        let body = page_title_with(f.ui, area, UiPage::Controls.title(), (place, title, line));
        let mut tab = cv.tab;
        if f.ui.segmented(
            "controls-tab",
            Rect::new(body.x, body.y, 330.0, 36.0),
            &mut tab,
            &["Keyboard", "Game controllers"],
        ) {
            cv.tab = tab;
        }
        let body = Rect::new(body.x, body.y + 50.0, body.w, (body.h - 50.0).max(0.0));
        if cv.tab == 1 {
            if f.state.omsi2_engine() {
                read_only(f, pv, pads, body);
            } else {
                pads_tab(f, pv, pads, body);
            }
        } else {
            pv.cancel_feedback_test(pads);
            keyboard_tab(f, kb, body);
        }
        return;
    }
    let body = page_title_beside(
        f.ui,
        area,
        UiPage::Controls.title(),
        (place, title, line),
        320.0,
    );
    let mut tab = cv.tab;
    if f.ui.segmented(
        "controls-tab",
        title_tabs(body, 320.0),
        &mut tab,
        &["Keyboard", "Game controllers"],
    ) {
        cv.tab = tab;
    }
    if cv.tab == 1 {
        pads_tab(f, pv, pads, body);
        return;
    }
    pv.cancel_feedback_test(pads);
    // a key pressed while one binding waits for it
    if let (Some((sec, idx)), Some(code)) = (kb.capturing, f.ui.input.raw_key) {
        use winit::keyboard::KeyCode as K;
        if code == K::Escape {
            kb.capturing = None;
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
            match f.host.dik_code(code) {
                Some(scan) => {
                    let m = omsi_content::input::chord(
                        f.ui.input.shift,
                        f.ui.input.ctrl,
                        f.ui.input.alt,
                    ) as i64;
                    let section = ["vehicles", "game"][sec];
                    let vr_binding = f
                        .state
                        .keybindings
                        .get(section)
                        .and_then(|a| a.as_array())
                        .and_then(|a| a.get(idx))
                        .and_then(|b| b.get("action"))
                        .and_then(|a| a.as_str())
                        .is_some_and(|a| a.starts_with("vr_"));
                    if let Some(b) = f
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
                    save_keys(f.state, vr_binding);
                }
                None => f.state.set_status(
                    format!("{code:?} has no DirectInput scan code the game understands."),
                    true,
                ),
            }
            kb.capturing = None;
        }
        f.ui.input.raw_key = None;
    }
    // The keys below are the ones the game uses only with "Custom controls" (Settings →
    // Driving keys); the ready-made layouts keep W A S D / the arrows for driving. Say so,
    // with the switch right here - and changing a key switches by itself (see `save_keys`).
    let preset = f
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
        let th = f.ui.paragraph_height(&text, tw, 12.5, Weight::Medium);
        let bar = Rect::new(body.x, body.y, body.w, (th + 22.0).max(54.0));
        f.ui.p().rounded(bar, 8.0, accent().alpha(0.1));
        f.ui.p().rounded_border(bar, 8.0, 1.0, accent().alpha(0.45));
        f.ui.icon(
            "info",
            Vec2::new(bar.x + 22.0, bar.center().y),
            20.0,
            accent(),
        );
        f.ui.paragraph(
            &text,
            Vec2::new(bar.x + 42.0, bar.center().y - th * 0.5),
            tw,
            12.5,
            Weight::Medium,
            TEXT_SOFT,
        );
        if f.ui.button(
            "kb-use-custom",
            Rect::new(bar.right() - bw - 10.0, bar.center().y - 18.0, bw, 36.0),
            "Use these keys",
            Some("keyboard"),
            ButtonKind::Primary,
        ) {
            use_custom_keys(f.state);
        }
        let used = bar.h + 12.0;
        Rect::new(body.x, body.y + used, body.w, body.h - used)
    } else {
        body
    };
    let half = (body.w - GAP * 2.0) * 0.5;
    let names = labels(f);
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
        f.ui.panel(r);
        let inner = card_title(
            f.ui,
            Rect::new(r.x + 18.0, r.y + 14.0, r.w - 36.0, r.h - 28.0),
            title,
        );
        f.ui.text_in(
            sub,
            Rect::new(inner.x, inner.y - 6.0, inner.w, 18.0),
            12.0,
            Weight::Regular,
            TEXT_DIM,
            Align::Left,
        );
        let mut filter = std::mem::take(&mut cv.filter[sec]);
        let event_w = if sec == 0 { 138.0 } else { 0.0 };
        let filter_w = if sec == 0 {
            (inner.w - event_w - GAP).max(120.0)
        } else {
            inner.w
        };
        f.ui.text_input(
            &format!("kb-filter-{sec}"),
            Rect::new(inner.x, inner.y + 18.0, filter_w, 34.0),
            &mut filter,
            if cv.events[sec] {
                "Filter events…"
            } else {
                "Filter…"
            },
            Some("search"),
        );
        if sec == 0
            && f.ui.button(
                "kb-events",
                Rect::new(inner.x + filter_w + GAP, inner.y + 18.0, event_w, 34.0),
                if cv.events[sec] {
                    "Back to keys"
                } else {
                    "Add event…"
                },
                Some(if cv.events[sec] { "arrow_back" } else { "add" }),
                ButtonKind::Normal,
            )
        {
            cv.events[sec] = !cv.events[sec];
            filter.clear();
        }
        cv.filter[sec] = filter.clone();
        let q = filter.to_lowercase();

        // OMSI's Add event dialog: all KY_ events from the language files, including
        // event tables supplied by installed mods. Picking one adds an unbound vehicle
        // entry and immediately waits for its key.
        if sec == 0 && cv.events[sec] {
            let mut events = names.events();
            if !q.is_empty() {
                events.retain(|(action, label)| {
                    action.to_lowercase().contains(&q) || matches(label, &q)
                });
            }
            let mut picked: Option<String> = None;
            f.ui.scroll_area(
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
                if let Some(a) = f
                    .state
                    .keybindings
                    .get_mut("vehicles")
                    .and_then(|a| a.as_array_mut())
                {
                    a.push(json!({ "action": action.clone(), "scan_code": 0, "modifier": 0 }));
                    kb.capturing = Some((0, a.len() - 1));
                    cv.events[0] = false;
                    cv.filter[0] = action;
                    f.state.set_status(
                        "Event added. Press the key you want to use (Escape cancels).",
                        false,
                    );
                }
            }
            continue;
        }

        let list: Vec<(usize, String, i64, i64)> = f
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
                    || matches(&names.text(a), &q)
                    || a.to_lowercase().contains(&q)
                    || f.host.key_name(*s, *m).to_lowercase().contains(&q)
            })
            .map(|(i, a, s, m)| {
                let clash = *s != 0
                    && list
                        .iter()
                        .any(|(j, _, s2, m2)| j != i && s2 == s && m2 == m);
                (*i, names.text(a), f.host.key_name(*s, *m), clash)
            })
            .collect();
        if sec == 1 {
            shown.sort_by_key(|(_, label, _, _)| !label.starts_with("VR:"));
        }
        let capturing = kb.capturing;
        // what the row's buttons asked: (entry, cleared) a key cleared or to be pressed,
        // `more` another key for an entry's action (#854)
        let mut clicked: Option<(usize, bool)> = None;
        let mut more: Option<usize> = None;
        let time = f.ui.time;
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
            if f.ui.button(
                &format!("kb-add-{sec}"),
                Rect::new(inner.x, list_top, inner.w, 36.0),
                &omsi_ui::tr("Add \"%{action}\" and give it a key")
                    .replace("%{action}", new_action),
                Some("add"),
                ButtonKind::Normal,
            ) {
                if let Some(a) = f
                    .state
                    .keybindings
                    .get_mut(*key)
                    .and_then(|a| a.as_array_mut())
                {
                    a.push(json!({ "action": new_action, "scan_code": 0, "modifier": 0 }));
                    kb.capturing = Some((sec, a.len() - 1));
                }
                // (the filter stays: the new row is the one it shows, waiting for its key)
            }
            list_top += 44.0;
        }
        f.ui.scroll_area(
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
                    ui.tooltip(pr, "Add another key");
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
                    ui.tooltip(xr, "Remove this key");
                    if cx {
                        clicked = Some((*i, true));
                    }
                }
                shown.len() as f32 * rh
            },
        );
        match clicked {
            Some((i, true)) => {
                let vr_binding = f
                    .state
                    .keybindings
                    .get(*key)
                    .and_then(|a| a.as_array())
                    .and_then(|a| a.get(i))
                    .and_then(|b| b.get("action"))
                    .and_then(|a| a.as_str())
                    .is_some_and(|a| a.starts_with("vr_"));
                if let Some(b) = f
                    .state
                    .keybindings
                    .get_mut(*key)
                    .and_then(|a| a.as_array_mut())
                    .and_then(|a| a.get_mut(i))
                {
                    b["scan_code"] = json!(0);
                    b["modifier"] = json!(0);
                }
                save_keys(f.state, vr_binding);
            }
            Some((i, false)) => kb.capturing = Some((sec, i)),
            None => {}
        }
        if let Some(i) = more {
            if let Some(a) = f
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
                    kb.capturing = Some((sec, i + 1));
                }
            }
        }
    }
    if !f.state.keybindings_error.is_empty() {
        let e = f.state.keybindings_error.clone();
        f.ui.text_in(
            &e,
            Rect::new(body.x, body.bottom() + 4.0, body.w, 18.0),
            12.0,
            Weight::Medium,
            DANGER,
            Align::Left,
        );
    }
}
