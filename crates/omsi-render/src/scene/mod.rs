//! The scene: its instances and their bounds, the render phases, the camera and the views'
//! geometry (the triple screen), and the logic behind the per-instance settings. Knows nothing
//! of a frame's passes.

pub mod bounds;
pub mod camera;
pub mod instances;
pub mod layouts;
pub mod model;
pub mod triple;
pub mod visibility;

pub use crate::assets::{MaterialId, MeshId, TextureId, Vertex};
pub use camera::Camera;
pub use model::{Instance, RenderPhase, Scene};
pub use triple::{panel_width, ScreenView, TripleScreen};

pub use bounds::InstanceBounds;
pub use bounds::{
    bounding_sphere, cull_blocks, instance_scale, mesh_bounds_changed, mesh_slot_replaced,
    point_in_vehicle_box, prepare_bounds, CULL_BLOCK,
};
#[cfg(feature = "test-support")]
pub use camera::camera_layout_entries;
pub use camera::{CameraUniform, ENHANCED_CAMERA_TEXTURES};
pub use instances::mark_changed;
pub use layouts::Layouts;
pub use visibility::CullHistory;
