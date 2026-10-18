//! Traffic path network and AI road vehicles (original units `mc_path`, `mc_pathrule`,
//! the AI part of `mc_roadvehicle`).
//!
//! Lanes come from `[path]` entries of splines (running along the spline at a lateral
//! offset) and of scenery objects (arcs in the object's frame). Lane ends are linked by
//! proximity, which reproduces the original's spline `prev`/`next` chains and the object
//! `[splinehelper]` connectors without needing their ids.
//!
//! - [`lane`]: a lane, its kind, map identity, geometry and rules.
//! - [`geometry`]: lanes from arcs, curves and polylines, and heading arithmetic.
//! - [`network`]: the lanes together: linking, conflicts, finding by place, routing.
//! - [`lights`]: traffic light controllers.
//! - [`driver`]: the AI driver ([`AiState`]).

mod driver;
mod geometry;
mod lane;
mod lights;
mod network;

pub use driver::{arrival_time, ramp_progress_for, AiState, LaneChange, Lead, MAX_BRAKE};
pub use geometry::{arc_point, wrap_deg, LaneBuilder};
pub use lane::{pool_density, Lane, LaneKey, LaneKind, AIR_NO_LIMIT_KMH, DEFAULT_PRIORITY};
pub use lights::{Aspect, LightStop, TrafficLightController, DEFAULT_APPROACH, UNLINKED_PHASE};
pub use network::{Crossing, Network, DEAD_END, GRID_CELL, MEET_DIST, REACH_MAX};
