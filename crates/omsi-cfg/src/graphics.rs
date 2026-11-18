//! The graphics mode: which of the three ways the game draws its picture. The game's setting
//! and the launcher both come through here, so they read the same values.

/// The graphics mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GraphicsMode {
    /// The picture as OMSI 2 draws it.
    Vanilla,
    /// OMSI 2's picture with conservative improvements; the default.
    #[default]
    VanillaPlus,
    /// The physically based picture.
    Enhanced,
}

/// What a saved value names.
enum Spelling {
    Known(GraphicsMode),
    /// Enhanced+, the mode that was removed: read as Enhanced.
    EnhancedPlus,
    Empty,
    Unknown,
}

fn spelling(text: &str) -> Spelling {
    match text
        .trim()
        .to_ascii_lowercase()
        .replace(['-', ' '], "_")
        .as_str()
    {
        "" => Spelling::Empty,
        "vanilla" | "classic" | "original" | "omsi" | "omsi2" | "omsi_2" => {
            Spelling::Known(GraphicsMode::Vanilla)
        }
        "vanilla_plus" | "vanilla+" | "vanillaplus" | "vanilla_+" => {
            Spelling::Known(GraphicsMode::VanillaPlus)
        }
        "enhanced" | "1" => Spelling::Known(GraphicsMode::Enhanced),
        "enhanced_plus" | "enhanced+" | "enhancedplus" | "enhanced_+" | "2" => {
            Spelling::EnhancedPlus
        }
        _ => Spelling::Unknown,
    }
}

impl GraphicsMode {
    /// Reads a saved value. Case, `-` and spaces do not matter. The removed Enhanced+ is
    /// Enhanced, with a line in the log (once per run). A missing or empty value is
    /// [`GraphicsMode::VanillaPlus`], and so is one that is not known, with a warning in the
    /// log (once per run).
    pub fn parse(text: &str) -> GraphicsMode {
        static ENHANCED_PLUS: std::sync::Once = std::sync::Once::new();
        static UNKNOWN: std::sync::Once = std::sync::Once::new();
        match spelling(text) {
            Spelling::Known(mode) => mode,
            Spelling::EnhancedPlus => {
                ENHANCED_PLUS.call_once(|| {
                    log::info!(
                        "settings: Enhanced+ graphics were removed; the setting is read as Enhanced"
                    )
                });
                GraphicsMode::Enhanced
            }
            Spelling::Empty => GraphicsMode::VanillaPlus,
            Spelling::Unknown => {
                UNKNOWN.call_once(|| {
                    log::warn!("graphics setting {text:?} is not known: using Vanilla+")
                });
                GraphicsMode::VanillaPlus
            }
        }
    }

    /// The spelling the setting is saved with.
    pub const fn as_str(self) -> &'static str {
        match self {
            GraphicsMode::Vanilla => "vanilla",
            GraphicsMode::VanillaPlus => "vanilla_plus",
            GraphicsMode::Enhanced => "enhanced",
        }
    }

    /// The name the launcher and the game's menu show.
    pub const fn label(self) -> &'static str {
        match self {
            GraphicsMode::Vanilla => "Vanilla (as OMSI 2)",
            GraphicsMode::VanillaPlus => "Vanilla+",
            GraphicsMode::Enhanced => "Enhanced",
        }
    }

    /// The three modes, in the order the lists show them.
    pub const ALL: [GraphicsMode; 3] = [
        GraphicsMode::Vanilla,
        GraphicsMode::VanillaPlus,
        GraphicsMode::Enhanced,
    ];

    /// The modes as a settings list offers them: the saved spelling and the name shown.
    pub const CHOICES: [(&'static str, &'static str); 3] = [
        (
            GraphicsMode::Vanilla.as_str(),
            GraphicsMode::Vanilla.label(),
        ),
        (
            GraphicsMode::VanillaPlus.as_str(),
            GraphicsMode::VanillaPlus.label(),
        ),
        (
            GraphicsMode::Enhanced.as_str(),
            GraphicsMode::Enhanced.label(),
        ),
    ];
}
