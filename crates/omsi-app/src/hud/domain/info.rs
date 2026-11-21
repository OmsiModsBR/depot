//! The information bar: what it reads from the game, the layout the player chose, its items
//! in groups, and how each item looks.

/// How an item of the information bar looks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InfoTone {
    /// White text.
    Plain,
    /// Grey text: a unit, a label.
    Dim,
    /// The line number: dark text on the accent.
    Badge,
    /// The speed limit: dark text on a white sign with a red frame.
    Limit,
    /// A warning value (a tank nearly empty): red text.
    Late,
    /// A frame rate that keeps up (green dot), is going slow (amber) or struggles (red).
    Fps(u8),
    /// The speed: white text as wide as three digits, so the bar does not move as it changes.
    Speed,
    /// The delay to the timetable as a chip: -1 ahead (green), 0 on time (the accent), 1 late
    /// (red); always as wide as "+00:00".
    Delay(i8),
    /// The pedals with no text: three bars side by side, filled from the bottom by how far
    /// each is pressed, in percent, in the order the pedals stand: the clutch (blue), the
    /// brake (red) and the throttle (green).
    Pedals([u8; 3]),
}

#[derive(Clone, Debug, PartialEq)]
pub struct InfoItem {
    pub text: String,
    pub tone: InfoTone,
    /// A name from the map or the timetable, drawn as it is (not looked up as an interface
    /// text, in which "Depot" or "Station" would come out in another language).
    pub raw: bool,
    /// The icon drawn before it (`hud-time`, `hud-stop` ...; see `omsi_ui::icons`).
    pub icon: Option<&'static str>,
    /// What it is, under the mouse after a second (an interface text, looked up), or the whole
    /// of a name cut short (`clip`).
    pub tip: Option<String>,
    /// A long name (the route) is cut short with "…" so that the next stop keeps its room;
    /// its whole is then the tip (drawn as it is when the item is `raw`).
    pub clip: bool,
}

impl InfoItem {
    pub(crate) fn new(text: impl Into<String>, tone: InfoTone) -> InfoItem {
        InfoItem {
            text: text.into(),
            tone,
            raw: false,
            icon: None,
            tip: None,
            clip: false,
        }
    }

    /// With the icon `icon` before it.
    pub(crate) fn with_icon(mut self, icon: &'static str) -> InfoItem {
        self.icon = Some(icon);
        self
    }

    /// Named `tip` under the mouse.
    pub(crate) fn with_tip(mut self, tip: impl Into<String>) -> InfoItem {
        self.tip = Some(tip.into());
        self
    }

    pub(crate) fn raw(text: impl Into<String>, tone: InfoTone) -> InfoItem {
        InfoItem {
            raw: true,
            ..InfoItem::new(text, tone)
        }
    }
}

/// One readout of the second tier: its items (a name above it in grey, the value), a
/// meter under the value when it is a level (the tank), and the mark of a warning.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InfoCell {
    pub items: Vec<InfoItem>,
    /// How full, 0 to 1.
    pub meter: Option<f32>,
    /// The cell is lit and its meter is red (a tank under 15 %).
    pub alert: bool,
    /// The icon left of it (`hud-fuel` ...).
    pub icon: Option<&'static str>,
    /// Its name under the mouse after a second.
    pub tip: Option<String>,
}

/// The information bar: items in groups, along the top or the bottom of the screen.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InfoBar {
    pub groups: Vec<Vec<InfoItem>>,
    pub bottom: bool,
    /// The two-tier layouts: the readouts of the second tier (none while it is folded away).
    pub tier2: Vec<InfoCell>,
    /// The second tier is its own plate under the first (not a row of the same plate).
    pub split: bool,
    /// A two-tier layout: the arrow that folds the second tier away is drawn.
    pub tiered: bool,
    /// The second tier is folded away (the arrow points down).
    pub collapsed: bool,
    /// The Essentials layout: the speed and its limit are drawn larger.
    pub large: bool,
}

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
    pub player: Option<&'a crate::gameplay::player::Player>,
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
    pub items: [bool; crate::settings::INFO_ITEM_KEYS.len()],
    /// The two-tier layouts: names above the readouts of the second tier.
    pub labels: bool,
    /// The two-tier layouts: a tank under 15 % is marked.
    pub low_fuel: bool,
    /// The two-tier layouts: the second tier is folded away.
    pub collapsed: bool,
}
