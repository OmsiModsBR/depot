//! Visibility history: what the views drew in their last frames.

use super::camera::ViewCulling;
use std::collections::HashMap;

/// What the views drew in their last frames, so that objects at a limit do not pop.
pub struct CullHistory {
    pub triple_culling: [ViewCulling; 3],
    /// `OMSI_DEBUG_FLICKER`: which near instances in view were drawn last frame.
    pub flicker: std::cell::RefCell<HashMap<usize, bool>>,
    /// Instances the main view drew last frame (a bit each): they keep being drawn a little
    /// past the size and distance limits, so that an object right at a limit does not pop
    /// in and out as the camera sways.
    pub cull_drawn: std::cell::RefCell<Vec<u64>>,
    /// The screen size each object (origin and radius) was judged by in the main view's last
    /// frame: all its meshes and LOD levels take the same one, so exactly one level of an
    /// object is drawn and it does not flip between levels with the view's jitter.
    pub object_sizes: std::cell::RefCell<hashbrown::HashMap<[u64; 4], f32>>,
    /// Cleared and reused as the next main view's object-size history.
    pub object_sizes_scratch: std::cell::RefCell<hashbrown::HashMap<[u64; 4], f32>>,
}

impl CullHistory {
    /// No history yet.
    pub fn new() -> Self {
        Self {
            triple_culling: Default::default(),
            flicker: std::cell::RefCell::new(HashMap::new()),
            cull_drawn: std::cell::RefCell::new(Vec::new()),
            object_sizes: Default::default(),
            object_sizes_scratch: Default::default(),
        }
    }
}
