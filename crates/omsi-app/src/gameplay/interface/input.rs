//! What the rest of the game reaches of the game play, by subject.

pub(crate) use super::super::domain::traffic_density;
pub(crate) use super::super::implementation::traffic_link;

/// The traffic: its types (`domain/traffic.rs`) and its procedures
/// (`implementation/traffic.rs`).
pub(crate) mod traffic {
    pub use crate::gameplay::domain::traffic::*;
    pub use crate::gameplay::implementation::traffic::*;
}
