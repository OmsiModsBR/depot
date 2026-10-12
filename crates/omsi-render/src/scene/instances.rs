//! Instances: making a scene, adding, moving, removing and recycling instances.

use super::model::Scene;
use crate::renderer::Renderer;

impl Renderer {
    /// Note that instance `i` needs its per-draw entries rewritten.
    pub(crate) fn mark_changed(scene: &mut Scene, i: usize) {
        if scene.dirty || i >= scene.uploaded_instances {
            return;
        }
        if scene.changed_mark.len() < scene.instances.len() {
            scene.changed_mark.resize(scene.instances.len(), false);
        }
        if !scene.changed_mark[i] {
            scene.changed_mark[i] = true;
            scene.changed.push(i);
        }
    }
}
