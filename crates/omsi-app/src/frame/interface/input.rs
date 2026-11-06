//! What the rest of the game reaches of the frame, by subject.

pub(crate) mod app_events {
    // the info page's types are only named from outside by the tests (`testing.rs`)
    #[cfg(feature = "test-support")]
    pub use crate::frame::domain::app_events::*;
    pub use crate::frame::implementation::app_events::*;
}

pub(crate) mod frame_stats {
    pub use crate::frame::domain::frame_stats::*;
}

pub(crate) use super::super::implementation::quit;
