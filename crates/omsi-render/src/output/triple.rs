//! A frame delivered to a triple screen: the three panels and the culling each keeps.

use crate::renderer::Renderer;

impl Renderer {
    pub(crate) fn swap_triple_culling(&mut self, panel: usize) {
        // Visibility and LOD hysteresis belong to one projection. A side panel
        // must not replace the previous centre view's history each frame.
        let history = &mut self.culling.triple_culling[panel];
        std::mem::swap(self.culling.cull_drawn.get_mut(), &mut history.drawn);
        std::mem::swap(self.culling.object_sizes.get_mut(), &mut history.sizes);
        std::mem::swap(
            self.culling.object_sizes_scratch.get_mut(),
            &mut history.scratch,
        );
    }
}
