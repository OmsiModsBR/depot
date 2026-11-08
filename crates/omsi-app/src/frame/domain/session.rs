//! What a session of the game carries from frame to frame, apart from the world and the
//! player: the traffic, the timetable, the player's duty and the driver's career. The window
//! and the offscreen run both step it (see `frame/implementation/session.rs`).

use crate::gameplay::career::Career;
use crate::gameplay::humans::Humans;
use crate::gameplay::journey::Journey;
use crate::gameplay::schedule::{PlayerDuty, Schedule};
use crate::gameplay::traffic::Traffic;

#[derive(Default)]
pub(crate) struct Session {
    pub(crate) traffic: Option<Traffic>,
    pub(crate) schedule: Option<Schedule>,
    pub(crate) duty: Option<PlayerDuty>,
    /// The duty was told the places of the stops beyond the loaded tiles.
    pub(crate) duty_places: bool,
    /// The driver's personnel file and this session's statistics.
    pub(crate) career: Career,
    /// The people on foot and the passengers (see `gameplay::humans`).
    pub(crate) humans: Option<Humans>,
    /// The duty's stops with their times as driven, kept in a file (`journey`).
    pub(crate) journey: Option<Journey>,
}
