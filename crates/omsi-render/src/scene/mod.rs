//! The scene: its instances and their bounds, the render phases, the camera and the views'
//! geometry (the triple screen), and the logic behind the per-instance settings. Knows nothing
//! of a frame's passes.

mod bounds;
mod camera;
mod instances;
mod interior;
mod model;
mod settings;
mod triple;

pub use camera::{Camera, Vertex};
pub use model::{Instance, MaterialId, MeshId, RenderPhase, Scene, TextureId};
pub use triple::{panel_width, ScreenView, TripleScreen};

#[cfg(test)]
pub(crate) use bounds::InstanceBounds;
pub(crate) use bounds::{point_in_vehicle_box, CULL_BLOCK};
pub(crate) use camera::{
    camera_layout_entries, CameraUniform, ViewCulling, ENHANCED_CAMERA_TEXTURES,
};
