//! The window input of the new interface: what the game hands the overlay of the mouse
//! over one of its windows (the "Add vehicle" window, the Alt bar's windows, the trip report).

/// A press or release of the left button, or a turn of the wheel, over the window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum WinInput {
    Press,
    Release,
    Wheel(f32),
    /// The window lost the mouse: a press it holds ends without a click.
    Lost,
}
