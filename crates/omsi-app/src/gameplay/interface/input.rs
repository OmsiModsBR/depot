//! What the rest of the game reaches of the game play, by subject.

pub(crate) use super::super::domain::line_times;
pub(crate) use super::super::domain::traffic_density;
pub(crate) use super::super::implementation::bus_service;
pub(crate) use super::super::implementation::career;
pub(crate) use super::super::implementation::duty_start;
pub(crate) use super::super::implementation::journey;
pub(crate) use super::super::implementation::money;
pub(crate) use super::super::implementation::schedule_paper;
pub(crate) use super::super::implementation::traffic_link;

/// The people on foot: their types (`domain/humans.rs`) and their procedures
/// (`implementation/humans.rs`, with the passengers in `humans/pax.rs`).
pub(crate) mod humans {
    pub use crate::gameplay::domain::humans::*;
    pub use crate::gameplay::implementation::humans::*;

    /// The passengers: their types and their procedures.
    pub mod pax {
        pub use crate::gameplay::domain::humans::pax::*;
        // the procedures are only named from outside by the tests (`testing.rs`)
        #[cfg(feature = "test-support")]
        pub use crate::gameplay::implementation::humans::pax::*;
    }
}

/// The scheduled AI buses: their types (`domain/schedule.rs`) and their procedures
/// (`implementation/schedule.rs`).
pub(crate) mod schedule {
    pub use crate::gameplay::domain::schedule::*;
    pub use crate::gameplay::implementation::schedule::*;
}

/// The traffic: its types (`domain/traffic.rs`) and its procedures
/// (`implementation/traffic.rs`).
pub(crate) mod traffic {
    pub use crate::gameplay::domain::traffic::*;
    pub use crate::gameplay::implementation::traffic::*;
}

pub(crate) mod player {
    pub use crate::gameplay::domain::player::*;
    pub use crate::gameplay::implementation::player::*;
}

pub(crate) mod driver {
    pub use crate::gameplay::domain::driver::*;
    pub use crate::gameplay::implementation::driver::*;
}

pub(crate) mod on_foot {
    pub use crate::gameplay::domain::on_foot::*;
    // the walking paces are only named from outside by the tests (`testing.rs`)
    #[cfg(feature = "test-support")]
    pub use crate::gameplay::implementation::on_foot::*;
}

pub(crate) mod rail_drive {
    pub(crate) use crate::gameplay::domain::rail_drive::*;
    pub(crate) use crate::gameplay::implementation::rail_drive::*;
}

pub(crate) use super::super::implementation::spawn;

pub(crate) use super::super::implementation::teleport;
