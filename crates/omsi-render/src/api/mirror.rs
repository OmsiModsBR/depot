//! The public methods for a second renderer: the asset mirror and the frame plan.
use crate::renderer::{AssetMirror, PlanSink, Renderer};
use crate::scene::Scene;

impl Renderer {
    /// Sends every mesh and texture of the scenes this renderer keeps to `mirror` as it goes
    /// up, in CPU form, from now on (None stops). Set it before the first asset is added:
    /// what went up before is not sent again (a mesh or texture prepared before it was set is
    /// added without being mirrored, with a warning in the log). While set, meshes and textures prepared on other
    /// threads keep a CPU copy until they are added.
    pub fn set_asset_mirror(&mut self, mirror: Option<AssetMirror>) {
        self.asset_mirror.set(mirror);
    }

    /// Hands the plan of each window picture to `sink`, after culling and before this
    /// renderer records its passes (None stops).
    pub fn set_plan_sink(&mut self, sink: Option<PlanSink>) {
        self.plan_sink = sink;
    }
}

impl Scene {
    /// Per-draw model matrices (column-major, relative to the render origin), indexed by the
    /// entries of [`crate::FramePlan::list`].
    pub fn slot_models(&self) -> &[[[f32; 4]; 4]] {
        &self.cpu_models
    }

    /// Per-draw parameters, two per entry (see [`crate::FramePlan`]).
    pub fn slot_params(&self) -> &[[f32; 4]] {
        &self.cpu_params
    }
}
