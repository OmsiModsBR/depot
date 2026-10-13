//! A frame delivered to a triple screen: the three panels and the culling each keeps.

use crate::scene::CullHistory;

/// Swap in a panel's own culling history (and the last one's back out).
pub(crate) fn swap_triple_culling(culling: &mut CullHistory, panel: usize) {
    // Visibility and LOD hysteresis belong to one projection. A side panel
    // must not replace the previous centre view's history each frame.
    let history = &mut culling.triple_culling[panel];
    std::mem::swap(culling.cull_drawn.get_mut(), &mut history.drawn);
    std::mem::swap(culling.object_sizes.get_mut(), &mut history.sizes);
    std::mem::swap(culling.object_sizes_scratch.get_mut(), &mut history.scratch);
}
