//! The launcher's other pages: the driver's profile, the settings, the key bindings, the
//! running games, the mods and where things are.

use super::theme::*;
use super::ui::{id_of, matches, ButtonKind, Ui};
use super::Launcher;
use glam::Vec2;
use omsi_launcher_lib as core;
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
    pub set: super::settings::SettingsView,
    pub pads: PadsView,
    pub tt: super::timetable::TimetableView,
    /// The Diagnostics page's tab and its integrity check (`integrity`).
    pub diag: super::diagnostics::DiagView,
    /// The keyboard tab on a computer (`keyboard`).
    pub kb: omsi_launcher_ui::window::KeyboardView,
}

/// The game controllers tab: the devices `gamectrler.cfg` sets up, the ones connected now,
/// the one shown, a button being waited for.
#[derive(Default)]
pub struct PadsView {
    pub io: Option<crate::controllers::Devices>,
    /// The set-up assistant, while it runs.
    pub wizard: Option<Wizard>,
    pub feedback_test: bool,
    pub devices: Option<Vec<crate::controllers::DeviceCfg>>,
    /// OMSI 2 mode: the devices of the OMSI 2 folder's own file, only shown.
    pub shown_devices: Option<Vec<crate::controllers::DeviceCfg>>,
    /// When the Controls page was last drawn (OMSI 2 mode reads its files again after a gap).
    pub controls_seen: Option<std::time::Instant>,
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
    pub calibration: Option<(std::time::Instant, crate::ffb_calibration::Calibration)>,
    pub ff_choice: Option<bool>,
    pub test_strength: f32,
}

impl PadsView {
    pub fn cancel_feedback_test(&mut self) {
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

// --- controls ---------------------------------------------------------------------------------

/// The game's own actions a controller's button can be given, besides the bus's: the doors
/// and gears of any bus, looking round while held, the bus radio while held, the cameras
/// and the views - both of OMSI's view resets, the one view's (C) and every view's (Space),
/// which a controller could not bring back to the first camera (#1167).
pub const PAD_GAME_ACTIONS: [&str; 26] = [
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

/// OMSI 2 mode: the devices of the OMSI 2 folder's own `Inputs/gamectrler.cfg` and what each
/// axis and button does, only shown (changed inside OMSI 2; nothing is written from here).
fn game_controllers_read_only(l: &mut Launcher, body: Rect) {
    use crate::controllers::Func;
    let bar =
        omsi_launcher_ui::window::read_only_bar(&mut l.ui, Rect::new(body.x, body.y, body.w, 44.0));
    let body = Rect::new(
        body.x,
        body.y + bar + 12.0,
        body.w,
        (body.h - bar - 12.0).max(0.0),
    );
    // (a calibration left running by the Depot engine's page must not outlive it)
    l.pages.pads.cancel_feedback_test();
    let root = std::path::PathBuf::from(&l.state.config.root);
    let names = control_names(l);
    let devices = l.pages.pads.shown_devices.get_or_insert_with(|| {
        omsi_content::input::gamectrler::read(&root.join("Inputs").join("gamectrler.cfg"))
            .unwrap_or_default()
    });
    l.ui.panel(body);
    let inner = Launcher::card_title(
        &mut l.ui,
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
                        action_text(names, action)
                    ),
                    false,
                ));
            }
        }
    }
    if lines.is_empty() {
        lines.push((omsi_ui::tr("none").into_owned(), false));
    }
    l.ui.scroll_area("pad-ro", inner, &mut |ui, v| {
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
            l.pages.pads.cancel_feedback_test();
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
    l.pages.pads.cancel_feedback_test();
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

/// Hide empty slots beyond the physical buttons without changing the saved controller file.
pub fn shown_button_count(
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
    if !l.state.omsi2_engine()
        && !l
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
    let omsi2 = l.state.omsi2_engine();
    for a in PAD_GAME_ACTIONS {
        if (!omsi2 || super::omsi2_map::action_in_omsi2(a)) && !actions.iter().any(|x| x == a) {
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
pub fn remove_device(
    devices: &mut Vec<crate::controllers::DeviceCfg>,
    selected: &mut usize,
) -> String {
    let name = devices.remove(*selected).name;
    *selected = (*selected).min(devices.len().saturating_sub(1));
    name
}

/// The steps of the set-up assistant (see `Wizard`): what the player is asked each time.
pub const WIZARD_STEPS: [(&str, &str); 5] = [
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

pub fn feedback_setup(
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
pub fn wizard_result(
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
    omsi_launcher_ui::window::use_custom_keys(&mut l.state)
}

pub(super) fn save_keys(l: &mut Launcher, vr_binding: bool) {
    omsi_launcher_ui::window::save_keys(&mut l.state, vr_binding);
}
