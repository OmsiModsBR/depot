//! Which graphics options each graphics mode offers, and when a change takes effect: the one
//! table the launcher and the game's own menu both ask (spec 001, `data-model.md`). An option
//! with no row here does not depend on the mode.
//!
//! The reasons are message keys (the English text the UI crates translate); nothing here is
//! translated.

use crate::GraphicsMode;

/// How one option shows in one graphics mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applies {
    /// Offered as it is.
    Offered,
    /// Offered with only these values to choose from.
    Choices {
        values: &'static [&'static str],
        reason: &'static str,
    },
    /// Shown as a fixed line: the mode draws with `value` (the settings file's spelling),
    /// written to the player as `shown`.
    Fixed {
        value: &'static str,
        shown: &'static str,
        reason: &'static str,
    },
    /// Not offered at all. The mode draws with `draws` (the file's spelling) where that is
    /// given, else the saved value is used as it is (it does nothing in this mode).
    NotOffered {
        draws: Option<&'static str>,
        reason: &'static str,
    },
}

/// When a change of the option takes effect in the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// In the running game, at once.
    Live,
    /// When the game starts the next time.
    NextStart,
}

/// One row of the table.
#[derive(Debug, Clone, Copy)]
pub struct GraphicsOption {
    /// The setting's key in the settings file (`preset` is the launcher's quality preset).
    pub key: &'static str,
    /// How it shows in Vanilla, Vanilla+ and Enhanced, in that order.
    pub per_mode: [Applies; 3],
    pub effect: Effect,
}

impl GraphicsOption {
    /// How the option shows in `mode`.
    pub fn applies(&self, mode: GraphicsMode) -> Applies {
        self.per_mode[index(mode)]
    }
}

fn index(mode: GraphicsMode) -> usize {
    match mode {
        GraphicsMode::Vanilla => 0,
        GraphicsMode::VanillaPlus => 1,
        GraphicsMode::Enhanced => 2,
    }
}

use Applies::Offered;
use Effect::{Live, NextStart};

const IMPROVEMENT: &str = "an improvement OMSI 2 does not have";
const NOT_DRAWN: &str = "not drawn by Vanilla yet";
const ENHANCED_ONLY: &str = "drawn by Enhanced only";
const VANILLA_ONLY: &str = "a choice of Vanilla's own renderer";
const VANILLA_API: &str = "Vanilla is drawn by Direct3D 9Ex";
const NO_RIG: &str = "Vanilla opens without VR and the triple screen";

const fn not(reason: &'static str) -> Applies {
    Applies::NotOffered {
        draws: None,
        reason,
    }
}

/// Not offered, and the mode draws with `draws` whatever is saved.
const fn held(draws: &'static str, reason: &'static str) -> Applies {
    Applies::NotOffered {
        draws: Some(draws),
        reason,
    }
}

/// Offered in every mode.
const EVERY: [Applies; 3] = [Offered, Offered, Offered];
/// Not in Vanilla, for `reason`; offered in the other two.
const fn modern(reason: &'static str) -> [Applies; 3] {
    [not(reason), Offered, Offered]
}
/// Not in Vanilla, which draws without it (off), for `reason`; offered in the other two.
const fn modern_off(reason: &'static str) -> [Applies; 3] {
    [held("false", reason), Offered, Offered]
}
/// Enhanced only (OMSI 2 has no such thing either).
const ENHANCED: [Applies; 3] = [not(IMPROVEMENT), not(ENHANCED_ONLY), Offered];
/// Vanilla only.
const VANILLA: [Applies; 3] = [Offered, not(VANILLA_ONLY), not(VANILLA_ONLY)];

const fn row(key: &'static str, per_mode: [Applies; 3], effect: Effect) -> GraphicsOption {
    GraphicsOption {
        key,
        per_mode,
        effect,
    }
}

