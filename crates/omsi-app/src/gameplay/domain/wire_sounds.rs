//! The recordings the player's trolleybus plays for its wire (see `omsi_trolley`'s
//! `WireSound`): read once from the convention's sound folder, kept for the session.

use std::sync::Arc;

/// The wire's recordings by name (`switch_left_sep`, `cross_int`, ...): None for one no
/// content root has (looked for once).
#[derive(Default)]
pub struct WireSounds {
    pub(crate) clips: hashbrown::HashMap<String, Option<Arc<omsi_audio::mixer::Clip>>>,
}
