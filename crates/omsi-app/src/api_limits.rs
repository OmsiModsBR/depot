//! What a graphics API leaves out of the graphics options. "DirectX9 (D3D9Ex)" is the "same as
//! OMSI 2" renderer for older hardware: it offers no graphics improvement and no option it does
//! not draw yet. The launcher and the game's menu read this one table; the game applies the
//! locked values to the settings it loads (the saved file is not rewritten, so the choices come
//! back with another API).

/// How an option shows under the API.
pub(crate) enum Limit {
    /// Not offered at all.
    Hidden,
    /// Offered as a fixed line: this value, and why.
    Locked(&'static str),
    /// Offered as it is, with only these values to choose from.
    Choices(&'static [&'static str]),
}

pub(crate) struct ApiLimit {
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
pub(crate) const D3D9: &[ApiLimit] = &[
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
        key: "msaa",
        limit: Limit::Locked("Off"),
        reason: NOT_DRAWN,
    },
    ApiLimit {
        key: "render_scale",
        limit: Limit::Locked("Auto"),
        reason: "DirectX9 draws at the window size",
    },
    ApiLimit {
        key: "anisotropy",
        limit: Limit::Locked("Off"),
        reason: NOT_DRAWN,
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
            "the weather's own clouds, nothing added; the sky itself is not drawn by DirectX9 yet",
    },
    ApiLimit {
        key: "windy_trees",
        limit: Limit::Locked("Off"),
        reason: NOT_DRAWN,
    },
];

/// The limit "DirectX9 (D3D9Ex)" puts on the option `key`, if any.
pub(crate) fn d3d9(key: &str) -> Option<&'static ApiLimit> {
    D3D9.iter().find(|l| l.key == key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_option_is_in_the_table_once_with_a_reason() {
        for (i, l) in D3D9.iter().enumerate() {
            assert!(!l.reason.is_empty(), "{} has no reason", l.key);
            assert!(
                D3D9[i + 1..].iter().all(|o| o.key != l.key),
                "{} is listed twice",
                l.key
            );
            assert_eq!(l.text().is_some(), l.shown().is_some());
            assert!(!l.is_row() || l.text().is_none());
        }
        assert!(d3d9("msaa")
            .and_then(|l| l.text())
            .is_some_and(|t| t.starts_with("Off - ")));
        assert!(d3d9("ssao").is_some_and(|l| l.text().is_none()));
        assert!(d3d9("view_distance").is_none());
        let sizes = d3d9("mirror_size").unwrap();
        assert!(sizes.is_row() && sizes.allows("256") && !sizes.allows("1024"));
        let refresh = d3d9("mirror_refresh").unwrap();
        assert!(refresh.allows("full") && !refresh.allows("eco"));
    }
}
