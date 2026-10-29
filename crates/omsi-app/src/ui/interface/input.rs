//! What the game hands the screens and calls on them: the new interface's side of the overlay
//! ([`NewUi`]) with the Esc menu; the loading screen's frame, its screenshots and the bus's card;
//! the end-of-trip report and the watch that opens it; the Alt bar and the state of its windows;
//! the "Add vehicle" window, its picture and its live view; the window input they share.

pub(crate) use super::super::domain::add::Picture;
pub use super::super::domain::add::{picture_key, AddFrame, AddWindow};
pub use super::super::domain::alt::{AltFrame, AltState};
pub(crate) use super::super::domain::loading::MAX_SLIDES;
pub use super::super::domain::loading::{bus_card, recent, CardText, Decoded, LoadingFrame};
pub use super::super::domain::overlay::NewUi;
pub use super::super::domain::pages::{page_frame, showing, PageFrame, Showing};
pub use super::super::domain::report::ReportFrame;
pub(crate) use super::super::domain::report::{Report, Watch};
pub(crate) use super::super::domain::win_input::WinInput;
pub use super::super::domain::windows::{ToolFrame, Tools};
pub(crate) use super::super::implementation::add::{read_picture, LiveView};
pub use super::super::implementation::loading::decode;
pub(crate) use super::super::implementation::loading::map_picture;
