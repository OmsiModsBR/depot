//! People on foot as a crowd: local avoidance between walkers and round vehicles, and
//! routes over a vehicle's cabin path network (`paths.cfg`).
//!
//! Avoidance is the anticipatory time-to-collision force of Karamouzas, Skinner and Guy
//! ("Universal power law governing pedestrian interactions", 2014): every walker looks at
//! when it would touch each neighbour if both kept their velocities and turns away from
//! that future contact the harder the sooner it comes. That is what makes two people give
//! way to each other a few metres early instead of bumping and sliding round each other
//! (which a plain repulsion does). A contact pass afterwards guarantees that nobody ends up
//! inside anybody else, and people standing still (waiting, queueing) are pushed less than
//! people walking - the one who walks goes round, the one who stands makes a little room.

mod corridor;
mod paths;
mod steering;

pub use corridor::{
    clamp_to_corridor, ease_into_corridor, heading_of, project_on_segment, turn_towards,
};
pub use paths::PathGraph;
pub use steering::{step, Block, CrowdParams, Walker};
