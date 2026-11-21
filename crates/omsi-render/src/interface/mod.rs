//! What the core and the render modes agree on: the data a picture is made of, handed to
//! another renderer as it is, and (with the mode trait) the steps a mode runs for a frame.

mod frame_plan;

pub use frame_plan::{FramePlan, PlanBatch, PlanKind, PlanSink};
