//! The launcher's other pages: the driver's profile, the settings, the key bindings, the
//! running games, the mods and where things are.

use super::theme::*;
use super::ui::{id_of, matches, ButtonKind};
use super::Launcher;
use glam::Vec2;
use omsi_launcher_ui::Page as UiPage;
use omsi_ui::paint::Align;
use omsi_ui::{Color, Rect, Weight};
use serde_json::json;

#[derive(Default)]
pub struct PagesView {
    /// The Profile page's driver and drive chosen (`omsi_launcher_ui::window::profile`).
    pub profile: omsi_launcher_ui::window::ProfileView,
    /// The "reset every setting" dialog is open.
    pub confirm_reset: bool,
    pub kb_filter: [String; 2],
    /// The OMSI-style "Add event..." browser is open for a keyboard section.
    pub kb_events: [bool; 2],
    pub drop_hover: bool,
    /// The Setup page's folders as edited, and its Content card (`setup`).
    pub setup: omsi_launcher_ui::window::SetupView,
    /// The Controls page's tab: 0 the keyboard, 1 the game controllers.
    pub controls_tab: usize,
    /// The Settings page's section and search (`settings`).
    pub set: omsi_launcher_ui::window::SettingsView,
    /// The Controls page's game controllers tab (`omsi_launcher_ui::window::pads_tab`).
    pub pads: omsi_launcher_ui::window::PadsView,
    /// The devices that tab has taken (`super::pads::GamePads`).
    pub pad_io: Option<crate::controllers::Devices>,
    pub tt: omsi_launcher_ui::window::TimetableView,
    /// The Diagnostics page's tab and its integrity check (`integrity`).
    pub diag: super::diagnostics::DiagView,
    /// The keyboard tab on a computer (`keyboard`).
    pub kb: omsi_launcher_ui::window::KeyboardView,
}

// --- controls ---------------------------------------------------------------------------------

/// The game controllers tab, its devices the game's.
fn game_controllers(l: &mut Launcher, body: Rect) {
    let hwnd = l
        .window
        .as_deref()
        .and_then(crate::controllers::window_handle);
    let root = l.state.config.root.clone();
    let mut pads = super::pads::GamePads {
        io: &mut l.pages.pad_io,
        hwnd,
        root: &root,
    };
    omsi_launcher_ui::window::pads_tab(frame!(l), &mut l.pages.pads, &mut pads, body);
}

/// OMSI 2 mode: the OMSI 2 folder's own controllers, only shown.
fn game_controllers_read_only(l: &mut Launcher, body: Rect) {
    let root = l.state.config.root.clone();
    let mut pads = super::pads::GamePads {
        io: &mut l.pages.pad_io,
        hwnd: None,
        root: &root,
    };
    omsi_launcher_ui::window::pads_read_only(frame!(l), &mut l.pages.pads, &mut pads, body);
}

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

pub fn known_action(a: &str) -> Option<String> {
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
    // (the shown file is read again the next time OMSI 2 is chosen)
    // (the keys and controllers are changed inside OMSI 2: coming back to the page, or back
    // from a game, shows the files as they are now)
    let now = std::time::Instant::now();
    let away = l
        .pages
        .pads
        .controls_seen
        .is_none_or(|t| now.duration_since(t) > std::time::Duration::from_millis(750));
    l.pages.pads.controls_seen = Some(now);
    if !l.state.omsi2_engine() || away {
        l.pages.pads.shown_devices = None;
    }
    if away && l.state.omsi2_engine() {
        l.pages.kb.original = None;
    }
    // (where on the page is the tab shown, and its line says how it is used)
    let (_, title, keys_line) = UiPage::Controls.heading().unwrap_or_default();
    let (place, line) = if l.pages.controls_tab == 0 {
        ("Keyboard", keys_line)
    } else {
        ("Game controllers", "What each axis and button of a wheel, pedals or joystick does: OMSI 2's gamectrler.cfg, in the content folder.")
    };
    // (OMSI 2 mode has only the read-only page, on a phone too)
    if !super::mobile::mobile() || l.state.omsi2_engine() {
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
            if l.state.omsi2_engine() {
                game_controllers_read_only(l, body);
            } else {
                game_controllers(l, body);
            }
        } else {
            l.pages.cancel_feedback_test();
            omsi_launcher_ui::window::keyboard_tab(frame!(l), &mut l.pages.kb, body);
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
    l.pages.cancel_feedback_test();
    // a key pressed while one binding waits for it
    if let (Some((sec, idx)), Some(code)) = (l.pages.kb.capturing, l.ui.input.raw_key) {
        use winit::keyboard::KeyCode as K;
        if code == K::Escape {
            l.pages.kb.capturing = None;
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
            l.pages.kb.capturing = None;
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
                    l.pages.kb.capturing = Some((0, a.len() - 1));
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
        let capturing = l.pages.kb.capturing;
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
                    l.pages.kb.capturing = Some((sec, a.len() - 1));
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
            Some((i, false)) => l.pages.kb.capturing = Some((sec, i)),
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
                    l.pages.kb.capturing = Some((sec, i + 1));
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

/// Settings → Driving keys: "Custom controls", the keys of the Controls page.
pub(super) fn use_custom_keys(l: &mut Launcher) -> bool {
    omsi_launcher_ui::window::use_custom_keys(&mut l.state)
}

pub(super) fn save_keys(l: &mut Launcher, vr_binding: bool) {
    omsi_launcher_ui::window::save_keys(&mut l.state, vr_binding);
}
