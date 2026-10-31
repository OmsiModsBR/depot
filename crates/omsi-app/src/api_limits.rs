//! What a graphics API leaves out of the graphics options. "DirectX9 (D3D9Ex)" is the "same as
//! OMSI 2" renderer for older hardware: it offers no graphics improvement and no option it does
//! not draw yet. The launcher and the game's menu read this one table; the game applies the
//! locked values to the settings it loads (the saved file is not rewritten, so the choices come
//! back with another API).

/// How an option shows under the API.
pub enum Limit {
    /// Not offered at all.
    Hidden,
    /// Offered as a fixed line: this value, and why.
    Locked(&'static str),
    /// Offered as it is, with only these values to choose from.
    Choices(&'static [&'static str]),
}

pub struct ApiLimit {
    /// The setting's key in the settings file.
    pub key: &'static str,
    pub limit: Limit,
    /// Why, as the player reads it.
    pub reason: &'static str,
}

impl ApiLimit {
    /// The fixed value of a locked option (`None` when hidden).
    pub fn shown(&self) -> Option<&'static str> {
        match self.limit {
            Limit::Hidden | Limit::Choices(_) => None,
            Limit::Locked(v) => Some(v),
        }
    }

    /// Whether the option is a row of its own, with or without fewer choices (not hidden,
    /// not a fixed line).
    pub fn is_row(&self) -> bool {
        matches!(self.limit, Limit::Choices(_))
    }

    /// `value` as this limit takes it: itself when allowed, else (for a `Choices` limit) the
    /// nearest allowed number (never 0 for a positive one), or the last choice when it is not a number.
    pub fn fit(&self, value: &str) -> String {
        let Limit::Choices(list) = self.limit else {
            return value.to_string();
        };
        if list.contains(&value) {
            return value.to_string();
        }
        let nearest = value.parse::<f64>().ok().and_then(|v| {
            list.iter()
                .filter_map(|c| c.parse::<f64>().ok().map(|n| (c, n)))
                // (zero is "off": a value above it never rounds down to it)
                .filter(|&(_, n)| v <= 0.0 || n > 0.0)
                .map(|(c, n)| (c, (n - v).abs()))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(c, _)| c.to_string())
        });
        nearest.unwrap_or_else(|| list.last().map(|c| c.to_string()).unwrap_or_default())
    }

    /// Whether `value` may be chosen (only a `Choices` limit leaves values out).
    pub fn allows(&self, value: &str) -> bool {
        match self.limit {
            Limit::Choices(list) => list.contains(&value),
            _ => true,
        }
    }

    /// The line of a locked option: its fixed value and the reason (`None` when hidden).
    pub fn text(&self) -> Option<String> {
        match self.limit {
            Limit::Hidden | Limit::Choices(_) => None,
            Limit::Locked(shown) => Some(format!("{shown} - {}", omsi_ui::tr(self.reason))),
        }
    }
}

const IMPROVEMENT: &str = "an improvement OMSI 2 does not have";
const NOT_DRAWN: &str = "not drawn by DirectX9 yet";

/// "DirectX9 (D3D9Ex)". `preset` is the launcher's and the menu's quality preset, not a key of
/// the file.
pub const D3D9: &[ApiLimit] = &[
    ApiLimit {
        key: "preset",
        limit: Limit::Hidden,
        reason: IMPROVEMENT,
    },
    ApiLimit {
        key: "graphics",
        limit: Limit::Locked("Vanilla (as OMSI 2)"),
        reason: "fixed with DirectX9 (D3D9Ex)",
    },
    ApiLimit {
        key: "shadows",
        limit: Limit::Hidden,
        reason: IMPROVEMENT,
    },
    ApiLimit {
        key: "shadow_size",
        limit: Limit::Hidden,
        reason: IMPROVEMENT,
    },
    ApiLimit {
        key: "shadow_casters",
        limit: Limit::Hidden,
        reason: IMPROVEMENT,
    },
    ApiLimit {
        key: "ssao",
        limit: Limit::Hidden,
        reason: IMPROVEMENT,
    },
    ApiLimit {
        key: "detail_textures",
        limit: Limit::Hidden,
        reason: IMPROVEMENT,
    },
    ApiLimit {
        key: "night_brightness",
        limit: Limit::Hidden,
        reason: IMPROVEMENT,
    },
    ApiLimit {
        key: "led_glow",
        limit: Limit::Hidden,
        reason: IMPROVEMENT,
    },
    ApiLimit {
        key: "led_mips",
        limit: Limit::Hidden,
        reason: IMPROVEMENT,
    },
    ApiLimit {
        key: "render_scale",
        limit: Limit::Locked("Auto"),
        reason: "DirectX9 draws at the window size",
    },
    // Mirrors are drawn (flat ones, as OMSI 2 has them): its picture sizes, and the real-time
    // reflections on or off. Economical and the biggest picture are ours.
    ApiLimit {
        key: "mirror_size",
        limit: Limit::Choices(&["0", "128", "256", "512"]),
        reason: "the sizes OMSI 2 offers",
    },
    ApiLimit {
        key: "mirror_refresh",
        limit: Limit::Choices(&["off", "full"]),
        reason: "OMSI 2 has real-time reflections on or off",
    },
    ApiLimit {
        key: "clouds",
        limit: Limit::Locked("As OMSI 2"),
        reason:
            "the weather's own clouds, nothing added (no fair-weather clouds, no procedural ones)",
    },
    ApiLimit {
        key: "windy_trees",
        limit: Limit::Locked("Off"),
        reason: NOT_DRAWN,
    },
];

/// The limit "DirectX9 (D3D9Ex)" puts on the option `key`, if any.
pub fn d3d9(key: &str) -> Option<&'static ApiLimit> {
    D3D9.iter().find(|l| l.key == key)
}
