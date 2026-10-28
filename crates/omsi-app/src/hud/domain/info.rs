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

/// The information bar: items in groups, along the top or the bottom of the screen.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InfoBar {
    pub groups: Vec<Vec<InfoItem>>,
    pub bottom: bool,
}
