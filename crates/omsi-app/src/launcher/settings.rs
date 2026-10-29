//! The Settings page (the Windows 11 concept): every option in a section on the left, in cards
//! of a few rows on the right, searchable; each row says what the option does and when a change
//! takes effect, marks a change from the default and takes it back.

use super::state::Choice;
use super::theme::*;
use super::ui::{id_of, matches, ButtonKind, Ui};
use super::{Launcher, Page};
use glam::Vec2;
use omsi_launcher_lib as core;
use omsi_launcher_ui::Page as UiPage;
use omsi_ui::paint::Align;
use omsi_ui::{Color, Rect, Weight};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::HashMap;

/// The page's sections, as the list on the left shows them.
pub const SECTIONS: [&str; 11] = [
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
];
pub const GENERAL: usize = 0;
const SIMULATION: usize = 1;
pub const GRAPHICS: usize = 2;
const ADVANCED: usize = 3;
const DISPLAY: usize = 4;
const SOUND: usize = 5;
pub const TRAFFIC: usize = 6;
pub const DEVICES: usize = 7;
const PLUGINS: usize = 8;
pub const PRESETS: usize = 9;
pub const INTERFACE: usize = 10;

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
    fn text(self) -> &'static str {
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

const fn toggle(
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

const fn pick(
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
const fn slide(
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
const fn slide_typed(
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
const fn slide_with(
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

const fn special(
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

const fn now(o: Opt) -> Opt {
    Opt {
        when: When::Now,
        ..o
    }
}

// (the sliders' texts and what they store)
fn pct(v: f32) -> String {
    format!("{:.0}%", v * 100.0)
}
fn pct_or_omsi(v: f32) -> String {
    if (v - 1.0).abs() < 0.01 {
        "OMSI".to_string()
    } else {
        pct(v)
    }
}
fn pct_or_off(v: f32) -> String {
    if v <= 0.0 {
        "Off".to_string()
    } else {
        pct(v)
    }
}
fn whole_pct_or_off(v: f32) -> String {
    if v <= 0.0 {
        "Off".to_string()
    } else {
        format!("{v:.0}%")
    }
}
fn mm(v: f32) -> String {
    format!("{v:.0} mm")
}
fn deg(v: f32) -> String {
    format!("{v:.0}°")
}
fn signed_deg(v: f32) -> String {
    format!("{v:+.0}°")
}
fn cm(v: f32) -> String {
    format!("{:+.0} cm", v * 100.0)
}
fn whole_pct(v: f32) -> String {
    format!("{v:.0} %")
}
fn ms_or_off(v: f32) -> String {
    if v <= 0.0 {
        "Off".to_string()
    } else {
        format!("{v:.0} ms")
    }
}
fn secs_as_ms(v: f32) -> String {
    format!("{:.0} ms", v * 1000.0)
}
fn fov(v: f32) -> String {
    if v < 20.0 {
        "Default".to_string()
    } else {
        deg(v)
    }
}
fn outside_fov(v: f32) -> String {
    if v < 20.0 {
        "Auto".to_string()
    } else {
        deg(v)
    }
}
fn night(v: f32) -> String {
    if v < 0.01 {
        "Off".to_string()
    } else {
        format!("+{v:.2}")
    }
}
fn led(v: f32) -> String {
    if v < 0.5 {
        "Off".to_string()
    } else {
        format!("{}", v as i64)
    }
}
fn led_mip(v: f32) -> String {
    if v < 0.005 {
        "Off".to_string()
    } else {
        format!("{v:.2}")
    }
}
fn wheel_lock(v: f32) -> String {
    if v < 45.0 {
        "OMSI".to_string()
    } else {
        deg(v)
    }
}
fn pedal(v: f32) -> String {
    if (v - 1.0).abs() < 0.01 {
        "Normal".to_string()
    } else if v < 1.0 {
        omsi_ui::tr("Softer x%{v}").replace("%{v}", &format!("{v:.2}"))
    } else {
        omsi_ui::tr("Stronger x%{v}").replace("%{v}", &format!("{v:.2}"))
    }
}
fn vibration(v: f32) -> String {
    if v < 0.01 {
        "Off".to_string()
    } else if (v - 1.0).abs() < 0.01 {
        "Normal".to_string()
    } else {
        pct(v)
    }
}
fn fade(v: f32) -> String {
    if v < 0.01 {
        "Off".to_string()
    } else {
        format!("{:.0} ms", (v * 1000.0).round() as i32)
    }
}
fn count(v: f32) -> String {
    format!("{v:.0}")
}
fn integer(v: f32) -> Value {
    json!(v.round() as i64)
}
fn percent_whole(v: f32) -> String {
    format!("{v:.0}%")
}
fn hundredths(v: f32) -> Value {
    json!((v * 100.0).round() / 100.0)
}
fn tenths(v: f32) -> Value {
    json!((v * 10.0).round() / 10.0)
}
fn twentieths(v: f32) -> Value {
    json!((v * 20.0).round() / 20.0)
}
fn whole(v: f32) -> Value {
    json!(v.round())
}
fn as_is(v: f32) -> Value {
    json!(v)
}
fn whole_or_default(v: f32) -> Value {
    json!(if v < 20.0 { 0.0 } else { v.round() })
}
fn lock_or_omsi(v: f32) -> Value {
    json!(if v < 45.0 { 0.0 } else { v.round() })
}
fn quarters(v: f32) -> Value {
    json!((v / 0.25).round() * 0.25)
}
fn whole_int(v: f32) -> Value {
    json!(v.round() as i64)
}
fn by_five_hundredths(v: f32) -> Value {
    json!((v / 0.05).round() * 0.05)
}
fn percent_axis(v: f32) -> Value {
    json!(v.round().clamp(0.0, 100.0))
}

const CORNERS: &[(&str, &str)] = &[
    ("top-left", "Top left"),
    ("top-right", "Top right"),
    ("bottom-left", "Bottom left"),
    ("bottom-right", "Bottom right"),
];
const INFO_BAR_MODES: &[(&str, &str)] = &[
    ("full", "Full bar"),
    ("compact", "Compact bar"),
    ("essential", "Essentials only"),
    ("custom", "Custom bar"),
];
const INFO_BAR_POSITIONS: &[(&str, &str)] = &[
    ("top", "Top of the screen"),
    ("bottom", "Bottom of the screen"),
];
const SEASONS: &[(&str, &str)] = &[
    ("auto", "By date"),
    ("spring", "Spring"),
    ("summer", "Summer"),
    ("autumn", "Autumn"),
    ("winter", "Winter"),
];

/// The window sizes the settings offer (`resolution`).
pub(crate) const RESOLUTIONS: &[(&str, &str)] = &[
    ("auto", "Automatic"),
    ("1280x720", "1280 x 720"),
    ("1280x800", "1280 x 800 (Steam Deck)"),
    ("1366x768", "1366 x 768"),
    ("1600x900", "1600 x 900"),
    ("1920x1080", "1920 x 1080"),
    ("1920x1200", "1920 x 1200"),
    ("2560x1440", "2560 x 1440"),
    ("3840x2160", "3840 x 2160"),
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
    // Simulation
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
    toggle(
        GRAPHICS,
        "Effects",
        "windy_trees",
        "Windy trees",
        "Leaves sway in the wind",
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
const ACCENTS: &[(&str, &str)] = &[
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

    fn on(&self, key: &str) -> bool {
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
fn default_in(st: &Store, key: &str) -> Option<Value> {
    if st.omsi2 {
        if let Some(d) = super::omsi2_map::default_of(key) {
            return Some(d);
        }
    }
    default_of(key)
}

fn value_text(v: &Value) -> String {
    match v {
        Value::String(x) => x.clone(),
        Value::Bool(b) => (*b as u8).to_string(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

fn same(a: &Value, b: &Value) -> bool {
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

/// Whether the settings choose "DirectX9 (D3D9Ex)" (Windows only), which draws as OMSI 2 does:
/// Graphics is Vanilla there, and fixed. (VR takes DirectX 12 instead, as the game does.)
pub(crate) fn d3d9_fixes_vanilla(s: &Value) -> bool {
    // (the environment read last: the page asks this for every option, every frame)
    cfg!(windows)
        && s.get("graphics_api").and_then(Value::as_str) == Some("d3d9")
        && !s.get("vr").and_then(Value::as_bool).unwrap_or(false)
        && !omsi_cfg::flags::OPENXR.is_set()
}

/// The limit the chosen graphics API puts on the option `key` (see `api_limits`).
fn api_limit(s: &Value, key: &str) -> Option<&'static crate::api_limits::ApiLimit> {
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
    if st.omsi2 && !super::omsi2_map::setting_in_omsi2(o.key, o.sec == INTERFACE) {
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
        "vr" => cfg!(windows),
        k if k.starts_with("vr_") => cfg!(windows) && st.on("vr"),
        k if k.starts_with("triple_") && k != "triple_screen" => st.on("triple_screen"),
        k if k.starts_with("head_tracking_") => st.on("head_tracking"),
        k if k.starts_with("info_item_") => st.get("info_bar_mode").as_str() == Some("custom"),
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
fn presets() -> [(&'static str, Value); 4] {
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

fn api_options() -> &'static [(&'static str, &'static str)] {
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

fn mb(v: i64) -> String {
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
        Kind::Pick(_) if st.omsi2 && o.key == "boarding" => own(&super::omsi2_map::TICKET_SELLING),
        Kind::Pick(list) => {
            // (under DirectX9 an option may leave some choices out, see `api_limits`)
            let limit = api_limit(st.s, o.key).filter(|_| !st.omsi2);
            list.iter()
                .filter(|(v, _)| limit.is_none_or(|l| l.allows(v)))
                .filter(|(v, _)| !st.omsi2 || super::omsi2_map::pick_allowed(o.key, v))
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
            let adapter =
                omsi_render::ADAPTER_TEXTURE_MB.load(std::sync::atomic::Ordering::Relaxed) as i64;
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
fn preset_now(st: &Store) -> Option<usize> {
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
fn choose(o: &Opt, st: &mut Store, value: &str) {
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
fn after(key: &str, st: &mut Store) {
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

const ROW_H: f32 = 50.0;
const HEAD_H: f32 = 44.0;

/// The page's cards: a group of options, or one of the page's own.
enum Card<'a> {
    Group(String, Vec<&'a Opt>),
    Api(&'a Opt),
    /// The accent colour as swatches (a search lists it as a dropdown row).
    Accent(&'a Opt),
    Own(&'static str),
}

impl Card<'_> {
    fn wide(&self) -> bool {
        matches!(self, Card::Api(_) | Card::Own("plugins"))
    }

    fn height(&self) -> f32 {
        match self {
            Card::Group(_, rows) => HEAD_H + rows.len() as f32 * ROW_H + 6.0,
            Card::Api(_) => 148.0,
            Card::Accent(_) => 312.0,
            Card::Own(name) => OWN_H.with(|h| h.borrow().get(name).copied().unwrap_or(200.0)),
        }
    }
}

thread_local! {
    /// How high each of the page's own cards came out the frame before.
    static OWN_H: RefCell<HashMap<&'static str, f32>> = RefCell::new(HashMap::new());
}

/// The cards of what is listed, in the order of the table (a search: "Section · Group").
fn cards<'a>(opts: &[&'a Opt], sec: usize, mixed: bool, omsi2: bool) -> Vec<Card<'a>> {
    let mut out: Vec<Card> = Vec::new();
    for o in opts {
        if !mixed && matches!(o.kind, Kind::Api) {
            out.push(Card::Api(o));
            continue;
        }
        if !mixed && o.key == "ui_accent" {
            out.push(Card::Accent(o));
            continue;
        }
        let title = if mixed {
            format!("{} · {}", SECTIONS[o.sec], o.group)
        } else {
            o.group.to_string()
        };
        match out
            .iter_mut()
            .find(|c| matches!(c, Card::Group(t, _) if *t == title))
        {
            Some(Card::Group(_, rows)) => rows.push(o),
            _ => out.push(Card::Group(title, vec![o])),
        }
    }
    if !mixed {
        let own: &[&str] = match sec {
            SOUND => &["radio"],
            DEVICES => &["controls"],
            PLUGINS => &["plugins"],
            PRESETS => &["oop", "profiles", "files"],
            INTERFACE => &["preview"],
            _ => &[],
        };
        // (the Depot's own radio stations, plug-ins and profiles are not OMSI 2's)
        out.extend(
            own.iter()
                .filter(|n| !omsi2 || !matches!(**n, "radio" | "plugins" | "profiles"))
                .map(|n| Card::Own(n)),
        );
    }
    out
}

/// What the page asks of the launcher: "Reset all settings", the Controls page at one of its tabs.
#[derive(Default)]
pub struct Outside {
    pub reset: bool,
    pub controls: Option<usize>,
}

pub fn settings(l: &mut Launcher, area: Rect) {
    let sec = l.pages.set.sec.min(SECTIONS.len() - 1);
    let (_, title, line) = UiPage::Settings.heading().unwrap_or_default();
    let searching = !l.pages.set.query.trim().is_empty();
    let place = if searching {
        "Search"
    } else if l.pages.set.changed_only {
        "Changed"
    } else {
        SECTIONS[sec]
    };
    let body = l.page_title_with(area, UiPage::Settings.title(), (place, title, line));
    let root = std::path::PathBuf::from(&l.state.config.root);
    let omsi2 = l.state.omsi2_engine();
    let mut start = start_values(&l.state.choice);
    let mut out = Outside::default();
    let narrow = body.w < 760.0;
    let view = &mut l.pages.set;
    let ui = &mut l.ui;
    // (in OMSI 2 mode the options OMSI 2 has are shown from, and kept in, its own file: the
    // page works on a copy of the settings with those laid over, taken apart again below)
    let data = omsi_launcher_lib::data_dir();
    let mut shown_settings = l.state.settings.clone();
    if omsi2 {
        let saved = view.omsi2_saved.get_or_insert_with(|| {
            omsi_launcher_lib::original_engine::config::load_settings_in(&data)
        });
        super::omsi2_map::overlay(&mut shown_settings, saved);
    } else {
        view.omsi2_saved = None;
    }
    let before = shown_settings.clone();
    let mut st = Store {
        s: &mut shown_settings,
        start: &mut start,
        changed: false,
        start_changed: false,
        omsi2,
    };
    // the bar: the search, all or the changed ones, how many are changed, the presets
    let bar_h = 34.0;
    // (each section's rows and whether one is changed, once a frame: the list and the chip use them)
    let mut per_sec = [(0usize, false); SECTIONS.len()];
    let mut n_changed = 0;
    for o in OPTS {
        if shown(o, &st) == Shown::Row {
            let c = changed(o, &st);
            per_sec[o.sec].0 += 1;
            per_sec[o.sec].1 |= c;
            n_changed += c as usize;
        } else if matches!(shown(o, &st), Shown::Locked(_)) {
            per_sec[o.sec].0 += 1;
        }
    }
    let sw = if narrow {
        body.w
    } else {
        (body.w * 0.4).min(520.0)
    };
    ui.text_input(
        "set-search",
        Rect::new(body.x, body.y, sw, bar_h),
        &mut view.query,
        "Search every option...",
        Some("search"),
    );
    let mut show = view.changed_only as usize;
    let (seg, mut y) = if narrow {
        (
            Rect::new(body.x, body.y + bar_h + 10.0, 200.0, bar_h),
            body.y + (bar_h + 10.0) * 2.0,
        )
    } else {
        (
            Rect::new(body.x + sw + 12.0, body.y, 200.0, bar_h),
            body.y + bar_h + 14.0,
        )
    };
    if ui.segmented("set-show", seg, &mut show, &["All", "Changed"]) {
        view.changed_only = show == 1;
    }
    if !narrow {
        let pw = ui.width("Presets", 13.0, Weight::Bold) + 48.0;
        if ui.button(
            "set-presets",
            Rect::new(body.right() - pw, body.y, pw, bar_h),
            "Presets",
            Some("inventory_2"),
            ButtonKind::Normal,
        ) {
            view.sec = PRESETS;
            view.query.clear();
            view.changed_only = false;
        }
        let chip = if n_changed == 1 {
            "1 changed from default".to_string()
        } else {
            omsi_ui::tr("%{n} changed from default").replace("%{n}", &n_changed.to_string())
        };
        let cw = ui.width(&chip, 12.0, Weight::Regular) + 24.0;
        let cr = Rect::new(body.right() - pw - 10.0 - cw, body.y + 4.0, cw, 26.0);
        ui.p().rounded(cr, 13.0, Color::WHITE.alpha(0.04));
        ui.p().rounded_border(cr, 13.0, 1.0, EDGE);
        ui.text_in(&chip, cr, 12.0, Weight::Regular, TEXT_DIM, Align::Center);
    }
    let q = view.query.clone();
    let mixed = !q.trim().is_empty() || view.changed_only;
    // the sections: a list on the left, a dropdown on a narrow window
    let content = if narrow {
        let mut s = sec;
        let names: Vec<String> = SECTIONS.iter().map(|s| s.to_string()).collect();
        if ui.select(
            "set-section",
            Rect::new(
                seg.right() + 10.0,
                seg.y,
                (body.right() - seg.right() - 10.0).max(120.0),
                bar_h,
            ),
            &mut s,
            &names,
        ) {
            view.sec = s;
            view.query.clear();
            view.changed_only = false;
        }
        Rect::new(body.x, y, body.w, (body.bottom() - y).max(0.0))
    } else {
        let lw = omsi_launcher_ui::kit::fit(230.0, body.w);
        let nav = Rect::new(body.x, y, lw, (body.bottom() - y).max(0.0));
        if let Some(s) = sections_card(ui, nav, sec, mixed, &per_sec) {
            view.sec = s;
            view.query.clear();
            view.changed_only = false;
        }
        y = nav.y;
        Rect::new(
            nav.right() + GAP,
            y,
            body.right() - nav.right() - GAP,
            nav.h,
        )
    };
    let sec = view.sec.min(SECTIONS.len() - 1);
    let opts = listed(sec, &q, view.changed_only, &st);
    section_body(
        ui, content, sec, mixed, narrow, &opts, &mut st, &mut out, &root,
    );
    // (the launcher speaks the chosen language at once)
    crate::ui_language(st.get("language").as_str().unwrap_or("ENG"));
    let (mut changed, start_changed) = (st.changed, st.start_changed);
    if changed && omsi2 {
        let saved = l.pages.set.omsi2_saved.get_or_insert_with(Default::default);
        let (kept, own) = super::omsi2_map::store_back(&before, &shown_settings, saved);
        if kept {
            if let Err(e) =
                omsi_launcher_lib::original_engine::config::save_settings_in(&data, saved)
            {
                l.state.set_status(
                    omsi_ui::tr("OMSI 2 mode settings were not saved: %{error}")
                        .replace("%{error}", &format!("{e:#}")),
                    true,
                );
            }
        }
        changed = !own.is_empty();
        for (k, v) in own {
            l.state.settings[k] = v;
        }
    } else if changed {
        l.state.settings = shown_settings;
    }
    if changed {
        l.state.settings_dirty = 0.3;
    }
    if start_changed {
        // (a season chosen moves the date into it, as it always did)
        let season = start["season"].as_str().unwrap_or("auto").to_string();
        let new_season = season != l.state.choice.season;
        apply_start(&mut l.state.choice, &start);
        if new_season {
            l.state.set_season(&season);
        }
        l.state.touched();
    }
    if out.reset {
        l.pages.confirm_reset = true;
    }
    if let Some(t) = out.controls {
        l.pages.controls_tab = t;
        l.go(Page::Controls);
    }
}

/// The section `sec` (or, `mixed`, what a search or "changed only" lists: `opts`) in `content`,
/// scrolled: its cards in two columns, one on a narrow window.
#[allow(clippy::too_many_arguments)]
pub fn section_body(
    ui: &mut Ui,
    content: Rect,
    sec: usize,
    mixed: bool,
    narrow: bool,
    opts: &[&Opt],
    st: &mut Store,
    out: &mut Outside,
    root: &std::path::Path,
) {
    let id = format!("set-page-{sec}-{mixed}");
    ui.scroll_area(&id, content, &mut |ui, v| {
        let w = v.w - 8.0;
        let list = cards(opts, sec, mixed, st.omsi2);
        if list.is_empty() {
            let text = if mixed {
                "Nothing matches. A search looks through every section."
            } else {
                "Nothing to set here with the options chosen now."
            };
            ui.text_in(
                text,
                Rect::new(v.x, v.y, w, 30.0),
                13.0,
                Weight::Regular,
                TEXT_DIM,
                Align::Left,
            );
            return 30.0;
        }
        // (a card goes under the shorter column; a wide one across both)
        let cols = if narrow { 1 } else { 2 };
        let cw = (w - GAP * (cols - 1) as f32) / cols as f32;
        let mut tops = vec![v.y; cols];
        for c in &list {
            let h = c.height();
            let r = if c.wide() || cols == 1 {
                let top = tops.iter().cloned().fold(v.y, f32::max);
                tops.iter_mut().for_each(|t| *t = top + h + GAP);
                Rect::new(v.x, top, w, h)
            } else {
                let k = if tops[1] < tops[0] { 1 } else { 0 };
                let r = Rect::new(v.x + k as f32 * (cw + GAP), tops[k], cw, h);
                tops[k] += h + GAP;
                r
            };
            match c {
                Card::Group(title, rows) => group_card(ui, r, title, rows, st),
                Card::Api(o) => api_card(ui, r, o, st),
                Card::Accent(o) => accent_card(ui, r, o, st),
                Card::Own(name) => {
                    let used = own_card(ui, r, name, st, out, root);
                    OWN_H.with(|m| m.borrow_mut().insert(name, used));
                }
            }
        }
        tops.iter().cloned().fold(v.y, f32::max) - v.y - GAP
    });
}

/// The sections on the left, with how many options each shows (a dot: one of them changed).
/// Returns a section clicked.
fn sections_card(
    ui: &mut Ui,
    r: Rect,
    sec: usize,
    mixed: bool,
    per_sec: &[(usize, bool)],
) -> Option<usize> {
    ui.panel(r);
    let mut picked = None;
    let x = r.x + 12.0;
    let w = r.w - 24.0;
    for (i, name) in SECTIONS.iter().enumerate() {
        let rr = Rect::new(x, r.y + 12.0 + i as f32 * 37.0, w, 33.0);
        let on = i == sec && !mixed;
        let (hover, _, clicked) = ui.interact_key(id_of(&format!("set-sec-{i}")), rr);
        if on {
            ui.p().rounded(rr, 5.0, accent().alpha(0.14));
            ui.p().rounded_border(rr, 5.0, 1.0, accent().alpha(0.6));
        } else if hover {
            ui.p().rounded(rr, 5.0, Color::WHITE.alpha(0.05));
        }
        let name = omsi_launcher_ui::kit::fit_text(ui, name, 13.5, rr.w - 60.0);
        ui.text_in(
            &name,
            Rect::new(rr.x + 10.0, rr.y, rr.w - 50.0, rr.h),
            13.5,
            Weight::Medium,
            TEXT,
            Align::Left,
        );
        let (n, any_changed) = per_sec[i];
        if n > 0 {
            ui.text_in(
                &n.to_string(),
                Rect::new(rr.right() - 44.0, rr.y, 34.0, rr.h),
                12.0,
                Weight::Regular,
                TEXT_DIM,
                Align::Right,
            );
            if any_changed {
                ui.p()
                    .circle(Vec2::new(rr.right() - 52.0, rr.center().y), 3.0, accent());
            }
        }
        if clicked {
            picked = Some(i);
        }
    }
    let note = "Keys and controllers are on the Controls page.";
    let nh = ui.paragraph_height(note, w, 12.0, Weight::Regular);
    let ny = r.bottom() - 14.0 - nh;
    if ny > r.y + 12.0 + SECTIONS.len() as f32 * 37.0 {
        ui.paragraph(note, Vec2::new(x, ny), w, 12.0, Weight::Regular, TEXT_DIM);
    }
    picked
}

/// A card's frame and title, its count at the right. Returns the top of its body.
fn card_frame(ui: &mut Ui, r: Rect, title: &str, count: Option<usize>) -> f32 {
    ui.panel(r);
    let tw = if count.is_some() {
        r.w - 80.0
    } else {
        r.w - 32.0
    };
    omsi_launcher_ui::kit::heading(ui, Rect::new(r.x + 16.0, r.y + 6.0, tw, 30.0), title);
    if let Some(n) = count {
        ui.text_in(
            &n.to_string(),
            Rect::new(r.right() - 56.0, r.y + 8.0, 40.0, 24.0),
            12.0,
            Weight::Regular,
            TEXT_DIM,
            Align::Right,
        );
    }
    r.y + HEAD_H
}

fn group_card(ui: &mut Ui, r: Rect, title: &str, rows: &[&Opt], st: &mut Store) {
    let mut y = card_frame(ui, r, title, Some(rows.len()));
    for o in rows {
        option_row(ui, Rect::new(r.x, y, r.w, ROW_H), o, st);
        y += ROW_H;
    }
}

/// One option: its name (a dot when changed), what it does and when it takes effect, its
/// control, and the way back to the default.
fn option_row(ui: &mut Ui, r: Rect, o: &Opt, st: &mut Store) {
    ui.p().rect(Rect::new(r.x, r.y, r.w, 1.0), EDGE);
    let locked = match shown(o, st) {
        Shown::Locked(t) => Some(t),
        _ => None,
    };
    let is_changed = locked.is_none() && changed(o, st);
    // (a switch takes little room: its name gets the rest)
    let cw = if matches!(o.kind, Kind::Toggle) && locked.is_none() {
        60.0
    } else {
        (r.w * 0.42).clamp(140.0, 210.0)
    };
    let ctrl = Rect::new(
        r.right() - 16.0 - 26.0 - cw,
        r.y + (r.h - 32.0) * 0.5,
        cw,
        32.0,
    );
    let room = ctrl.x - r.x - 16.0 - 12.0;
    let x = r.x + 16.0;
    let fade = if locked.is_some() { 0.5 } else { 1.0 };
    let label = omsi_launcher_ui::kit::fit_text(ui, o.label, 13.5, room - 12.0);
    ui.text_in(
        &label,
        Rect::new(x, r.y + 6.0, room, 20.0),
        13.5,
        Weight::Regular,
        TEXT.alpha(fade),
        Align::Left,
    );
    if is_changed {
        let lw = ui.width(&label, 13.5, Weight::Regular);
        ui.p()
            .circle(Vec2::new(x + lw + 9.0, r.y + 16.0), 3.0, accent());
    }
    // (the line under the name: what it does, then when a change takes effect - or why it is fixed)
    let (when, wc) = match &locked {
        Some(_) => ("Locked".to_string(), WARN),
        None => (
            omsi_ui::tr(o.when.text()).into_owned(),
            if o.when == When::Now { OK } else { TEXT_DIM },
        ),
    };
    let ww = ui.width(&when, 11.5, Weight::Regular);
    let hint = match &locked {
        Some(t) => t
            .split_once(" - ")
            .map(|x| x.1.to_string())
            .unwrap_or_default(),
        None => omsi_ui::tr(o.hint).into_owned(),
    };
    let mut wx = x;
    if !hint.is_empty() {
        let h =
            omsi_launcher_ui::kit::fit_text(ui, &format!("{hint} · "), 11.5, (room - ww).max(20.0));
        ui.text_in(
            &h,
            Rect::new(x, r.y + 26.0, room, 16.0),
            11.5,
            Weight::Regular,
            TEXT_DIM.alpha(fade),
            Align::Left,
        );
        wx += ui.width(&h, 11.5, Weight::Regular);
    }
    ui.text_in(
        &when,
        Rect::new(wx, r.y + 26.0, ww + 4.0, 16.0),
        11.5,
        Weight::Regular,
        wc,
        Align::Left,
    );
    if let Some(t) = &locked {
        let value = t.split_once(" - ").map(|x| x.0).unwrap_or(t);
        ui.p().rounded(ctrl, 4.0, FIELD.alpha(0.6));
        ui.p().rounded_border(ctrl, 4.0, 1.0, EDGE);
        ui.text_in(
            value,
            Rect::new(ctrl.x + 12.0, ctrl.y, ctrl.w - 24.0, ctrl.h),
            13.0,
            Weight::Regular,
            TEXT_DIM,
            Align::Left,
        );
        ui.icon(
            "lock",
            Vec2::new(r.right() - 16.0 - 9.0, r.center().y),
            15.0,
            TEXT_DIM,
        );
        return;
    }
    let id = format!("opt-{}", o.key);
    match &o.kind {
        Kind::Toggle => {
            let mut v = st.on(o.key);
            if ui.toggle(&id, ctrl, &mut v, "") {
                st.set(o.key, json!(v));
                after(o.key, st);
            }
        }
        Kind::Slide {
            min,
            max,
            step,
            show,
            keep,
            typed,
        } => {
            let or = default_in(st, o.key)
                .and_then(|d| d.as_f64())
                .unwrap_or(*min as f64);
            let mut v = st.get(o.key).as_f64().unwrap_or(or) as f32;
            let mut moved = false;
            if *typed {
                // (the slider's own value text gives way to a field over it)
                moved |= ui.slider(&id, ctrl, &mut v, *min, *max, *step, "", &|_| String::new());
                // (right of the slider's own click area, which ends 50 px before the control)
                let fr = Rect::new(ctrl.right() - 48.0, ctrl.y + 2.0, 48.0, ctrl.h - 4.0);
                let name = format!("{id}-typed");
                let fid = omsi_ui::widgets::id_of(&name);
                let mut text = TYPED.with(|t| {
                    let t = t.borrow();
                    if ui.focus == Some(fid) && t.0 == fid {
                        t.1.clone()
                    } else {
                        show(v)
                    }
                });
                if ui.text_input(&name, fr, &mut text, "", None) {
                    TYPED.with(|t| *t.borrow_mut() = (fid, text.clone()));
                    let digits = text.trim().trim_end_matches('%').trim();
                    if let Ok(x) = digits.parse::<f32>() {
                        let x = if show(0.0).ends_with('%') && *max <= 5.0 {
                            x / 100.0
                        } else {
                            x
                        };
                        v = ((x / step.max(1e-6)).round() * step).clamp(*min, *max);
                        moved = true;
                    }
                }
            } else {
                moved |= ui.slider(&id, ctrl, &mut v, *min, *max, *step, "", &|x| show(x));
            }
            if moved {
                st.set(o.key, keep(v));
                after(o.key, st);
            }
        }
        _ => {
            let mut options = options_of(o, st);
            let mut sel = chosen(o, st, &mut options);
            let labels: Vec<String> = options.iter().map(|x| x.1.clone()).collect();
            if ui.select(&id, ctrl, &mut sel, &labels) {
                let value = options[sel].0.clone();
                choose(o, st, &value);
                after(o.key, st);
            }
        }
    }
    if is_changed
        && ui.icon_button(
            &format!("opt-reset-{}", o.key),
            Vec2::new(r.right() - 16.0 - 9.0, r.center().y),
            12.0,
            "restart_alt",
            "Back to the default",
        )
    {
        reset(o, st);
    }
}

/// The graphics API first, as a card of its own across the page: it decides what the other
/// graphics options can do.
thread_local! {
    /// The text being typed in a slider's field: which field, and what it holds.
    static TYPED: std::cell::RefCell<(omsi_ui::widgets::Id, String)> =
        const { std::cell::RefCell::new((0, String::new())) };
}

fn api_card(ui: &mut Ui, r: Rect, o: &Opt, st: &mut Store) {
    ui.panel(r);
    ui.p().rounded(r, RADIUS, accent().alpha(0.08));
    ui.p().rounded_border(r, RADIUS, 1.0, accent().alpha(0.55));
    let top = card_frame_plain(
        ui,
        r,
        "Graphics API",
        "decides what the other graphics options can do",
    );
    let fw = (r.w * 0.36).clamp(220.0, 320.0);
    let field = Rect::new(r.x + 16.0, top, fw, 40.0);
    let mut options = options_of(o, st);
    let mut sel = chosen(o, st, &mut options);
    let labels: Vec<String> = options.iter().map(|x| x.1.clone()).collect();
    if ui.select("opt-graphics_api", field, &mut sel, &labels) {
        let value = options[sel].0.clone();
        choose(o, st, &value);
    }
    let names = api_options()
        .iter()
        .map(|x| omsi_ui::tr(x.1).into_owned())
        .collect::<Vec<_>>()
        .join(" · ");
    let nh = ui.paragraph(
        &names,
        Vec2::new(field.x, field.bottom() + 8.0),
        fw,
        11.5,
        Weight::Regular,
        TEXT_DIM,
    );
    ui.text_in(
        "Next start",
        Rect::new(field.x, field.bottom() + 10.0 + nh, fw, 16.0),
        11.5,
        Weight::Regular,
        TEXT_DIM,
        Align::Left,
    );
    let tx = field.right() + 24.0;
    let text = if d3d9_fixes_vanilla(st.s) {
        "DirectX9 (D3D9Ex) is on. It is for compatibility and better performance on older hardware, and draws as close to the real OMSI 2 as it can: only the options OMSI 2 has, no improvements. What goes beyond OMSI 2 is hidden; what DirectX9 does not draw yet is shown locked. Your own choices come back when you switch API."
    } else {
        "The API you choose can limit other options. When one is not available with it, that option is locked and says why. Today only DirectX9 (D3D9Ex) limits options."
    };
    ui.paragraph(
        text,
        Vec2::new(tx, top + 2.0),
        (r.right() - 16.0 - tx).max(100.0),
        13.0,
        Weight::Regular,
        TEXT_SOFT,
    );
}

/// A card's title with a quiet note at its right. Returns the top of its body.
fn card_frame_plain(ui: &mut Ui, r: Rect, title: &str, note: &str) -> f32 {
    omsi_launcher_ui::kit::heading(ui, Rect::new(r.x + 16.0, r.y + 6.0, r.w * 0.5, 30.0), title);
    ui.text_in(
        note,
        Rect::new(r.x + r.w * 0.5, r.y + 8.0, r.w * 0.5 - 16.0, 24.0),
        12.0,
        Weight::Regular,
        TEXT_DIM,
        Align::Right,
    );
    r.y + HEAD_H
}

/// The page's own cards (radio stations, controls, plug-ins, presets and files). Returns the
/// height it needed.
fn own_card(
    ui: &mut Ui,
    r: Rect,
    name: &str,
    st: &mut Store,
    out: &mut Outside,
    root: &std::path::Path,
) -> f32 {
    match name {
        "radio" => radio_stations(ui, r),
        "controls" => {
            let mut y = card_frame(ui, r, "Keys and controllers", None);
            let x = r.x + 16.0;
            let w = r.w - 32.0;
            y += ui.paragraph(
                "Which key or button does what is set on the Controls page.",
                Vec2::new(x, y),
                w,
                12.5,
                Weight::Regular,
                TEXT_DIM,
            ) + 10.0;
            if ui.button(
                "s-go-keys",
                Rect::new(x, y, w, 34.0),
                "Change the keys",
                Some("keyboard"),
                ButtonKind::Normal,
            ) {
                out.controls = Some(0);
            }
            y += 42.0;
            if ui.button(
                "s-go-pads",
                Rect::new(x, y, w, 34.0),
                "Set up a wheel or pedals",
                Some("sports_esports"),
                ButtonKind::Normal,
            ) {
                out.controls = Some(1);
            }
            y + 34.0 + 16.0 - r.y
        }
        "plugins" => plugins_card(ui, r, root),
        "oop" => option_presets_card(ui, r, st, root),
        "preview" => preview_card(ui, r),
        "profiles" => profiles_card(ui, r, st),
        _ => files_card(ui, r, out),
    }
}

thread_local! {
    /// The radio stations as the page edits them (`radio.cfg`, read the first time).
    pub static RADIO: RefCell<Option<Vec<(String, String)>>> = const { RefCell::new(None) };
}

/// The bus radios' internet stations (`radio.cfg`, see `radio`): each with its name and
/// address, removed or added here, saved at once (#857). Returns the card's height.
fn radio_stations(ui: &mut Ui, r: Rect) -> f32 {
    let mut y = card_frame(ui, r, "Radio stations", None);
    let x = r.x + 16.0;
    let w = r.w - 32.0;
    y += ui.paragraph("A radio's station button n plays the n-th station, a cassette player the first; Shift+R steps through them. An address is an MP3, AAC or Ogg stream or an .m3u/.pls playlist.", Vec2::new(x, y), w, 12.5, Weight::Regular, TEXT_DIM) + 8.0;
    RADIO.with(|cell| {
        let mut cell = cell.borrow_mut();
        let list = cell.get_or_insert_with(crate::radio::own_stations);
        let mut changed = false;
        let mut remove = None;
        for (k, (name, address)) in list.iter_mut().enumerate() {
            let row = Rect::new(x, y, w, ROW - 2.0);
            let nw = (row.w * 0.3).round();
            changed |= ui.text_input(
                &format!("radio-name-{k}"),
                Rect::new(row.x, row.y, nw, row.h),
                name,
                "Name",
                None,
            );
            changed |= ui.text_input(
                &format!("radio-url-{k}"),
                Rect::new(row.x + nw + 8.0, row.y, row.w - nw - 8.0 - 36.0, row.h),
                address,
                "https://…",
                None,
            );
            if ui.icon_button(
                &format!("radio-del-{k}"),
                Vec2::new(row.right() - 16.0, row.center().y),
                14.0,
                "delete",
                "Remove this station",
            ) {
                remove = Some(k);
            }
            y += ROW + 4.0;
        }
        if let Some(k) = remove {
            list.remove(k);
            changed = true;
        }
        if ui.button(
            "radio-add",
            Rect::new(x, y, w, ROW - 2.0),
            "Add a station",
            Some("add"),
            ButtonKind::Normal,
        ) {
            list.push((String::new(), String::new()));
        }
        y += ROW + 4.0;
        if changed {
            if let Err(e) = crate::radio::save_stations(list) {
                log::warn!("radio.cfg: {e}");
            }
        }
    });
    y + 12.0 - r.y
}

thread_local! {
    /// The plug-ins of the OMSI 2 folder (`plugins/*.opl`), read once per folder.
    static PLUGINS_FOUND: RefCell<Option<(std::path::PathBuf, Vec<String>)>> = const { RefCell::new(None) };
}

/// The OMSI 2 folder's plug-ins: listed, read only (they are not loaded in this build).
fn plugins_card(ui: &mut Ui, r: Rect, root: &std::path::Path) -> f32 {
    let names = PLUGINS_FOUND.with(|c| {
        let mut c = c.borrow_mut();
        if c.as_ref().is_none_or(|x| x.0 != root) {
            let mut list: Vec<String> = std::fs::read_dir(root.join("plugins"))
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("opl")))
                .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
                .collect();
            list.sort_by_key(|n| n.to_lowercase());
            *c = Some((root.to_path_buf(), list));
        }
        c.as_ref().map(|x| x.1.clone()).unwrap_or_default()
    });
    let mut y = card_frame(ui, r, "Plug-ins found", Some(names.len()));
    let x = r.x + 16.0;
    let w = r.w - 32.0;
    if names.is_empty() {
        ui.text_in(
            "The OMSI 2 folder has no plug-ins.",
            Rect::new(x, y, w, 24.0),
            13.0,
            Weight::Regular,
            TEXT_DIM,
            Align::Left,
        );
        y += 30.0;
    }
    for n in &names {
        ui.p().rect(Rect::new(r.x, y, r.w, 1.0), EDGE);
        ui.text_in(
            n,
            Rect::new(x, y, w * 0.6, 34.0),
            13.0,
            Weight::Regular,
            TEXT,
            Align::Left,
        );
        ui.text_in(
            "Not loaded in this build",
            Rect::new(x + w * 0.6, y, w * 0.4, 34.0),
            12.5,
            Weight::Regular,
            TEXT_DIM,
            Align::Right,
        );
        y += 34.0;
    }
    y += 6.0;
    y += ui.paragraph("Plug-ins are listed so you can see what a bus or a map expects. They are not loaded in this build.", Vec2::new(x, y), w, 12.0, Weight::Regular, TEXT_DIM) + 14.0;
    y - r.y
}

thread_local! {
    /// OMSI 2's own option presets (`option_presets/*.oop`), read once for each OMSI 2 folder.
    static OOP: RefCell<Option<(std::path::PathBuf, Vec<(String, Value)>)>> = const { RefCell::new(None) };
}

/// OMSI 2's option presets: each applied with a click (only the options this game reads from them).
fn option_presets_card(ui: &mut Ui, r: Rect, st: &mut Store, root: &std::path::Path) -> f32 {
    // (read again for another OMSI 2 folder)
    let list = OOP.with(|c| {
        let mut c = c.borrow_mut();
        if c.as_ref().is_none_or(|(at, _)| at != root) {
            *c = Some((root.to_path_buf(), core::option_presets()));
        }
        c.as_ref().map(|x| x.1.clone()).unwrap_or_default()
    });
    let mut y = card_frame(ui, r, "OMSI 2's option presets", Some(list.len()));
    let x = r.x + 16.0;
    let w = r.w - 32.0;
    y += ui.paragraph("From the option_presets folder of the OMSI 2 folder. One changes only what this game reads from it: the frame limit, the object detail and distance, the anisotropic filtering, the texture memory and the mirrors.", Vec2::new(x, y), w, 12.0, Weight::Regular, TEXT_DIM) + 8.0;
    if list.is_empty() {
        ui.text_in(
            "No presets found.",
            Rect::new(x, y, w, 24.0),
            13.0,
            Weight::Regular,
            TEXT_DIM,
            Align::Left,
        );
        y += 30.0;
    }
    for (k, (name, values)) in list.iter().enumerate() {
        ui.text_in(
            name,
            Rect::new(x, y, w - 100.0, 34.0),
            13.0,
            Weight::Regular,
            TEXT,
            Align::Left,
        );
        if ui.button(
            &format!("oop-{k}"),
            Rect::new(r.right() - 16.0 - 90.0, y + 2.0, 90.0, 30.0),
            "Apply",
            None,
            ButtonKind::Normal,
        ) {
            for (key, v) in values.as_object().into_iter().flatten() {
                st.set(key, v.clone());
            }
        }
        y += 36.0;
    }
    y + 10.0 - r.y
}

/// The saved graphics profiles' card: the list, the name being typed.
#[derive(Default)]
struct GfxProfileUi {
    name: String,
    sel: usize,
    list: Option<Vec<String>>,
    msg: String,
}

thread_local! {
    static GFX_PROFILES: RefCell<GfxProfileUi> = RefCell::new(GfxProfileUi::default());
}

/// Save, load and delete the graphics settings as named profiles.
fn profiles_card(ui: &mut Ui, r: Rect, st: &mut Store) -> f32 {
    let mut y = card_frame(ui, r, "Graphics profiles", None);
    let x = r.x + 16.0;
    let w = r.w - 32.0;
    let row = |y: &mut f32| {
        let rr = Rect::new(x, *y, w, ROW - 2.0);
        *y += ROW + 4.0;
        rr
    };
    GFX_PROFILES.with(|g| {
        let mut g = g.borrow_mut();
        let g = &mut *g;
        let names: Vec<String> = g
            .list
            .get_or_insert_with(|| core::graphics_profiles().into_keys().collect())
            .clone();
        g.sel = g.sel.min(names.len().saturating_sub(1));
        let labels: Vec<String> = if names.is_empty() {
            vec!["No saved profiles".to_string()]
        } else {
            names.clone()
        };
        let r = row(&mut y);
        ui.label(Rect::new(r.x, r.y, r.w * 0.45, r.h), "Saved profile");
        if ui.select(
            "s-gp-sel",
            Rect::new(r.x + r.w * 0.45, r.y, r.w * 0.55, r.h),
            &mut g.sel,
            &labels,
        ) && !names.is_empty()
        {
            g.name = names[g.sel].clone();
        }
        let r = row(&mut y);
        let half = (r.w - GAP) * 0.5;
        if ui.button(
            "s-gp-load",
            Rect::new(r.x, r.y, half, r.h),
            "Load",
            Some("download"),
            ButtonKind::Normal,
        ) && !names.is_empty()
        {
            let name = names[g.sel].clone();
            match core::graphics_profiles().get(&name) {
                Some(p) => {
                    core::apply_graphics_profile(p, st.s);
                    st.changed = true;
                    g.msg = omsi_ui::tr("Loaded \"%{name}\".").replace("%{name}", &name);
                }
                None => g.msg = omsi_ui::tr("\"%{name}\" is gone.").replace("%{name}", &name),
            }
        }
        if ui.button(
            "s-gp-del",
            Rect::new(r.x + half + GAP, r.y, half, r.h),
            "Delete",
            Some("delete"),
            ButtonKind::Danger,
        ) && !names.is_empty()
        {
            let name = names[g.sel].clone();
            g.msg = match core::delete_graphics_profile(&name) {
                Ok(()) => omsi_ui::tr("Deleted \"%{name}\".").replace("%{name}", &name),
                Err(e) => format!("{e:#}"),
            };
            g.list = None;
        }
        let r = row(&mut y);
        ui.text_input("s-gp-name", r, &mut g.name, "Profile name", None);
        let r = row(&mut y);
        if ui.button(
            "s-gp-save",
            r,
            "Save current graphics as profile",
            Some("save"),
            ButtonKind::Primary,
        ) {
            g.msg = match core::save_graphics_profile(&g.name, st.s) {
                Ok(name) => {
                    g.name = name.clone();
                    g.list = None;
                    if let Some(i) = core::graphics_profiles().keys().position(|k| *k == name) {
                        g.sel = i;
                    }
                    omsi_ui::tr("Saved \"%{name}\".").replace("%{name}", &name)
                }
                Err(e) => format!("{e:#}"),
            };
        }
        if !g.msg.is_empty() {
            y += ui.paragraph(&g.msg, Vec2::new(x, y), w, 12.5, Weight::Regular, TEXT_DIM) + 8.0;
        }
    });
    y + 10.0 - r.y
}

/// Every setting back to how it came, the settings' folder, this build.
fn files_card(ui: &mut Ui, r: Rect, out: &mut Outside) -> f32 {
    let mut y = card_frame(ui, r, "Reset and files", None);
    let x = r.x + 16.0;
    let w = r.w - 32.0;
    if ui.button(
        "s-reset",
        Rect::new(x, y, w, 34.0),
        "Reset every setting...",
        Some("restart_alt"),
        ButtonKind::Danger,
    ) {
        out.reset = true;
    }
    y += 42.0;
    if ui.button(
        "s-folder",
        Rect::new(x, y, w, 34.0),
        "Open the settings folder",
        Some("folder_open"),
        ButtonKind::Normal,
    ) {
        core::open_folder(&core::data_dir());
    }
    y += 46.0;
    // this build's version and the project's release page (in-app updating is disabled, see
    // docs/removed/UPDATER_DISABLED.md; the link opens only on a click)
    ui.text_in(
        &omsi_ui::tr("This is Project Depot %{version}")
            .replace("%{version}", &crate::about::current_version().to_string()),
        Rect::new(x, y, w, 22.0),
        12.5,
        Weight::Regular,
        TEXT_DIM,
        Align::Left,
    );
    y += 26.0;
    if ui.button(
        "s-about-releases",
        Rect::new(x, y, w, 34.0),
        "github.com/OmsiModsBR/depot/releases",
        Some("open_in_new"),
        ButtonKind::Ghost,
    ) {
        crate::about::open_url(crate::about::RELEASES_URL);
    }
    y + 34.0 + 14.0 - r.y
}

/// The dialog that asks before every setting goes back to how it came.
pub fn reset_dialog(l: &mut Launcher) {
    let size = l.ui.size;
    let full = Rect::new(0.0, 0.0, size.x, size.y);
    l.ui.solid(full);
    l.ui.p().rect(full, super::start::SCRIM);
    let w = (size.x - 48.0).min(520.0);
    let h = 190.0;
    let r = Rect::new((size.x - w) * 0.5, (size.y - h) * 0.5, w, h);
    l.ui.panel(r);
    let inner = Rect::new(r.x + 24.0, r.y + 20.0, r.w - 48.0, r.h - 40.0);
    l.ui.icon(
        "restart_alt",
        Vec2::new(inner.x + 14.0, inner.y + 14.0),
        26.0,
        DANGER,
    );
    l.ui.text_in(
        "Reset every setting?",
        Rect::new(inner.x + 38.0, inner.y, inner.w - 38.0, 28.0),
        18.0,
        Weight::Bold,
        TEXT,
        Align::Left,
    );
    l.ui.paragraph("Every option on this page goes back to how it came, the start's choices (season, traffic, passengers) included. The language, the drivers, the key bindings and the game folder stay.", Vec2::new(inner.x, inner.y + 40.0), inner.w, 13.0, Weight::Regular, TEXT_DIM);
    let by = inner.bottom() - 38.0;
    if l.ui.button(
        "reset-no",
        Rect::new(inner.right() - 250.0, by, 110.0, 38.0),
        "Cancel",
        None,
        ButtonKind::Normal,
    ) {
        l.pages.confirm_reset = false;
    }
    if l.ui.button(
        "reset-yes",
        Rect::new(inner.right() - 130.0, by, 130.0, 38.0),
        "Reset",
        Some("restart_alt"),
        ButtonKind::Danger,
    ) {
        // (with OMSI 2 chosen the options on the page are its own: the Depot's stay as they are)
        if l.state.omsi2_engine() {
            let none = serde_json::Map::new();
            match omsi_launcher_lib::original_engine::config::save_settings_in(
                &omsi_launcher_lib::data_dir(),
                &none,
            ) {
                Ok(()) => {
                    l.pages.set.omsi2_saved = Some(none);
                    l.state
                        .set_status("Every setting is back to how it came.", false);
                }
                Err(e) => l.state.set_status(
                    omsi_ui::tr("OMSI 2 mode settings were not reset: %{error}")
                        .replace("%{error}", &format!("{e:#}")),
                    true,
                ),
            }
            l.pages.confirm_reset = false;
            return;
        }
        let language = l.state.settings.get("language").cloned();
        l.state.settings = core::settings_from_text(None);
        if let Some(lang) = language {
            l.state.settings["language"] = lang;
        }
        l.state.settings_dirty = 0.3;
        omsi_ui::theme::set_accent(omsi_ui::theme::Accent::default());
        // (the start's choices this page shows go back too, the season with what it brings)
        let season_before = l.state.choice.season.clone();
        apply_start(&mut l.state.choice, &start_values(&Choice::default()));
        let season = l.state.choice.season.clone();
        if season != season_before {
            l.state.set_season(&season);
        }
        l.state.touched();
        l.pages.confirm_reset = false;
        l.state
            .set_status("Every setting is back to how it came.", false);
    }
}

// --- interface ----------------------------------------------------------------------------------

/// The accent colour as swatches: the launcher and every menu of the game wear it. Its title
/// marks a change, as a row's name does, and the button under it takes it back.
fn accent_card(ui: &mut Ui, r: Rect, o: &Opt, st: &mut Store) {
    use omsi_ui::theme::Accent;
    let top = card_frame(ui, r, o.label, None);
    let x = r.x + 16.0;
    let iw = r.w - 32.0;
    if changed(o, st) {
        let tw = ui.width(&omsi_ui::tr(o.label).to_uppercase(), 12.0, Weight::Bold) * 1.12;
        ui.p()
            .circle(Vec2::new(x + tw + 10.0, r.y + 22.0), 3.0, accent());
    }
    ui.text_in(
        &omsi_ui::tr(When::Now.text()),
        Rect::new(r.right() - 116.0, r.y + 8.0, 100.0, 24.0),
        11.5,
        Weight::Regular,
        OK,
        Align::Right,
    );
    let chosen = Accent::named(st.get(o.key).as_str().unwrap_or("")).unwrap_or_default();
    let gap = 12.0;
    let w = (iw - gap * 3.0) / 4.0;
    for (i, a) in Accent::ALL.into_iter().enumerate() {
        let sx = x + (i % 4) as f32 * (w + gap);
        let sy = top + (i / 4) as f32 * 84.0;
        let sw = Rect::new(sx, sy, w, 52.0);
        let id = id_of(&format!("accent-{}", a.name().to_ascii_lowercase()));
        let (hover, _, clicked) = ui.interact_key(id, Rect::new(sx, sy, w, 76.0));
        let on = a == chosen;
        ui.p().rounded(
            sw,
            RADIUS,
            if hover && !on {
                a.color().lighten(0.08)
            } else {
                a.color()
            },
        );
        if on {
            ui.p()
                .rounded_border(sw.inset(-3.0), RADIUS + 3.0, 2.0, TEXT);
            ui.icon(
                "check_circle",
                sw.center(),
                22.0,
                omsi_ui::theme::on(a.color()),
            );
        }
        let name = if a == Accent::default() {
            omsi_ui::tr("%{name} (default)").replace("%{name}", &omsi_ui::tr(a.name()))
        } else {
            a.name().to_string()
        };
        ui.text_in(
            &name,
            Rect::new(sx, sy + 56.0, w, 20.0),
            12.0,
            Weight::Regular,
            if on { TEXT } else { TEXT_DIM },
            Align::Center,
        );
        if clicked && !on {
            st.set(o.key, json!(a.name()));
            after(o.key, st);
        }
    }
    let mut y = top + 84.0 * 2.0;
    y += ui.paragraph("A short fixed palette, chosen for contrast on the dark background. It applies to the launcher and to every menu of the game, the Alt bar included.", Vec2::new(x, y), iw, 12.0, Weight::Regular, TEXT_DIM) + 10.0;
    if ui.button(
        "accent-default",
        Rect::new(x, y, 160.0, 34.0),
        "Back to default",
        Some("restart_alt"),
        ButtonKind::Normal,
    ) && chosen != Accent::default()
    {
        reset(o, st);
    }
}

/// What wears the accent, drawn and not clickable: the page's own widgets are the real ones.
fn preview_card(ui: &mut Ui, r: Rect) -> f32 {
    let mut y = card_frame(ui, r, "Preview", None);
    let x = r.x + 16.0;
    let iw = r.w - 32.0;
    let nav = Rect::new(x, y, 200.0, 40.0);
    ui.p().rounded(nav, 4.0, HOVER);
    ui.p()
        .rounded(Rect::new(nav.x, nav.y + 11.0, 3.0, 18.0), 1.5, accent());
    ui.icon(
        "map",
        Vec2::new(nav.x + 24.0, nav.center().y),
        20.0,
        accent(),
    );
    ui.text_in(
        "Maps",
        Rect::new(nav.x + 44.0, nav.y, 140.0, nav.h),
        14.0,
        Weight::Medium,
        TEXT,
        Align::Left,
    );
    y += 52.0;
    let go = Rect::new(x, y, 130.0, 34.0);
    ui.p().rounded(go, 6.0, accent());
    ui.text_in(
        "Start driving",
        go,
        13.0,
        Weight::Bold,
        on_accent(),
        Align::Center,
    );
    let cancel = Rect::new(go.right() + 10.0, y, 90.0, 34.0);
    ui.p().rounded(cancel, 6.0, FIELD);
    ui.p().rounded_border(cancel, 6.0, 1.0, EDGE);
    ui.text_in("Cancel", cancel, 13.0, Weight::Medium, TEXT, Align::Center);
    for (k, on) in [(0.0, true), (1.0, false)] {
        let t = Rect::new(cancel.right() + 16.0 + k * 50.0, y + 8.0, 34.0, 18.0);
        ui.p().rounded(
            t,
            9.0,
            if on {
                accent()
            } else {
                Color::rgba(62, 62, 62, 1.0)
            },
        );
        ui.p().circle(
            Vec2::new(if on { t.right() - 9.0 } else { t.x + 9.0 }, t.center().y),
            6.0,
            Color::rgba(240, 240, 240, 1.0),
        );
    }
    y += 50.0;
    let row = Rect::new(x, y, (iw * 0.55).max(200.0), 48.0);
    ui.p().rounded(row, 6.0, omsi_ui::theme::accent_soft());
    ui.p().rounded_border(row, 6.0, 1.0, accent().alpha(0.55));
    ui.text_in(
        "Example bus A",
        Rect::new(row.x + 12.0, row.y + 5.0, row.w - 24.0, 20.0),
        13.5,
        Weight::Bold,
        TEXT,
        Align::Left,
    );
    ui.text_in(
        "Selected item",
        Rect::new(row.x + 12.0, row.y + 24.0, row.w - 24.0, 18.0),
        12.0,
        Weight::Regular,
        TEXT_DIM,
        Align::Left,
    );
    ui.text_in(
        "Link and highlight text",
        Rect::new(
            row.right() + 14.0,
            row.y,
            (x + iw - row.right() - 14.0).max(0.0),
            row.h,
        ),
        13.5,
        Weight::Regular,
        accent(),
        Align::Left,
    );
    y + 60.0 - r.y
}
