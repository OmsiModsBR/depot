//! What the game hands the screens and calls on them: the loading screen's frame, its
//! screenshots and the bus's card; the end-of-trip report and the watch that opens it; the
//! Alt bar and the state of its windows.

pub use super::super::domain::alt::{AltFrame, AltState};
pub(crate) use super::super::domain::loading::MAX_SLIDES;
pub use super::super::domain::loading::{bus_card, recent, CardText, Decoded, LoadingFrame};
pub use super::super::domain::report::ReportFrame;
pub(crate) use super::super::domain::report::{Report, Watch};
pub use super::super::domain::windows::{ToolFrame, Tools};
pub use super::super::implementation::loading::decode;
pub(crate) use super::super::implementation::loading::map_picture;
