//! The types of the frame: the window's events and what the frame keeps (the info page's
//! mode and data, the frame rate's "1 %" low). Their procedures are in `implementation/`.

use crate::gameplay::player::Player;

/// What the information bar says, in the layout the player chose (`info_bar_mode`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InfoMode {
    /// Every item.
    Full,
    /// The time, the speed and its limit, the line, the next stop and the delay.
    Compact,
    /// The speed, its limit and the delay.
    Essential,
    /// The full bar with the items the player switched off left out.
    Custom,
    /// The driving items in one row, small readouts under them that fold away.
    TwoTier,
    /// The driving items on a plate of their own, the readouts on a strip under it.
    Split,
}

/// What `info_bar` reads from the game.
pub struct InfoData<'a> {
    pub clock: &'a omsi_sim::SimClock,
    pub player: Option<&'a Player>,
    pub duty: Option<&'a crate::gameplay::schedule::PlayerDuty>,
    /// How many are aboard (none: the passengers are switched off for this drive).
    pub passengers: Option<usize>,
    /// How many the bus has places for.
    pub capacity: Option<usize>,
    /// The kilometres driven this session (`Career::metres`).
    pub metres: f64,
    /// The speed limit of the road the bus is on, in km/h.
    pub limit: Option<f32>,
    /// The frame rate and its 1 % low.
    pub fps: Option<(f32, Option<f32>)>,
    /// Which items the custom layout shows (`Settings::info_items`).
    pub items: [bool; 9],
    /// The two-tier layouts: names above the readouts of the second tier.
    pub labels: bool,
    /// The two-tier layouts: a tank under 15 % is marked.
    pub low_fuel: bool,
    /// The two-tier layouts: the second tier is folded away.
    pub collapsed: bool,
}
