//! What the Settings page offers and remembers: the sections, every option (`OPTS`) and how
//! it is changed, shown and reset, read from and written to the settings as the launcher keeps
//! them (`Store`), and the section, search and "changed only" the page shows (`SettingsView`).

use super::super::interface::output::{core, Choice};
use super::modules::Asks;
use omsi_launcher_lib::settings::{d3d9_fixes_vanilla, RESOLUTIONS};
use omsi_ui::widgets::matches;
use serde_json::{json, Value};

/// The line under the "Physics" list (spec 002, FR-003).
pub const PHYSICS_NOTICE: &str = "A bus with [depot_physics] in its file runs Project Depot Physics when you drive it, whatever is chosen here. Mods tuned for OMSI 2 may stand or handle differently in Project Depot Physics";

/// The page's sections, as the list on the left shows them.
pub const SECTIONS: [&str; 14] = [
    "General",
    "Simulation",
    "Graphics",
    "Advanced graphics",
    "Display and VR",
    "Sound",
    "Traffic and people",
    "Driving devices",
    "Plug-ins",
    "Presets and files",
    "Interface",
    "Modules",
    "Developer",
    "Performance",
];
pub const GENERAL: usize = 0;
pub const SIMULATION: usize = 1;
pub const GRAPHICS: usize = 2;
pub(crate) const ADVANCED: usize = 3;
pub(crate) const DISPLAY: usize = 4;
pub(crate) const SOUND: usize = 5;
pub const TRAFFIC: usize = 6;
pub const DEVICES: usize = 7;
pub(crate) const PLUGINS: usize = 8;
pub const PRESETS: usize = 9;
pub const INTERFACE: usize = 10;
/// OmsiLaunch's optional modules: OMSI 2 mode only (the Depot has none).
pub const MODULES: usize = 11;
/// The Depot's developer tools (#500): the Depot engine only.
pub const DEVELOPER: usize = 12;

/// Under "DirectX9 (D3D9Ex)" only: the options that lower the load on an older or integrated-graphics
/// PC, gathered from the other sections (it owns none; see `PERFORMANCE_OPTS`).
pub const PERFORMANCE: usize = 13;

/// The options of the Performance section: key, and the card it sits in. They are the page's own
/// rows from the other sections (one row is one setting), in this order. Each lowers the load on
/// the processor, the graphics chip or the memory; the ones OMSI 2 lacks are off by default.
pub const PERFORMANCE_OPTS: &[(&str, &str)] = &[
    ("msaa", "Picture"),
    ("anisotropy", "Picture"),
    ("mirror_size", "Mirrors"),
    ("mirror_refresh", "Mirrors"),
    ("view_distance", "Distance and detail"),
    ("max_obj_dist", "Distance and detail"),
    ("min_obj_size", "Distance and detail"),
    ("ai_max_random", "Traffic and people"),
    ("ai_max_scheduled", "Traffic and people"),
    ("ai_unsched_factor", "Traffic and people"),
    ("ai_priority_scheduled", "Traffic and people"),
    ("ai_max_parked", "Traffic and people"),
    ("ai_max_humans", "Traffic and people"),
    ("pax_density", "Traffic and people"),
    ("smoke", "Effects"),
    ("smoke_user_only", "Effects"),
    ("smoke_max", "Effects"),
    ("sun_glow", "Effects"),
    ("reflections", "Effects"),
    ("texture_memory", "Textures and memory"),
    ("max_fps", "Frame rate"),
    ("vsync", "Frame rate"),
];

/// Whether the Performance section is offered with the settings `s` (the Depot's DirectX9 only).
pub fn performance_offered(s: &Value, omsi2: bool) -> bool {
    !omsi2 && d3d9_fixes_vanilla(s)
}

/// Whether section `sec` is listed: Modules only while OMSI 2 is the engine, Developer only
/// while the Depot is, Performance only where it is offered.
pub(crate) fn section_shown(sec: usize, s: &Value, omsi2: bool) -> bool {
    match sec {
        MODULES => omsi2,
        DEVELOPER => !omsi2,
        PERFORMANCE => performance_offered(s, omsi2),
        _ => true,
    }
}

/// What the page remembers: the section shown, the search, "changed only".
#[derive(Default)]
pub struct SettingsView {
    pub sec: usize,
    pub query: String,
    pub changed_only: bool,
    /// OMSI 2 mode's own settings (`~/.depot/omsi2/settings.json`), read when the page is
    /// first drawn in that mode.
    pub omsi2_saved: Option<serde_json::Map<String, Value>>,
}

/// How an option is changed.
pub enum Kind {
    Toggle,
    Pick(&'static [(&'static str, &'static str)]),
    /// A slider over `min..max` by `step`; `show` is the value's text, `keep` what is stored.
    Slide {
        min: f32,
        max: f32,
        step: f32,
        show: fn(f32) -> String,
        keep: fn(f32) -> Value,
        /// The value can also be typed in a field beside the slider.
        typed: bool,
    },
    /// The quality preset: sets several graphics options at once (not a key of the file).
    Preset,
    /// The graphics API: what this platform offers.
    Api,
    /// The texture memory: "Automatic" says what it comes to here.
    TexMem,
    Language,
}

/// When a change takes effect.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum When {
    /// The launcher's own: at once.
    Now,
    /// Read by the game when it starts.
    Next,
}

impl When {
    pub(crate) fn text(self) -> &'static str {
        match self {
            When::Now => "At once",
            When::Next => "Next start",
        }
    }
}

/// One option: where it is, what it is called, how it changes. A key starting with `start.` is
/// one of the start's choices (`Choice`), not of the settings file.
pub struct Opt {
    pub sec: usize,
    pub group: &'static str,
    pub key: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
    pub kind: Kind,
    pub when: When,
}

pub(crate) const fn toggle(
    sec: usize,
    group: &'static str,
    key: &'static str,
    label: &'static str,
    hint: &'static str,
) -> Opt {
    Opt {
        sec,
        group,
        key,
        label,
        hint,
        kind: Kind::Toggle,
        when: When::Next,
    }
}