/// Every graphics option whose offer depends on the mode: the Graphics and Advanced graphics
/// groups of the launcher, VR and the triple screen.
pub const GRAPHICS_OPTIONS: &[GraphicsOption] = &[
    // --- picture
    row("graphics", EVERY, NextStart),
    row("graphics_api", modern(VANILLA_API), NextStart),
    row("preset", EVERY, NextStart),
    row("msaa", EVERY, NextStart),
    row(
        "render_scale",
        [
            Applies::Choices {
                // (Auto is the window's own size: the scale is the player's choice there,
                // never automatic)
                values: &["auto", "1", "0.85", "0.75", "0.67", "0.5"],
                reason: "Vanilla draws at the window size unless a smaller scale is chosen",
            },
            Offered,
            Offered,
        ],
        NextStart,
    ),
    row("dx9_upscale", VANILLA, Live),
    row("anisotropy", EVERY, NextStart),
    // --- view
    row("view_distance", EVERY, NextStart),
    row("max_obj_dist", EVERY, NextStart),
    row("min_obj_size", EVERY, NextStart),
    // --- light and shadows (OMSI 2 has sun shadows: Vanilla does not draw them yet)
    row("shadows", modern_off(NOT_DRAWN), Live),
    row("shadow_size", modern(NOT_DRAWN), NextStart),
    row("shadow_casters", modern(NOT_DRAWN), NextStart),
    row("ssao", modern_off(IMPROVEMENT), Live),
    row(
        "shadow_blobs",
        [
            Applies::Fixed {
                value: "true",
                shown: "On",
                reason: "OMSI 2 always draws them",
            },
            Offered,
            Offered,
        ],
        NextStart,
    ),
    row(
        "night_brightness",
        [held("0", IMPROVEMENT), not(ENHANCED_ONLY), Offered],
        Live,
    ),
    // --- effects
    row("reflections", EVERY, NextStart),
    row(
        "mirror_size",
        [
            Applies::Choices {
                values: &["0", "128", "256", "512"],
                reason: "the sizes OMSI 2 offers",
            },
            Offered,
            Offered,
        ],
        NextStart,
    ),
    row(
        "mirror_refresh",
        [
            Applies::Choices {
                values: &["off", "full"],
                reason: "OMSI 2 has real-time reflections on or off",
            },
            Offered,
            Offered,
        ],
        Live,
    ),
    row(
        "clouds",
        [
            Applies::Fixed {
                value: "true",
                shown: "As OMSI 2",
                reason:
                    "the weather's own clouds, nothing added (no fair-weather clouds, no procedural ones)",
            },
            Offered,
            Offered,
        ],
        NextStart,
    ),
    row("sun_glow", EVERY, Live),
    row(
        "windy_trees",
        [
            Applies::Fixed {
                value: "false",
                shown: "Off",
                reason: NOT_DRAWN,
            },
            Offered,
            Offered,
        ],
        Live,
    ),
    row("smoke", EVERY, NextStart),
    row("smoke_user_only", EVERY, NextStart),
    row("smoke_max", EVERY, NextStart),
    row("smoke_in_mirrors", EVERY, NextStart),
    row("rain_reflections", VANILLA, Live),
    row("humans_in_rain_reflections", VANILLA, Live),
    row("fewer_night_lights", VANILLA, Live),
    row("dx9_glass_rain", VANILLA, Live),
    row("dx9_coronas", VANILLA, Live),
    row("dx9_stars", VANILLA, Live),
    row("dx9_cloud_picture", VANILLA, Live),
    // --- textures and LED displays
    row("texture_memory", EVERY, NextStart),
    row("dx9_texture_limit", VANILLA, NextStart),
    row("texture_compression", EVERY, NextStart),
    row("detail_textures", modern_off(IMPROVEMENT), Live),
    row("led_glow", ENHANCED, Live),
    row("led_mips", ENHANCED, Live),
    // --- VR headset
    row("vr", modern(NO_RIG), NextStart),
    row("vr_scale", modern(NO_RIG), NextStart),
    row("vr_head_smoothing_ms", modern(NO_RIG), NextStart),
    row("vr_mirror_rate", modern(NO_RIG), NextStart),
    row("vr_desktop_mirror", modern(NO_RIG), NextStart),
    // --- triple screen
    row("triple_screen", modern(NO_RIG), Live),
    row("triple_span", modern(NO_RIG), NextStart),
    row("triple_hud_center", modern(NO_RIG), Live),
    row("triple_width_mm", modern(NO_RIG), Live),
    row("triple_distance_mm", modern(NO_RIG), Live),
    row("triple_bezel_mm", modern(NO_RIG), Live),
    row("triple_left_angle_deg", modern(NO_RIG), Live),
    row("triple_right_angle_deg", modern(NO_RIG), Live),
    row("triple_eye_height_mm", modern(NO_RIG), Live),
    row("triple_fov_deg", modern(NO_RIG), Live),
];

/// The row of the option `key`, if its offer depends on the mode.
pub fn graphics_option(key: &str) -> Option<&'static GraphicsOption> {
    GRAPHICS_OPTIONS.iter().find(|o| o.key == key)
}

/// How the option `key` shows in `mode` (offered, when it has no row).
pub fn applies(key: &str, mode: GraphicsMode) -> Applies {
    graphics_option(key).map_or(Offered, |o| o.applies(mode))
}

/// Whether the launcher offers `key` in `mode`, as a row or as a fixed line.
pub fn offered_in_launcher(key: &str, mode: GraphicsMode) -> bool {
    !matches!(applies(key, mode), Applies::NotOffered { .. })
}

/// Whether the game's menu offers `key` in `mode`: offered in the launcher, and a change
/// takes effect at once.
pub fn offered_in_game(key: &str, mode: GraphicsMode) -> bool {
    offered_in_launcher(key, mode) && graphics_option(key).is_none_or(|o| o.effect == Live)
}

/// Whether `value` may be chosen for `key` in `mode` (only `Choices` leaves values out).
pub fn allows(key: &str, mode: GraphicsMode, value: &str) -> bool {
    match applies(key, mode) {
        Applies::Choices { values, .. } => values.contains(&value),
        _ => true,
    }
}

/// The value the game draws `key` with in `mode`, from the saved `value`: itself where the
/// option is offered, the fixed one of a fixed option, what the mode draws with for one not
/// offered (else the saved one, kept for another mode), and for `Choices` the nearest allowed number (never 0, which is off, for a positive
/// one), or the last choice when the value is not a number. The saved value is never
/// rewritten.
pub fn fit(key: &str, mode: GraphicsMode, value: &str) -> String {
    match applies(key, mode) {
        Applies::Offered | Applies::NotOffered { draws: None, .. } => value.to_string(),
        Applies::Fixed { value, .. }
        | Applies::NotOffered {
            draws: Some(value), ..
        } => value.to_string(),
        Applies::Choices { values, .. } => nearest(values, value),
    }
}

fn nearest(values: &[&str], value: &str) -> String {
    if values.contains(&value) {
        return value.to_string();
    }
    let near = value.parse::<f64>().ok().and_then(|v| {
        values
            .iter()
            .filter_map(|c| c.parse::<f64>().ok().map(|n| (c, n)))
            // (zero is "off": a value above it never rounds down to it)
            .filter(|&(_, n)| v <= 0.0 || n > 0.0)
            .map(|(c, n)| (c, (n - v).abs()))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(c, _)| c.to_string())
    });
    near.unwrap_or_else(|| values.last().map(|c| c.to_string()).unwrap_or_default())
}
