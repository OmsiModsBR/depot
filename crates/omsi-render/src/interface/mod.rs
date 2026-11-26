//! What the core and the render modes agree on: the data a picture is made of, handed to
//! another renderer as it is, and the steps a mode runs for a frame.

pub mod frame_plan;
pub mod mode;

pub use frame_plan::{FramePlan, PlanBatch, PlanKind, PlanSink};
pub use mode::{FogCull, FogLampsParts, LeadViewParts, ModeResources, PostParts, RenderMode};
pub use mode::{ModeShading, ModeView, PipeSet};
