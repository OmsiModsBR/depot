//! Collision world: oriented boxes (scenery objects, parked and moving vehicles) tested
//! against a vehicle's bounding box - separating axes in the ground plane plus a height
//! range, which is what a bus body against walls, posts and cars needs. The contact query
//! gives the depth, the direction to push the vehicle out and where the two touch, for the
//! impulse response in [`crate::physics`].

mod box3;
mod mesh;
mod obb;
mod polygon;
mod world;

pub use box3::Box3;
pub use mesh::{MeshObstacle, MeshShape};
pub use obb::{impact_height, Contact, Obb};
pub use world::CollisionWorld;

pub(crate) use mesh::PART_CELL;
