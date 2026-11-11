//! What the game calls.

pub use crate::domain::convention::{Convention, ConventionError, ConventionParams, RegistryEntry};
pub use crate::domain::network::{
    SegmentRef, SpanEnd, SpanId, TileWires, WireJoint, WireNetwork, WireObject, WireObjectKind,
    WireSide, WireSpan, JOIN_TOLERANCE,
};
pub use crate::implementation::convention::{
    load_convention, parse_params, parse_registry, PARAMS_FILE, REGISTRY_FILE,
};
pub use crate::implementation::network::build_tile_wires;