pub(crate) const fn pick(
    sec: usize,
    group: &'static str,
    key: &'static str,
    label: &'static str,
    hint: &'static str,
    options: &'static [(&'static str, &'static str)],
) -> Opt {
    Opt {
        sec,
        group,
        key,
        label,
        hint,
        kind: Kind::Pick(options),
        when: When::Next,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn slide(
    sec: usize,
    group: &'static str,
    key: &'static str,
    label: &'static str,
    hint: &'static str,
    range: (f32, f32, f32),
    show: fn(f32) -> String,
    keep: fn(f32) -> Value,
) -> Opt {
    slide_with(sec, group, key, label, hint, range, show, keep, false)
}

/// A slider whose value can also be typed (the original's traffic and people counts).
#[allow(clippy::too_many_arguments)]
pub(crate) const fn slide_typed(
    sec: usize,
    group: &'static str,
    key: &'static str,
    label: &'static str,
    hint: &'static str,
    range: (f32, f32, f32),
    show: fn(f32) -> String,
    keep: fn(f32) -> Value,
) -> Opt {
    slide_with(sec, group, key, label, hint, range, show, keep, true)
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn slide_with(
    sec: usize,
    group: &'static str,
    key: &'static str,
    label: &'static str,
    hint: &'static str,
    (min, max, step): (f32, f32, f32),
    show: fn(f32) -> String,
    keep: fn(f32) -> Value,
    typed: bool,
) -> Opt {
    Opt {
        sec,
        group,
        key,
        label,
        hint,
        kind: Kind::Slide {
            min,
            max,
            step,
            show,
            keep,
            typed,
        },
        when: When::Next,
    }
}

pub(crate) const fn special(
    sec: usize,
    group: &'static str,
    key: &'static str,
    label: &'static str,
    hint: &'static str,
    kind: Kind,
) -> Opt {
    Opt {
        sec,
        group,
        key,
        label,
        hint,
        kind,
        when: When::Next,
    }
}

pub(crate) const fn now(o: Opt) -> Opt {
    Opt {
        when: When::Now,
        ..o
    }
}

// (the sliders' texts and what they store)
pub(crate) fn pct(v: f32) -> String {
    format!("{:.0}%", v * 100.0)
}
pub(crate) fn pct_or_omsi(v: f32) -> String {
    if (v - 1.0).abs() < 0.01 {
        "OMSI".to_string()
    } else {
        pct(v)
    }
}
pub(crate) fn pct_or_off(v: f32) -> String {
    if v <= 0.0 {
        "Off".to_string()
    } else {
        pct(v)
    }
}
pub(crate) fn whole_pct_or_off(v: f32) -> String {
    if v <= 0.0 {
        "Off".to_string()
    } else {
        format!("{v:.0}%")
    }
}
pub(crate) fn mm(v: f32) -> String {
    format!("{v:.0} mm")
}
pub(crate) fn deg(v: f32) -> String {
    format!("{v:.0}°")
}
pub(crate) fn signed_deg(v: f32) -> String {
    format!("{v:+.0}°")
}
pub(crate) fn cm(v: f32) -> String {
    format!("{:+.0} cm", v * 100.0)
}
pub(crate) fn whole_pct(v: f32) -> String {
    format!("{v:.0} %")
}
pub(crate) fn ms_or_off(v: f32) -> String {
    if v <= 0.0 {
        "Off".to_string()
    } else {
        format!("{v:.0} ms")
    }
}
pub(crate) fn secs_as_ms(v: f32) -> String {
    format!("{:.0} ms", v * 1000.0)
}
pub(crate) fn fov(v: f32) -> String {
    if v < 20.0 {
        "Default".to_string()
    } else {
        deg(v)
    }
}
pub(crate) fn outside_fov(v: f32) -> String {
    if v < 20.0 {
        "Auto".to_string()
    } else {
        deg(v)
    }
}
pub(crate) fn night(v: f32) -> String {
    if v < 0.01 {
        "Off".to_string()
    } else {
        format!("+{v:.2}")
    }
}
pub(crate) fn led(v: f32) -> String {
    if v < 0.5 {
        "Off".to_string()
    } else {
        format!("{}", v as i64)
    }
}
pub(crate) fn led_mip(v: f32) -> String {
    if v < 0.005 {
        "Off".to_string()
    } else {
        format!("{v:.2}")
    }
}
pub(crate) fn wheel_lock(v: f32) -> String {
    if v < 45.0 {
        "OMSI".to_string()
    } else {
        deg(v)
    }
}
pub(crate) fn pedal(v: f32) -> String {
    if (v - 1.0).abs() < 0.01 {
        "Normal".to_string()
    } else if v < 1.0 {
        omsi_ui::tr("Softer x%{v}").replace("%{v}", &format!("{v:.2}"))
    } else {
        omsi_ui::tr("Stronger x%{v}").replace("%{v}", &format!("{v:.2}"))
    }
}
pub(crate) fn vibration(v: f32) -> String {
    if v < 0.01 {
        "Off".to_string()
    } else if (v - 1.0).abs() < 0.01 {
        "Normal".to_string()
    } else {
        pct(v)
    }
}
pub(crate) fn fade(v: f32) -> String {
    if v < 0.01 {
        "Off".to_string()
    } else {
        format!("{:.0} ms", (v * 1000.0).round() as i32)
    }
}
pub(crate) fn count(v: f32) -> String {
    format!("{v:.0}")
}
pub(crate) fn integer(v: f32) -> Value {
    json!(v.round() as i64)
}
pub(crate) fn percent_whole(v: f32) -> String {
    format!("{v:.0}%")
}
pub(crate) fn hundredths(v: f32) -> Value {
    json!((v * 100.0).round() / 100.0)
}
pub(crate) fn tenths(v: f32) -> Value {
    json!((v * 10.0).round() / 10.0)
}
pub(crate) fn twentieths(v: f32) -> Value {
    json!((v * 20.0).round() / 20.0)
}
pub(crate) fn whole(v: f32) -> Value {
    json!(v.round())
}
pub(crate) fn as_is(v: f32) -> Value {
    json!(v)
}
pub(crate) fn whole_or_default(v: f32) -> Value {
    json!(if v < 20.0 { 0.0 } else { v.round() })
}
pub(crate) fn lock_or_omsi(v: f32) -> Value {
    json!(if v < 45.0 { 0.0 } else { v.round() })
}
pub(crate) fn quarters(v: f32) -> Value {
    json!((v / 0.25).round() * 0.25)
}
pub(crate) fn whole_int(v: f32) -> Value {
    json!(v.round() as i64)
}
pub(crate) fn by_five_hundredths(v: f32) -> Value {
    json!((v / 0.05).round() * 0.05)
}
pub(crate) fn percent_axis(v: f32) -> Value {
    json!(v.round().clamp(0.0, 100.0))
}

pub(crate) const CORNERS: &[(&str, &str)] = &[
    ("top-left", "Top left"),
    ("top-right", "Top right"),
    ("bottom-left", "Bottom left"),
    ("bottom-right", "Bottom right"),
];
pub(crate) const INFO_BAR_MODES: &[(&str, &str)] = &[
    ("full", "Full bar"),
    ("compact", "Compact bar"),
    ("essential", "Essentials only"),
    ("custom", "Custom bar"),
    ("twotier", "Two-tier bar"),
    ("split", "Split bar"),
];
pub(crate) const INFO_BAR_POSITIONS: &[(&str, &str)] = &[
    ("top", "Top of the screen"),
    ("bottom", "Bottom of the screen"),
];
pub(crate) const SEASONS: &[(&str, &str)] = &[
    ("auto", "By date"),
    ("spring", "Spring"),
    ("summer", "Summer"),
    ("autumn", "Autumn"),
    ("winter", "Winter"),
];

/// The Online options (chat, chat size, other players' names, voice chat), kept apart and
/// off the page while multiplayer is disabled: the server will be a Depot feature of its
/// own. Move them back into `OPTS` (and their keys into the test's `BEFORE`) to show them.
#[allow(dead_code)]
pub static ONLINE_OPTS: &[Opt] = &[
    toggle(GENERAL, "Online", "chat", "Chat in online games", ""),
    slide(
        GENERAL,
        "Online",
        "chat_size",
        "Chat size",
        "Ctrl and the wheel over it in the game",
        (0.5, 3.0, 0.1),
        pct,
        tenths,
    ),
    toggle(
        GENERAL,
        "Online",
        "name_tags",
        "Other players' names",
        "Above their buses",
    ),
    toggle(
        GENERAL,
        "Online",
        "voice_chat",
        "Voice chat",
        "GreenTeaSpeak with the Project Depot plugin, when the server names a voice server",
    ),
];

/// Every option of the page, section by section, card by card.
pub static OPTS: &[Opt] = &[
    // General
    now(special(
        GENERAL,
        "Language",
        "language",
        "Language",
        "The launcher's texts at once, the game's at its next start",
        Kind::Language,
    )),
    toggle(
        GENERAL,
        "Driving help",
        "steering_linear",
        "Steering linearity",
        "Keys turn the wheel at OMSI's steady pace",
    ),
    toggle(
        GENERAL,
        "Driving help",
        "old_steering",
        "Old steering",
        "The wheel stays; turn it back yourself",
    ),
    toggle(
        GENERAL,
        "Driving help",
        "red_steer_spd",
        "Dynamic steering",
        "Slower steering keys at speed (OMSI's redSteerSpd)",
    ),
    toggle(
        GENERAL,
        "Driving help",
        "blinker_cancel",
        "Indicators cancel themselves",
        "As the bus's script does",
    ),
    toggle(
        GENERAL,
        "Driving help",
        "brake_hold",
        "Keyboard brake holds",
        "Stays on until the throttle, as in OMSI",
    ),
    toggle(
        GENERAL,
        "Driving help",
        "auto_clutch",
        "Automatic clutch",
        "For manual gearboxes",
    ),
    toggle(
        GENERAL,
        "Driving help",
        "auto_shift",
        "Automated manual gearbox",
        "Shifts a manual gearbox by the engine speed",
    ),
    toggle(
        GENERAL,
        "Driving help",
        "get_up",
        "Ability to get up",
        "Ctrl+Shift+G leaves the seat",
    ),
    slide(
        GENERAL,
        "On screen",
        "ui_scale",
        "Game interface size",
        "Texts, menu, timetable and navigator",
        (0.5, 2.0, 0.05),
        pct,
        hundredths,
    ),
    toggle(
        GENERAL,
        "On screen",
        "ui_scale_window",
        "Interface grows with the window",
        "On windows taller than 1080p",
    ),
    slide(
        GENERAL,
        "On screen",
        "ui_opacity",
        "Interface opacity",
        "The panels behind the texts",
        (0.2, 1.0, 0.05),
        pct,
        hundredths,
    ),
    toggle(
        GENERAL,
        "On screen",
        "tooltips",
        "Name of the button under the mouse",
        "",
    ),
    toggle(
        GENERAL,
        "On screen",
        "show_fps",
        "Frame rate in the corner",
        "",
    ),
    toggle(
        GENERAL,
        "On screen",
        "info_bar",
        "Information bar (Shift+Y)",
        "Displays information such as the time, speed, and other details along the screen",
    ),
    pick(
        GENERAL,
        "On screen",
        "info_bar_mode",
        "Information bar layout",
        "Full bar shows every item; Compact bar the driving essentials and the next stop; Essentials only the speed, its limit and the delay; Custom bar the items chosen below",
        INFO_BAR_MODES,
    ),
    pick(
        GENERAL,
        "On screen",
        "info_bar_pos",
        "Information bar position",
        "Along the top or the bottom of the screen",
        INFO_BAR_POSITIONS,
    ),
    toggle(
        GENERAL,
        "On screen",
        "info_bar_fps",
        "Frame rate in the information bar",
        "Adds the frame rate and its 1 % low as the bar's last item",
    ),
    toggle(
        GENERAL,
        "On screen",
        "info_bar_collapsed",
        "Second tier starts folded away",
        "In the Two-tier and Split layouts the lower readouts start hidden; the arrow beside the bar shows them",
    ),
    toggle(
        GENERAL,
        "On screen",
        "info_bar_labels",
        "Names in the second tier",
        "Writes the name of each readout (Trip, Tank, ...) above its value in the Two-tier and Split layouts",
    ),
    toggle(
        GENERAL,
        "On screen",
        "info_bar_lowfuel",
        "Highlight a low tank",
        "Turns the tank meter red and marks its readout when under 15 % in the Two-tier and Split layouts",
    ),
    toggle(
        GENERAL,
        "On screen",
        "info_item_time",
        "Time",
        "Shown in the Custom bar layout",
    ),
    toggle(
        GENERAL,
        "On screen",
        "info_item_speed",
        "Speed and limit",
        "Shown in the Custom bar layout",
    ),
    toggle(
        GENERAL,
        "On screen",
        "info_item_trip",
        "Trip distance",
        "Shown in the Custom bar layout",
    ),
    toggle(
        GENERAL,
        "On screen",
        "info_item_odometer",
        "Odometer",
        "Shown in the Custom bar layout",
    ),
    toggle(
        GENERAL,
        "On screen",
        "info_item_temps",
        "Temperatures",
        "Shown in the Custom bar layout",
    ),
    toggle(
        GENERAL,
        "On screen",
        "info_item_tank",
        "Tank",
        "Shown in the Custom bar layout",
    ),
    toggle(
        GENERAL,
        "On screen",
        "info_item_pax",
        "Passengers",
        "Shown in the Custom bar layout",
    ),
    toggle(
        GENERAL,
        "On screen",
        "info_item_line",
        "Line and destination",
        "Shown in the Custom bar layout",
    ),
    toggle(
        GENERAL,
        "On screen",
        "info_item_next",
        "Next stop and delay",
        "Shown in the Custom bar layout",
    ),
    toggle(
        GENERAL,
        "On screen",
        "notes",
        "Notes in the top-left corner",
        "",
    ),
    toggle(
        GENERAL,
        "Navigator",
        "navigator",
        "Navigator",
        "Shift+N: map, schedule, off",
    ),
    toggle(
        GENERAL,
        "Navigator",
        "nav_arrows",
        "Route arrows",
        "As in OMSI 2",
    ),
    toggle(GENERAL, "Navigator", "nav_ai", "AI vehicles on the map", ""),
    toggle(
        GENERAL,
        "Navigator",
        "nav_follow_view",
        "Minimap on foot and in the free camera",
        "On foot the minimap shows a walker icon; in the free camera (F4) it centres on a camera icon and moves with the camera instead of staying on the bus",
    ),
    pick(
        GENERAL,
        "Navigator",
        "navigator_corner",
        "Corner",
        "Where the navigator sits",
        CORNERS,
    ),
    now(toggle(
        GENERAL,
        "Launcher",
        "launcher_rest",
        "The launcher rests while a game runs",
        "Gives the graphics card to the game",
    )),
    // Developer
    toggle(
        DEVELOPER,
        "Developer mode",
        "developer_mode",
        "Enable developer mode",
        "Tools for people who make or test vehicles and maps. Off by default: while it is off, nothing of them runs. The tools themselves are not in this version yet",
    ),
    // Simulation
    pick(
        SIMULATION,
        "Vehicle physics",
        "physics",
        "Physics",
        PHYSICS_NOTICE,
        &[
            ("classic", "OMSI Classic"),
            ("depot", "Project Depot Physics"),
        ],
    ),
    toggle(
        SIMULATION,
        "Collisions",
        "collision_vehicles",
        "With vehicles",
        "",
    ),
    toggle(
        SIMULATION,
        "Collisions",
        "collision_objects",
        "With objects",
        "Walls, poles",
    ),
    toggle(
        SIMULATION,
        "Collisions",
        "collision_pedestrians",
        "With people",
        "",
    ),
    toggle(
        SIMULATION,
        "Trolleybus",
        "trolleybus",
        "Overhead wire and poles",
        "For declared trolleybuses",
    ),
    pick(
        SIMULATION,
        "Wear and saving",
        "maintenance",
        "Maintenance",
        "How the buses wear",
        &[
            ("0", "Infinite (no wear)"),
            ("1", "Very bad"),
            ("2", "Bad"),
            ("3", "Normal"),
            ("4", "Good"),
        ],
    ),
    pick(
        SIMULATION,
        "Wear and saving",
        "autosave_minutes",
        "Autosave",
        "The last situation, while you play",
        &[
            ("0", "Off"),
            ("2", "Every 2 minutes"),
            ("5", "Every 5 minutes"),
            ("10", "Every 10 minutes"),
            ("15", "Every 15 minutes"),
        ],
    ),
    toggle(
        SIMULATION,
        "Clock and weather",
        "use_real_time",
        "Start at the real time",
        "",
    ),
    toggle(
        SIMULATION,
        "Clock and weather",
        "use_real_date",
        "Start on today's date",
        "",
    ),
    toggle(
        SIMULATION,
        "Clock and weather",
        "time_sync",
        "Sync the clock with the real time",
        "Locks the time",
    ),
    toggle(
        SIMULATION,
        "Clock and weather",
        "metar_sync",
        "Sync the weather with METAR",
        "Locks the weather",
    ),
    pick(
        SIMULATION,
        "Clock and weather",
        "time_speed",
        "Time speed",
        "Not in multiplayer or with the real-time sync",
        &[
            ("1", "Real time"),
            ("2", "x2"),
            ("4", "x4"),
            ("8", "x8"),
            ("15", "x15"),
            ("30", "x30"),
        ],
    ),
    pick(
        SIMULATION,
        "Start of a drive",
        "start.season",
        "Season",
        "By date follows the start's date",
        SEASONS,
    ),
    toggle(
        SIMULATION,
        "Start of a drive",
        "start.autostart",
        "Put the bus into service on start",
        "Shift+U",
    ),
    // Graphics
    special(
        GRAPHICS,
        "Graphics API",
        "graphics_api",
        "Graphics API",
        "Decides what the other graphics options can do",
        Kind::Api,
    ),
    special(
        GRAPHICS,
        "Picture",
        "preset",
        "Quality preset",
        "Sets many options at once",
        Kind::Preset,
    ),
    pick(
        GRAPHICS,
        "Picture",
        "graphics",
        "Graphics",
        "Vanilla draws what OMSI 2 draws",
        &[
            ("vanilla", "Vanilla (as OMSI 2)"),
            ("vanilla_plus", "Vanilla+"),
            ("enhanced", "Enhanced"),
            ("enhanced_plus", "Enhanced+"),
        ],
    ),
    pick(
        GRAPHICS,
        "Picture",
        "msaa",
        "Anti-aliasing",
        "Smooths edges",
        &[
            ("1", "Off"),
            ("2", "2x MSAA"),
            ("4", "4x MSAA"),
            ("8", "8x MSAA"),
        ],
    ),
    pick(
        GRAPHICS,
        "Picture",
        "render_scale",
        "Render scale",
        "Draws smaller, then fits the window",
        &[
            ("auto", "Auto"),
            ("1", "100%"),
            ("0.85", "85%"),
            ("0.75", "75%"),
            ("0.67", "67%"),
            ("0.5", "50%"),
        ],
    ),
    // (DirectX9 only, and only below 100 %, see `shown`)
    pick(
        GRAPHICS,
        "Picture",
        "dx9_upscale",
        "Upscaling",
        "How the smaller picture is scaled up to the window's size. Bilinear is the plain filter; Sharper keeps edges tighter. Both differ from OMSI 2's picture.",
        &[("bilinear", "Bilinear"), ("fsr1", "Sharper")],
    ),
    pick(
        GRAPHICS,
        "Picture",
        "anisotropy",
        "Anisotropic filtering",
        "Sharper textures at an angle",
        &[
            ("1", "Off"),
            ("2", "2x"),
            ("4", "4x"),
            ("8", "8x"),
            ("16", "16x"),
        ],
    ),
    pick(
        GRAPHICS,
        "View",
        "view_distance",
        "View distance",
        "How far you see",
        &[
            ("auto", "Default (1200 m)"),
            ("600", "600 m - fastest"),
            ("900", "900 m"),
            ("1200", "1200 m"),
            ("1500", "1500 m"),
            ("2000", "2000 m"),
            ("2500", "2500 m"),
        ],
    ),
    pick(
        GRAPHICS,
        "View",
        "max_obj_dist",
        "Object distance",
        "How far objects are drawn",
        &[
            ("auto", "Automatic"),
            ("500", "500 m"),
            ("750", "750 m"),
            ("900", "900 m"),
            ("1500", "1500 m"),
            ("3000", "3000 m"),
        ],
    ),
    pick(
        GRAPHICS,
        "View",
        "min_obj_size",
        "Small objects",
        "Hides very small things at a distance",
        &[
            ("0.005", "All"),
            ("0.013", "Normal"),
            ("0.02", "Fewer (faster)"),
            ("0.03", "Few (fastest)"),
        ],
    ),
    toggle(GRAPHICS, "Light and shadows", "shadows", "Sun shadows", ""),
    pick(
        GRAPHICS,
        "Light and shadows",
        "shadow_size",
        "Shadow map",
        "Sharper shadows, more memory",
        &[("1024", "1024"), ("2048", "2048"), ("4096", "4096")],
    ),
    toggle(
        GRAPHICS,
        "Light and shadows",
        "ssao",
        "Ambient occlusion",
        "",
    ),
    pick(
        GRAPHICS,
        "Light and shadows",
        "shadow_casters",
        "Shadows cast by",
        "",
        &[
            ("all", "Every solid mesh"),
            ("omsi", "[shadow] meshes, as OMSI"),
        ],
    ),
    toggle(
        GRAPHICS,
        "Light and shadows",
        "shadow_blobs",
        "OMSI's shadow meshes",
        "Under vehicles, in every graphics mode",
    ),
    slide(
        GRAPHICS,
        "Light and shadows",
        "night_brightness",
        "Night brightness",
        "How much brighter the night is shown",
        (0.0, 3.0, 0.25),
        night,
        quarters,
    ),
    toggle(
        GRAPHICS,
        "Effects",
        "reflections",
        "Reflection maps",
        "Paint, chrome, glass",
    ),
    pick(
        GRAPHICS,
        "Effects",
        "mirror_size",
        "Mirrors",
        "What mirrors show",
        &[
            ("0", "Off"),
            ("128", "Low (128)"),
            ("256", "Normal (256)"),
            ("512", "High (512)"),
            ("1024", "Very high (1024)"),
        ],
    ),
    pick(
        GRAPHICS,
        "Effects",
        "mirror_refresh",
        "Real-time reflections",
        "",
        &[
            ("off", "None (frozen picture)"),
            ("eco", "Economical"),
            ("full", "Full"),
        ],
    ),
    toggle(GRAPHICS, "Effects", "clouds", "Clouds", ""),
    toggle(GRAPHICS, "Effects", "sun_glow", "Sun glow", ""),
    toggle(
        GRAPHICS,
        "Effects",
        "windy_trees",
        "Windy trees",
        "Leaves sway in the wind",
    ),
    toggle(
        GRAPHICS,
        "Effects",
        "smoke",
        "Smoke and particles",
        "Exhaust, chimneys and the like",
    ),
    toggle(
        GRAPHICS,
        "Effects",
        "smoke_user_only",
        "Smoke of the player's vehicle only",
        "Other vehicles and the scenery draw none",
    ),
    slide_typed(
        GRAPHICS,
        "Effects",
        "smoke_max",
        "Most particles per system",
        "Each smoke system keeps at most this many",
        (1.0, 100.0, 1.0),
        count,
        integer,
    ),
    toggle(
        GRAPHICS,
        "Effects",
        "smoke_in_mirrors",
        "Smoke in mirrors",
        "Smoke and particles also show in the mirrors",
    ),
    // (DirectX9 only, see `shown`)
    toggle(
        GRAPHICS,
        "Effects",
        "dx9_snow",
        "Snow",
        "Snowflakes fall in snow weather",
    ),
    // (DirectX9 only, see `shown`)
    toggle(
        GRAPHICS,
        "Effects",
        "fewer_night_lights",
        "Fewer night lights",
        "Fewer street and building lamps light each spot, which saves a weak graphics card work; the night looks different from OMSI 2",
    ),
    // (DirectX9 only, our own ways to ease its work, see `shown`)
    toggle(
        GRAPHICS,
        "Effects",
        "dx9_glass_rain",
        "Raindrops on glass",
        "Drops gather on the panes in the rain",
    ),
    toggle(
        GRAPHICS,
        "Effects",
        "dx9_coronas",
        "Light coronas",
        "The glow round lamps and headlights",
    ),
    toggle(
        GRAPHICS,
        "Effects",
        "dx9_stars",
        "Stars",
        "The points of light in the night sky",
    ),
    toggle(
        GRAPHICS,
        "Effects",
        "dx9_cloud_picture",
        "Cloud picture",
        "The clouds drawn over the sky; their light on the scene stays",
    ),
    // Advanced graphics
    special(
        ADVANCED,
        "Textures",
        "texture_memory",
        "Texture memory",
        "How much the textures may take",
        Kind::TexMem,
    ),
    // (DirectX9 only, see `shown`)
    pick(
        ADVANCED,
        "Textures",
        "dx9_texture_limit",
        "Texture size limit",
        "Larger pictures load smaller: less memory, a softer image",
        &[
            ("0", "No limit"),
            ("2048", "2048 px"),
            ("1024", "1024 px"),
            ("512", "512 px"),
        ],
    ),
    toggle(
        ADVANCED,
        "Textures",
        "texture_compression",
        "Compress textures on loading",
        "Less memory, a slower first load",
    ),
    toggle(
        ADVANCED,
        "Textures",
        "detail_textures",
        "Detail texturing up close",
        "Fine grain on the ground and large walls",
    ),
    slide(
        ADVANCED,
        "LED displays",
        "led_glow",
        "LED glow",
        "How brightly the dots burn",
        (0.0, 15.0, 1.0),
        led,
        whole_int,
    ),
    slide(
        ADVANCED,
        "LED displays",
        "led_mips",
        "LED mip strength",
        "Off: the sharpest dots and the most shimmer",
        (0.0, 4.0, 0.05),
        led_mip,
        by_five_hundredths,
    ),
    // Display and VR
    toggle(DISPLAY, "Window", "fullscreen", "Fullscreen", ""),
    pick(
        DISPLAY,
        "Window",
        "resolution",
        "Window size",
        "Automatic fits the screen",
        RESOLUTIONS,
    ),
    toggle(DISPLAY, "Window", "vsync", "V-sync", ""),
    pick(
        DISPLAY,
        "Window",
        "max_fps",
        "Frame limit",
        "",
        &[
            ("0", "Screen refresh rate"),
            ("30", "30 fps"),
            ("45", "45 fps"),
            ("60", "60 fps"),
            ("120", "120 fps"),
            ("144", "144 fps"),
            ("1000", "Unlimited"),
        ],
    ),
    slide(
        DISPLAY,
        "Cameras and views",
        "fov",
        "Field of view",
        "The driver's view",
        (0.0, 120.0, 1.0),
        fov,
        whole_or_default,
    ),
    slide(
        DISPLAY,
        "Cameras and views",
        "triple_fov_deg",
        "Field of view",
        "Of the three screens",
        (0.0, 120.0, 1.0),
        fov,
        whole_or_default,
    ),
    slide(
        DISPLAY,
        "Cameras and views",
        "outside_fov",
        "Outside view field of view",
        "F3",
        (0.0, 120.0, 1.0),
        outside_fov,
        whole_or_default,
    ),
    slide(
        DISPLAY,
        "Cameras and views",
        "look_sens",
        "Mouse look sensitivity",
        "",
        (0.1, 2.0, 0.05),
        pct_or_omsi,
        hundredths,
    ),
    slide(
        DISPLAY,
        "Cameras and views",
        "look_smoothing_ms",
        "Smooth the mouse look",
        "",
        (0.0, 200.0, 10.0),
        ms_or_off,
        whole,
    ),
    toggle(
        DISPLAY,
        "Cameras and views",
        "right_stick_look",
        "Right stick turns the view",
        "",
    ),
    toggle(
        DISPLAY,
        "Cameras and views",
        "alt_view",
        "Right mouse button turns the view",
        "Shift+right zooms; off: right zooms as in OMSI",
    ),
    toggle(
        DISPLAY,
        "Cameras and views",
        "precision_zoom",
        "Precision mouse zoom",
        "A field-of-view curve instead of steps",
    ),
    toggle(
        DISPLAY,
        "Cameras and views",
        "driverview_smooth",
        "Camera glides between viewpoints",
        "",
    ),
    toggle(
        DISPLAY,
        "Cameras and views",
        "hands_in_cab",
        "Driver's hands in the cab view",
        "",
    ),
    toggle(
        DISPLAY,
        "Cameras and views",
        "camera_collision",
        "Camera collisions",
        "Outside view",
    ),
    toggle(
        DISPLAY,
        "Cameras and views",
        "driver",
        "Driver at the wheel",
        "Outside views",
    ),
    toggle(
        DISPLAY,
        "Head and motion",
        "head_movement",
        "Head moves with the bus",
        "",
    ),
    slide(
        DISPLAY,
        "Head and motion",
        "head_idle",
        "Head sway at a standstill",
        "",
        (0.0, 1.0, 0.05),
        pct_or_off,
        hundredths,
    ),
    slide(
        DISPLAY,
        "Head and motion",
        "head_idle_pace",
        "Sway pace",
        "",
        (0.5, 2.0, 0.05),
        pct,
        hundredths,
    ),
    toggle(
        DISPLAY,
        "Head and motion",
        "steer_look",
        "View turns with the steering",
        "",
    ),
    slide(
        DISPLAY,
        "Head and motion",
        "steer_look_angle",
        "Steering view angle",
        "",
        (0.0, 60.0, 1.0),
        deg,
        as_is,
    ),
    slide(
        DISPLAY,
        "Head and motion",
        "steer_look_response",
        "Steering view response",
        "",
        (0.05, 1.0, 0.05),
        secs_as_ms,
        as_is,
    ),
    toggle(
        DISPLAY,
        "Head tracking",
        "head_tracking",
        "Head tracking",
        "Native TrackIR / OpenTrack",
    ),
    slide(
        DISPLAY,
        "Head tracking",
        "head_tracking_yaw_sens",
        "Yaw sensitivity",
        "Left / right",
        (0.0, 100.0, 1.0),
        whole_pct_or_off,
        percent_axis,
    ),
    slide(
        DISPLAY,
        "Head tracking",
        "head_tracking_pitch_sens",
        "Pitch sensitivity",
        "Up / down",
        (0.0, 100.0, 1.0),
        whole_pct_or_off,
        percent_axis,
    ),
    slide(
        DISPLAY,
        "Head tracking",
        "head_tracking_roll_sens",
        "Roll sensitivity",
        "",
        (0.0, 100.0, 1.0),
        whole_pct_or_off,
        percent_axis,
    ),
    toggle(
        DISPLAY,
        "Head tracking",
        "head_tracking_invert_yaw",
        "Invert yaw",
        "",
    ),
    toggle(
        DISPLAY,
        "Head tracking",
        "head_tracking_invert_pitch",
        "Invert pitch",
        "",
    ),
    toggle(
        DISPLAY,
        "Head tracking",
        "head_tracking_invert_roll",
        "Invert roll",
        "",
    ),
    slide(
        DISPLAY,
        "Head tracking",
        "head_tracking_x_sens",
        "X sensitivity",
        "Left / right",
        (0.0, 100.0, 1.0),
        whole_pct_or_off,
        percent_axis,
    ),
    slide(
        DISPLAY,
        "Head tracking",
        "head_tracking_y_sens",
        "Y sensitivity",
        "Up / down",
        (0.0, 100.0, 1.0),
        whole_pct_or_off,
        percent_axis,
    ),
    slide(
        DISPLAY,
        "Head tracking",
        "head_tracking_z_sens",
        "Z sensitivity",
        "Forward / back",
        (0.0, 100.0, 1.0),
        whole_pct_or_off,
        percent_axis,
    ),
    toggle(
        DISPLAY,
        "Head tracking",
        "head_tracking_invert_x",
        "Invert X",
        "",
    ),
    toggle(
        DISPLAY,
        "Head tracking",
        "head_tracking_invert_y",
        "Invert Y",
        "",
    ),
    toggle(
        DISPLAY,
        "Head tracking",
        "head_tracking_invert_z",
        "Invert Z",
        "",
    ),
    toggle(
        DISPLAY,
        "Triple screen",
        "triple_screen",
        "Three screen projections",
        "Three equal screens in a row; OpenXR takes priority",
    ),
    toggle(
        DISPLAY,
        "Triple screen",
        "triple_span",
        "Span three monitors at startup",
        "",
    ),
    toggle(
        DISPLAY,
        "Triple screen",
        "triple_hud_center",
        "HUD on the centre screen",
        "",
    ),
    slide(
        DISPLAY,
        "Triple screen",
        "triple_width_mm",
        "Visible width of one panel",
        "",
        (200.0, 2000.0, 10.0),
        mm,
        as_is,
    ),
    slide(
        DISPLAY,
        "Triple screen",
        "triple_distance_mm",
        "Eye to centre screen",
        "",
        (200.0, 3000.0, 10.0),
        mm,
        as_is,
    ),
    slide(
        DISPLAY,
        "Triple screen",
        "triple_bezel_mm",
        "Both frames at each join",
        "",
        (0.0, 100.0, 1.0),
        mm,
        as_is,
    ),
    slide(
        DISPLAY,
        "Triple screen",
        "triple_left_angle_deg",
        "Left screen inward angle",
        "",
        (0.0, 90.0, 1.0),
        deg,
        as_is,
    ),
    slide(
        DISPLAY,
        "Triple screen",
        "triple_right_angle_deg",
        "Right screen inward angle",
        "",
        (0.0, 90.0, 1.0),
        deg,
        as_is,
    ),
    slide(
        DISPLAY,
        "Triple screen",
        "triple_eye_height_mm",
        "Eye above screen centre",
        "",
        (-500.0, 500.0, 1.0),
        mm,
        as_is,
    ),
    toggle(DISPLAY, "VR headset", "vr", "Use an OpenXR headset", ""),
    pick(
        DISPLAY,
        "VR headset",
        "vr_scale",
        "Eye resolution",
        "",
        &[
            ("0.5", "50%"),
            ("0.65", "65%"),
            ("0.8", "80%"),
            ("1", "100%"),
        ],
    ),
    pick(
        DISPLAY,
        "VR headset",
        "vr_head_smoothing_ms",
        "Head tracking smoothing",
        "",
        &[
            ("0", "Off"),
            ("5", "5 ms"),
            ("10", "10 ms"),
            ("20", "20 ms"),
            ("30", "30 ms"),
        ],
    ),
    pick(
        DISPLAY,
        "VR headset",
        "vr_mirror_rate",
        "Bus mirror refresh",
        "Shared by all mirrors; higher costs frame rate",
        &[
            ("0", "Off"),
            ("8", "8/s"),
            ("16", "16/s"),
            ("24", "24/s"),
            ("32", "32/s"),
            ("48", "48/s"),
            ("60", "60/s"),
            ("90", "90/s"),
            ("120", "120/s"),
            ("180", "180/s"),
            ("240", "240/s"),
            ("360", "360/s"),
            ("-1", "Every frame"),
        ],
    ),
    toggle(
        DISPLAY,
        "VR headset",
        "vr_desktop_mirror",
        "Headset picture on the monitor",
        "",
    ),
    // Sound
    slide(
        SOUND,
        "Volume",
        "volume",
        "Volume",
        "",
        (0.0, 1.0, 0.05),
        pct,
        hundredths,
    ),
    slide(
        SOUND,
        "Volume",
        "vol_ai",
        "Traffic",
        "",
        (0.0, 1.0, 0.05),
        pct,
        hundredths,
    ),
    slide(
        SOUND,
        "Volume",
        "vol_scenery",
        "Surroundings",
        "",
        (0.0, 1.0, 0.05),
        pct,
        hundredths,
    ),
    toggle(SOUND, "Sound", "doppler", "Doppler effect", ""),
    toggle(
        SOUND,
        "Sound",
        "muffle_outside",
        "Muffle outside sounds",
        "Heard from the cab",
    ),
    pick(
        SOUND,
        "Sound",
        "pax_voices",
        "Passenger voices",
        "",
        &[
            ("all", "Greetings and tickets"),
            ("tickets", "Only the ticket asked for"),
            ("off", "Silent"),
        ],
    ),
    // Traffic and people
    slide(
        TRAFFIC,
        "AI traffic",
        "start.traffic",
        "Cars around",
        "At the start of a drive",
        (0.0, 120.0, 1.0),
        count,
        as_is,
    ),
    toggle(
        TRAFFIC,
        "AI traffic",
        "start.schedule",
        "Timetable buses",
        "Drive their tours on the map",
    ),
    slide_typed(
        TRAFFIC,
        "AI traffic",
        "ai_max_random",
        "Vehicles without a timetable",
        "Cars and trucks at once, at most",
        (0.0, 1000.0, 1.0),
        count,
        integer,
    ),
    slide_typed(
        TRAFFIC,
        "AI traffic",
        "ai_max_scheduled",
        "Vehicles with a timetable",
        "Buses at once, at most",
        (0.0, 1000.0, 1.0),
        count,
        integer,
    ),
    slide_typed(
        TRAFFIC,
        "AI traffic",
        "ai_unsched_factor",
        "Road traffic factor",
        "Cars and trucks, as a share of the map's traffic",
        (1.0, 300.0, 1.0),
        percent_whole,
        integer,
    ),
    slide(
        TRAFFIC,
        "AI traffic",
        "ai_priority_scheduled",
        "Highest line priority",
        "1 only the most important lines, 4 all of them",
        (1.0, 4.0, 1.0),
        count,
        integer,
    ),
    slide_typed(
        TRAFFIC,
        "AI traffic",
        "ai_max_parked",
        "Parked cars",
        "The share of the parking spaces with a car",
        (0.0, 100.0, 1.0),
        percent_whole,
        integer,
    ),
    toggle(
        TRAFFIC,
        "People",
        "start.passengers",
        "Passengers",
        "At the stops",
    ),
    slide_typed(
        TRAFFIC,
        "People",
        "ai_max_humans",
        "Most people",
        "The most people visible at once, waiting, walking or riding",
        (0.0, 1000.0, 1.0),
        count,
        integer,
    ),
    slide_typed(
        TRAFFIC,
        "People",
        "pax_density",
        "Passengers at the stops",
        "How many people wait at the stops, as a share",
        (0.0, 2.0, 0.01),
        pct,
        hundredths,
    ),
    pick(
        TRAFFIC,
        "People",
        "boarding",
        "Boarding",
        "How passengers get their tickets",
        &[
            ("auto", "Pay and take the ticket"),
            ("pay", "The driver sells the ticket"),
            ("walk", "Just walk in"),
        ],
    ),
    toggle(
        TRAFFIC,
        "People",
        "exact_fare",
        "Passengers pay the exact fare",
        "No change at the cash desk",
    ),
    // Driving devices
    pick(
        DEVICES,
        "Keyboard and mouse",
        "drive_keys",
        "Driving keys",
        "",
        &[
            ("omsi", "Custom controls (Controls page)"),
            ("simple", "W A S D + arrows"),
            ("wasd", "W A S D only"),
            ("arrows", "Arrow keys only"),
        ],
    ),
    slide(
        DEVICES,
        "Keyboard and mouse",
        "mouse_sens",
        "Mouse steering sensitivity",
        "O switches the mouse steering",
        (0.1, 3.0, 0.05),
        pct_or_omsi,
        hundredths,
    ),
    toggle(
        DEVICES,
        "Keyboard and mouse",
        "mouse_smooth",
        "Smooth mouse steering",
        "Off: the wheel follows the cursor at once, as in OMSI",
    ),
    toggle(
        DEVICES,
        "Keyboard and mouse",
        "mouse_right_off",
        "A right click ends the mouse steering",
        "As in OMSI",
    ),
    toggle(
        DEVICES,
        "Wheel and pedals",
        "momentary_gears",
        "H-pattern shifter returns to neutral",
        "When the gear is released",
    ),
    slide(
        DEVICES,
        "Wheel and pedals",
        "wheel_range",
        "Wheel rotation",
        "The wheel's own",
        (180.0, 1800.0, 30.0),
        deg,
        whole,
    ),
    slide(
        DEVICES,
        "Wheel and pedals",
        "wheel_lock",
        "Full lock at",
        "A real bus: about two and a half turns",
        (0.0, 1800.0, 30.0),
        wheel_lock,
        lock_or_omsi,
    ),
    slide(
        DEVICES,
        "Wheel and pedals",
        "pad_steer_smooth",
        "Stick steering smoothing",
        "",
        (0.0, 300.0, 10.0),
        ms_or_off,
        whole,
    ),
    toggle(
        DEVICES,
        "Wheel and pedals",
        "pad_steer_linear",
        "Stick steers like a wheel",
        "A wheel seen as a gamepad",
    ),
    toggle(
        DEVICES,
        "Wheel and pedals",
        "arrows_switch_cams",
        "Arrow keys switch the cameras with a wheel",
        "No glance",
    ),
    slide(
        DEVICES,
        "Wheel and pedals",
        "pedal_throttle",
        "Throttle pedal strength",
        "",
        (0.5, 2.0, 0.05),
        pedal,
        hundredths,
    ),
    slide(
        DEVICES,
        "Wheel and pedals",
        "pedal_brake",
        "Brake pedal strength",
        "",
        (0.5, 2.0, 0.05),
        pedal,
        hundredths,
    ),
    toggle(
        DEVICES,
        "Force feedback",
        "ff_enabled",
        "Force feedback and vibration",
        "",
    ),
    toggle(
        DEVICES,
        "Force feedback",
        "ff_invert",
        "Invert force feedback by default",
        "A wheel with a saved direction keeps its own",
    ),
    slide(
        DEVICES,
        "Force feedback",
        "ff_road_vib",
        "Road texture vibration",
        "",
        (0.0, 4.0, 0.05),
        vibration,
        twentieths,
    ),
    slide(
        DEVICES,
        "Force feedback",
        "ff_engine_vib",
        "Engine vibration",
        "",
        (0.0, 4.0, 0.05),
        vibration,
        twentieths,
    ),
    slide(
        DEVICES,
        "Force feedback",
        "ff_fade",
        "Vibration fade-out",
        "",
        (0.0, 1.5, 0.05),
        fade,
        hundredths,
    ),
    slide(
        DEVICES,
        "Seat",
        "seat_y",
        "Seat forward / back",
        "Moves the driver's eye",
        (-0.6, 0.6, 0.01),
        cm,
        hundredths,
    ),
    slide(
        DEVICES,
        "Seat",
        "seat_z",
        "Seat up / down",
        "",
        (-0.6, 0.6, 0.01),
        cm,
        hundredths,
    ),
    slide(
        DEVICES,
        "Seat",
        "seat_x",
        "Seat right / left",
        "",
        (-0.6, 0.6, 0.01),
        cm,
        hundredths,
    ),
    slide(
        DEVICES,
        "Seat",
        "seat_pitch_deg",
        "Head pitch",
        "",
        (-45.0, 45.0, 1.0),
        signed_deg,
        whole,
    ),
    // interface: the launcher's own, at once (the accent also colours the game's menus)
    now(pick(
        INTERFACE,
        "Accent color",
        "ui_accent",
        "Accent color",
        "The launcher and every menu of the game",
        ACCENTS,
    )),
    now(pick(
        INTERFACE,
        "Layout",
        "ui_nav",
        "Navigation",
        "Top bar: the pages in a row",
        &[("sidebar", "Sidebar"), ("top", "Top bar")],
    )),
    now(pick(
        INTERFACE,
        "Layout",
        "ui_background",
        "Background",
        "Scene: a blurred picture behind",
        &[("plain", "Plain dark"), ("scene", "Scene")],
    )),
    now(slide(
        INTERFACE,
        "Layout",
        "ui_dimming",
        "Dimming",
        "How much the scene is darkened under the text",
        (30.0, 90.0, 1.0),
        whole_pct,
        whole_int,
    )),
    now(pick(
        INTERFACE,
        "Layout",
        "ui_corners",
        "Corners",
        "Square is flatter",
        &[("rounded", "Rounded"), ("square", "Square")],
    )),
];

/// The accent colours (`omsi_ui::theme::Accent`), the default first.
pub(crate) const ACCENTS: &[(&str, &str)] = &[
    ("Amber", "Amber"),
    ("Orange", "Orange"),
    ("Red", "Red"),
    ("Pink", "Pink"),
    ("Purple", "Purple"),
    ("Blue", "Blue"),
    ("Teal", "Teal"),
    ("Green", "Green"),
];

/// The settings and the start's choices the page changes, and whether it changed either.
pub struct Store<'a> {
    pub s: &'a mut Value,
    pub start: &'a mut Value,
    pub changed: bool,
    pub start_changed: bool,
    /// The original OMSI 2 is the engine: only what it has is shown.
    pub omsi2: bool,
    /// (in) What the graphics adapter is taken to hold for textures, in MB (0: not known).
    pub adapter_mb: i64,
    /// (in) The most a chosen texture budget may come to on it, in MB (0: not known).
    pub ceiling_mb: i64,
}

impl Store<'_> {
    pub fn get(&self, key: &str) -> &Value {
        match key.strip_prefix("start.") {
            Some(k) => self.start.get(k),
            None => self.s.get(key),
        }
        .unwrap_or(&Value::Null)
    }

    pub fn set(&mut self, key: &str, v: Value) {
        match key.strip_prefix("start.") {
            Some(k) => {
                self.start[k] = v;
                self.start_changed = true;
            }
            None => {
                self.s[key] = v;
                self.changed = true;
            }
        }
    }

    pub(crate) fn on(&self, key: &str) -> bool {
        self.get(key).as_bool().unwrap_or(false)
    }
}

/// The start's choices this page shows, as values.
pub fn start_values(c: &Choice) -> Value {
    json!({"season": c.season, "autostart": c.autostart, "traffic": c.traffic, "passengers": c.passengers, "schedule": c.schedule})
}

/// The start's choices back from `start_values`.
pub fn apply_start(c: &mut Choice, v: &Value) {
    c.season = v["season"].as_str().unwrap_or("auto").to_string();
    c.autostart = v["autostart"].as_bool().unwrap_or(false);
    c.traffic = v["traffic"].as_f64().unwrap_or(30.0) as f32;
    c.passengers = v["passengers"].as_bool().unwrap_or(true);
    c.schedule = v["schedule"].as_bool().unwrap_or(true);
}

thread_local! {
    /// Every option's default: the settings as they come, the start's choices as they come.
    static DEFAULTS: (Value, Value) = (core::settings_from_text(None), start_values(&Choice::default()));
}

pub fn default_of(key: &str) -> Option<Value> {
    DEFAULTS.with(|d| match key.strip_prefix("start.") {
        Some(k) => d.1.get(k).cloned(),
        None => d.0.get(key).cloned(),
    })
}

/// The default of `key` where the page now is: the catalog's in OMSI 2 mode for the options
/// OMSI 2 has, the Depot's otherwise.
pub(crate) fn default_in(st: &Store, key: &str) -> Option<Value> {
    if st.omsi2 {
        if let Some(d) = crate::omsi2_map::default_of(key) {
            return Some(d);
        }
    }
    default_of(key)
}

pub(crate) fn value_text(v: &Value) -> String {
    match v {
        Value::String(x) => x.clone(),
        Value::Bool(b) => (*b as u8).to_string(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

pub(crate) fn same(a: &Value, b: &Value) -> bool {
    let (a, b) = (value_text(a), value_text(b));
    a == b
        || a.parse::<f64>()
            .ok()
            .zip(b.parse::<f64>().ok())
            .is_some_and(|(x, y)| (x - y).abs() < 1e-6)
}

/// The option differs from how it comes (an option with no default, or the preset, never does).
pub fn changed(o: &Opt, st: &Store) -> bool {
    if matches!(o.kind, Kind::Preset) {
        return false;
    }
    let v = st.get(o.key);
    !v.is_null() && default_in(st, o.key).is_some_and(|d| !same(v, &d))
}

/// The limit the chosen graphics API puts on the option `key` (see `api_limits`).
pub(crate) fn api_limit(s: &Value, key: &str) -> Option<&'static crate::api_limits::ApiLimit> {
    if d3d9_fixes_vanilla(s) {
        crate::api_limits::d3d9(key)
    } else {
        None
    }
}

/// How an option shows with the settings as they are.
#[derive(PartialEq, Debug)]
pub enum Shown {
    Row,
    /// Fixed by the graphics API: this text (its value and why).
    Locked(String),
    Hidden,
}

pub fn shown(o: &Opt, st: &Store) -> Shown {
    if st.omsi2 && !crate::omsi2_map::setting_in_omsi2(o.key, o.sec == INTERFACE) {
        return Shown::Hidden;
    }
    // (the Depot's graphics API limits are not OMSI 2's)
    let depot_api = !st.omsi2;
    if let Some(l) = api_limit(st.s, o.key).filter(|l| depot_api && !l.is_row()) {
        return match l.text() {
            Some(t) => Shown::Locked(t),
            None => Shown::Hidden,
        };
    }
    let fixed = depot_api && d3d9_fixes_vanilla(st.s);
    let graphics = st.get("graphics").as_str().unwrap_or("");
    // (Vanilla draws what OMSI 2 draws: no sun shadows, ambient occlusion or detail grain;
    // Enhanced+ traces its shadows, occlusion and reflections: always on there)
    let classic = fixed || graphics == "vanilla";
    let traced = !fixed && graphics == "enhanced_plus";
    let triple = st.on("triple_screen") && !st.on("vr");
    let visible = match o.key {
        "graphics_api" => !cfg!(target_os = "macos"),
        "shadow_size" | "shadow_casters" | "detail_textures" | "night_brightness" | "led_glow"
        | "led_mips" => !classic,
        "ssao" | "shadows" => !classic && !traced,
        "reflections" => !traced,
        "fov" => !triple,
        "triple_fov_deg" => triple,
        // (the falling snow DirectX9 draws on request: the other APIs draw their own)
        "dx9_snow" => fixed,
        "fewer_night_lights" => fixed,
        // (the picture DirectX9 draws smaller is brought up: only when a scale below 100 % is chosen)
        "dx9_upscale" => {
            let scale = st.get("render_scale");
            let scale = scale
                .as_str()
                .and_then(|t| t.trim().parse::<f64>().ok())
                .or_else(|| scale.as_f64());
            fixed && scale.is_some_and(|x| x > 0.0 && x < 0.995)
        }
        // (and our own ways to ease its work: DirectX9 draws what they leave out)
        "dx9_glass_rain" | "dx9_coronas" | "dx9_stars" | "dx9_cloud_picture"
        | "dx9_texture_limit" => fixed,
        "vr" => cfg!(windows),
        k if k.starts_with("vr_") => cfg!(windows) && st.on("vr"),
        k if k.starts_with("triple_") && k != "triple_screen" => st.on("triple_screen"),
        k if k.starts_with("head_tracking_") => st.on("head_tracking"),
        k if k.starts_with("info_item_") => st.get("info_bar_mode").as_str() == Some("custom"),
        "info_bar_collapsed" | "info_bar_labels" | "info_bar_lowfuel" => {
            matches!(st.get("info_bar_mode").as_str(), Some("twotier" | "split"))
        }
        "ui_dimming" => st.get("ui_background").as_str() == Some("scene"),
        _ => true,
    };
    if visible {
        Shown::Row
    } else {
        Shown::Hidden
    }
}

/// The quality presets: they set most of the graphics options. (OMSI's own option_presets/*.oop
/// are named after the PCs of their day - "PC 2006", "X10 high" - and are on Presets and files.)
pub(crate) fn presets() -> [(&'static str, Value); 4] {
    [
        (
            "Low",
            json!({"msaa": 1, "anisotropy": 2, "shadow_size": 1024, "ssao": false, "shadows": false, "detail_textures": false, "clouds": false, "view_distance": "600", "min_obj_size": 0.03, "max_obj_dist": "500", "mirror_size": 128, "mirror_refresh": "eco", "render_scale": "0.75", "texture_memory": 800}),
        ),
        (
            "Medium",
            json!({"msaa": 2, "anisotropy": 4, "shadow_size": 2048, "ssao": false, "shadows": true, "detail_textures": true, "clouds": true, "view_distance": "900", "min_obj_size": 0.02, "max_obj_dist": "750", "mirror_size": 256, "mirror_refresh": "eco", "render_scale": "auto", "texture_memory": 1200}),
        ),
        (
            "High",
            json!({"msaa": 4, "anisotropy": 8, "shadow_size": 2048, "ssao": true, "shadows": true, "detail_textures": true, "clouds": true, "view_distance": "auto", "min_obj_size": 0.013, "max_obj_dist": "auto", "mirror_size": 256, "mirror_refresh": "full", "render_scale": "auto", "texture_memory": 0}),
        ),
        (
            "Ultra",
            json!({"msaa": 4, "anisotropy": 8, "shadow_size": 4096, "ssao": true, "shadows": true, "detail_textures": true, "clouds": true, "view_distance": "2000", "min_obj_size": 0.005, "max_obj_dist": "1500", "mirror_size": 512, "mirror_refresh": "full", "render_scale": "auto", "texture_memory": 0}),
        ),
    ]
}

pub(crate) fn api_options() -> &'static [(&'static str, &'static str)] {
    // (a Mac has Metal only; elsewhere a driver's Vulkan that misbehaves, or a card without
    // it, is got round here; "DirectX9 (D3D9Ex)" is the game window's own Direct3D 9Ex
    // renderer, Windows only and never chosen by "Automatic")
    if cfg!(windows) {
        &[
            ("auto", "Automatic"),
            ("vulkan", "Vulkan"),
            ("dx12", "DirectX 12"),
            ("gl", "OpenGL"),
            ("d3d9", "DirectX9 (D3D9Ex)"),
        ]
    } else {
        &[
            ("auto", "Automatic"),
            ("vulkan", "Vulkan"),
            ("gl", "OpenGL"),
        ]
    }
}

pub(crate) fn mb(v: i64) -> String {
    if v >= 1000 {
        format!("{:.1} GB", v as f64 / 1000.0)
    } else {
        format!("{v} MB")
    }
}

/// The choices of a dropdown option (the texture memory's "Automatic" says what it comes to).
pub fn options_of(o: &Opt, st: &Store) -> Vec<(String, String)> {
    let own = |list: &[(&str, &str)]| {
        list.iter()
            .map(|(v, t)| (v.to_string(), t.to_string()))
            .collect()
    };
    match &o.kind {
        Kind::Pick(_) if st.omsi2 && o.key == "boarding" => own(&crate::omsi2_map::TICKET_SELLING),
        Kind::Pick(list) => {
            // (under DirectX9 an option may leave some choices out, see `api_limits`)
            let limit = api_limit(st.s, o.key).filter(|_| !st.omsi2);
            list.iter()
                .filter(|(v, _)| limit.is_none_or(|l| l.allows(v)))
                .filter(|(v, _)| !st.omsi2 || crate::omsi2_map::pick_allowed(o.key, v))
                .map(|(v, t)| (v.to_string(), t.to_string()))
                .collect()
        }
        Kind::Api => own(api_options()),
        Kind::Language => own(&core::LANGUAGES
            .iter()
            .map(|l| (l.0, l.1))
            .collect::<Vec<_>>()),
        Kind::Preset => presets()
            .iter()
            .map(|p| (p.0.to_string(), p.0.to_string()))
            .chain([("custom".to_string(), "Custom".to_string())])
            .collect(),
        Kind::TexMem => {
            // (the game takes the smaller of an eighth of the memory and what the graphics
            // adapter is taken to hold, see `memory::texture_budget`)
            let adapter = st.adapter_mb;
            let auto = match (st.get("texture_memory_auto").as_i64().unwrap_or(0), adapter) {
                (m, 0) => m,
                (0, a) => a,
                (m, a) => m.min(a),
            };
            let auto = if auto > 0 {
                omsi_ui::tr("Automatic (%{size} here)").replace("%{size}", &mb(auto))
            } else {
                "Automatic".to_string()
            };
            let mut out = vec![("0".to_string(), auto)];
            out.extend([500, 1000, 1500, 2000, 3000, 4000, 6000].map(|m| (m.to_string(), mb(m))));
            out
        }
        Kind::Toggle | Kind::Slide { .. } => Vec::new(),
    }
}

/// Which preset the settings match now (`None`: Custom).
pub(crate) fn preset_now(st: &Store) -> Option<usize> {
    presets().iter().position(|p| {
        p.1.as_object()
            .is_some_and(|o| o.iter().all(|(k, v)| same(st.get(k), v)))
    })
}

/// A dropdown option's choice now: its index, a value written by hand getting an entry of its own.
pub fn chosen(o: &Opt, st: &Store, options: &mut Vec<(String, String)>) -> usize {
    if matches!(o.kind, Kind::Preset) {
        return preset_now(st).unwrap_or(options.len() - 1);
    }
    let mut cur = value_text(st.get(o.key));
    // (a saved value DirectX9 does not offer shows as the choice it takes, see `api_limits`)
    if let Some(l) = api_limit(st.s, o.key).filter(|_| !st.omsi2) {
        cur = l.fit(&cur);
    }
    // (anisotropy 0 is off, the same entry as 1)
    if o.key == "anisotropy" && cur == "0" {
        cur = "1".to_string();
    }
    match options.iter().position(|x| same(&json!(x.0), &json!(cur))) {
        Some(i) => i,
        None if !cur.is_empty() => {
            options.push((cur.clone(), cur));
            options.len() - 1
        }
        None => 0,
    }
}

/// Option `o` set to the dropdown's entry `value`, stored as the kind of value it had.
pub(crate) fn choose(o: &Opt, st: &mut Store, value: &str) {
    if matches!(o.kind, Kind::Preset) {
        if let Some(p) = presets().into_iter().find(|p| p.0 == value) {
            for (k, v) in p.1.as_object().into_iter().flatten() {
                st.set(k, v.clone());
            }
        }
        return;
    }
    let like = if st.get(o.key).is_null() {
        default_in(st, o.key).unwrap_or(Value::Null)
    } else {
        st.get(o.key).clone()
    };
    let v = match like {
        Value::Bool(_) => json!(value == "1"),
        Value::Number(_) => value
            .parse::<f64>()
            .map(|f| {
                if f.fract() == 0.0 {
                    json!(f as i64)
                } else {
                    json!(f)
                }
            })
            .unwrap_or(json!(value)),
        _ => json!(value),
    };
    // (DirectX9 is for older hardware: choosing it brings the anti-aliasing and anisotropic
    // filtering down to 2x and 4x, which the player may raise again)
    if o.key == "graphics_api" && v == json!("d3d9") && cfg!(windows) {
        for (key, most) in [("msaa", 2), ("anisotropy", 4)] {
            if st.get(key).as_i64().is_some_and(|n| n > most) {
                st.set(key, json!(most));
            }
        }
    }
    st.set(o.key, v);
}

/// Option `o` back to how it comes.
pub fn reset(o: &Opt, st: &mut Store) {
    if let Some(d) = default_in(st, o.key) {
        st.set(o.key, d);
        after(o.key, st);
    }
}

/// What a change of `key` brings along.
pub(crate) fn after(key: &str, st: &mut Store) {
    match key {
        // (the field of view of the three screens follows from their distance)
        "triple_distance_mm" => st.s["triple_fov_deg"] = json!(0.0),
        "ui_accent" => omsi_ui::theme::set_accent(
            omsi_ui::theme::Accent::named(st.get("ui_accent").as_str().unwrap_or(""))
                .unwrap_or_default(),
        ),
        _ => {}
    }
}

/// The options a search or "changed only" lists, or a section's.
pub fn listed<'a>(sec: usize, query: &str, changed_only: bool, st: &Store) -> Vec<&'a Opt> {
    let q = query.trim().to_lowercase();
    if sec == PERFORMANCE && q.is_empty() && !changed_only {
        return PERFORMANCE_OPTS
            .iter()
            .filter_map(|(k, _)| OPTS.iter().find(|o| o.key == *k))
            .filter(|o| shown(o, st) != Shown::Hidden)
            .collect();
    }
    OPTS.iter()
        .filter(|o| {
            if q.is_empty() {
                changed_only || o.sec == sec
            } else {
                matches(o.label, &q)
                    || matches(o.hint, &q)
                    || matches(o.group, &q)
                    || matches(SECTIONS[o.sec], &q)
            }
        })
        .filter(|o| match shown(o, st) {
            Shown::Hidden => false,
            // (a row fixed by the graphics API cannot be taken back here: "changed" leaves it out)
            Shown::Locked(_) => !changed_only,
            Shown::Row => !changed_only || changed(o, st),
        })
        .collect()
}

/// What the page asks of the launcher: "Reset all settings", the Controls page at one of its tabs.
#[derive(Default)]
pub struct Outside {
    pub reset: bool,
    pub controls: Option<usize>,
    /// (in) A game runs: the OMSI 2 folder's plug-ins are not switched meanwhile.
    pub game_runs: bool,
    /// A module's install or removal (Modules, OMSI 2 mode).
    pub modules: Asks,
}
