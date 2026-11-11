//! Which of the two vehicle physics engines a vehicle runs: OMSI Classic (the default) or
//! Project Depot Physics. The game's setting, the launcher and a vehicle's `.bus` file all
//! come through here, so they read the same values.

/// The vehicle physics engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PhysicsMode {
    /// OMSI 2's own rules; the default.
    #[default]
    Classic,
    /// Project Depot Physics.
    Depot,
}

/// Why a vehicle got its mode (for the session log).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// The game's setting.
    Setting,
    /// The `[depot_physics]` key in the vehicle's `.bus` file.
    BusKey,
}

/// Who drives a vehicle when it appears.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Driver {
    Player,
    Ai,
}

impl PhysicsMode {
    /// Reads a saved value. Case, `-`, `_` and spaces do not matter. A missing or empty value is
    /// [`PhysicsMode::Classic`], and so is one that is not known, with a warning in the log.
    pub fn parse(text: &str) -> PhysicsMode {
        let key: String = text
            .chars()
            .filter(|c| !matches!(c, '-' | '_') && !c.is_whitespace())
            .flat_map(char::to_lowercase)
            .collect();
        match key.as_str() {
            "" | "classic" | "omsi" | "omsi2" | "omsiclassic" => PhysicsMode::Classic,
            "depot" | "projectdepot" => PhysicsMode::Depot,
            _ => {
                log::warn!("physics setting {text:?} is not known: using OMSI Classic");
                PhysicsMode::Classic
            }
        }
    }

    /// The spelling the setting is saved with.
    pub fn as_str(self) -> &'static str {
        match self {
            PhysicsMode::Classic => "classic",
            PhysicsMode::Depot => "depot",
        }
    }

    /// The mode of one vehicle: Depot when the player drives a vehicle whose file has the
    /// `[depot_physics]` key (`depot_key`), whatever the setting `global` says; otherwise
    /// `global`. An AI vehicle never takes the key, and no key forces Classic.
    pub fn for_vehicle(
        global: PhysicsMode,
        depot_key: bool,
        driver: Driver,
    ) -> (PhysicsMode, Reason) {
        if depot_key && driver == Driver::Player {
            (PhysicsMode::Depot, Reason::BusKey)
        } else {
            (global, Reason::Setting)
        }
    }
}
