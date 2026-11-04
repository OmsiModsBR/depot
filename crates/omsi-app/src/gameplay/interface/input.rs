//! What the rest of the game reaches of the game play, by subject.

pub(crate) use super::super::domain::traffic_density;
pub(crate) use super::super::implementation::traffic_link;

/// The people on foot: their types (`domain/humans.rs`) and their procedures
/// (`implementation/humans.rs`, with the passengers in `humans/pax.rs`).
pub(crate) mod humans {
    pub use crate::gameplay::domain::humans::*;
    pub use crate::gameplay::implementation::humans::*;

    /// The passengers: their types and their procedures.
    pub mod pax {
        pub use crate::gameplay::domain::humans::pax::*;
        pub use crate::gameplay::implementation::humans::pax::*;
    }
}

/// The traffic: its types (`domain/traffic.rs`) and its procedures
/// (`implementation/traffic.rs`).
pub(crate) mod traffic {
    pub use crate::gameplay::domain::traffic::*;
    pub use crate::gameplay::implementation::traffic::*;
}
