//! The crate's internals, open to the tests under `tests/` (feature `test-support`).

pub mod domain {
    pub use crate::domain::{caps, glass_rain, render_scale, scene, ui, wet_road};
}

pub mod implementation {
    pub use crate::implementation::null;
}

#[cfg(windows)]
pub mod d3d9 {
    pub use crate::implementation::d3d9::*;
}
