//! The scene: its instances and their bounds, the render phases, the camera and the views'
//! geometry (the triple screen), and the logic behind the per-instance settings. Knows nothing
//! of a frame's passes.

mod bounds;
mod camera;
mod instances;
mod layouts;
mod model;
mod triple;
mod visibility;

pub use crate::assets::{MaterialId, MeshId, TextureId, Vertex};
pub use camera::Camera;
pub use model::{Instance, RenderPhase, Scene};
pub use triple::{panel_width, ScreenView, TripleScreen};

pub(crate) use bounds::InstanceBounds;
pub(crate) use bounds::{
    bounding_sphere, cull_blocks, instance_scale, mesh_bounds_changed, mesh_slot_replaced,
    point_in_vehicle_box, prepare_bounds, CULL_BLOCK,
};
#[cfg(test)]
pub(crate) use camera::camera_layout_entries;
pub(crate) use camera::{CameraUniform, ENHANCED_CAMERA_TEXTURES};
pub(crate) use instances::mark_changed;
pub(crate) use layouts::Layouts;
pub(crate) use visibility::CullHistory;
