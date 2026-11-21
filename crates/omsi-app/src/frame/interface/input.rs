//! What the rest of the game reaches of the frame, by subject.

pub(crate) mod app_events {
    pub use crate::frame::implementation::app_events::*;
}

pub(crate) mod frame_stats {
    pub use crate::frame::domain::frame_stats::*;
}

pub(crate) use super::super::implementation::quit;
pub(crate) use super::super::implementation::session;
