//! The information bar: its items in groups, and how each item looks.

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
    /// A delay: red text.
    Late,
    /// Ahead of the timetable: green text.
    Early,
    /// A frame rate that keeps up (green dot), is going slow (amber) or struggles (red).
    Fps(u8),
}

#[derive(Clone, Debug, PartialEq)]
pub struct InfoItem {
    pub text: String,
    pub tone: InfoTone,
    /// A name from the map or the timetable, drawn as it is (not looked up as an interface
    /// text, in which "Depot" or "Station" would come out in another language).
    pub raw: bool,
}

impl InfoItem {
    pub(crate) fn new(text: impl Into<String>, tone: InfoTone) -> InfoItem {
        InfoItem {
            text: text.into(),
            tone,
            raw: false,
        }
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
}
