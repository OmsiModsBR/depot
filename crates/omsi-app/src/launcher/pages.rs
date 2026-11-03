//! What the launcher's pages keep between frames (`PagesView`), the Controls page with the
//! game's controllers, and the names of the game's own actions.

use super::Launcher;
use omsi_ui::Rect;

#[derive(Default)]
pub struct PagesView {
    /// The Profile page's driver and drive chosen (`omsi_launcher_ui::window::profile`).
    pub profile: omsi_launcher_ui::window::ProfileView,
    /// The "reset every setting" dialog is open.
    pub confirm_reset: bool,
    pub drop_hover: bool,
    /// The Setup page's folders as edited, and its Content card (`setup`).
    pub setup: omsi_launcher_ui::window::SetupView,
    /// The Controls page's tab and, on a phone, its keyboard list (`controls`).
    pub controls: omsi_launcher_ui::window::ControlsView,
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

pub(super) fn action_text(names: &crate::describe::ControlNames, a: &str) -> String {
    known_action(a).unwrap_or_else(|| names.control(a))
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

/// The Controls page in `area` (`omsi_launcher_ui::window::controls`), its controllers the
/// game's.
pub fn controls(l: &mut Launcher, area: Rect) {
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
    omsi_launcher_ui::window::controls(
        frame!(l),
        &mut l.pages.controls,
        &mut l.pages.kb,
        &mut l.pages.pads,
        &mut pads,
        area,
    );
}
