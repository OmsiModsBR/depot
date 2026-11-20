//! What the game calls.

pub use crate::domain::convention::{Convention, ConventionError, ConventionParams, RegistryEntry};
pub use crate::domain::declaration::{PoleGeometry, PoleParams, TrolleyDeclaration};
pub use crate::domain::network::{
    SegmentRef, SpanEnd, SpanId, TileWires, WireJoint, WireNetwork, WireObject, WireObjectKind,
    WireSide, WireSpan, JOIN_TOLERANCE,
};
pub use crate::domain::pole::{
    BodyPose, DewireReason, PoleEvent, PoleInputs, PoleOutput, PoleOutputs, PolePhase, PoleSim,
    PoleState, WireCursor,
};
pub use crate::domain::sound::{
    SoundParams, SparkParams, WireSound, WireSoundKind, WireWeather, SOUND_DIR,
};
pub use crate::implementation::convention::{
    load_convention, parse_params, parse_registry, PARAMS_FILE, REGISTRY_FILE,
};
pub use crate::implementation::declaration::{
    declaration_from, declaration_from_model, find_declaration, poles_in_model, same_vehicle_file,
};
pub use crate::implementation::network::build_tile_wires;
