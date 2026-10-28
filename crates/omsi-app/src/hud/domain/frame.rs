//! Everything the interface draws in a frame, as the game hands it over: the [`Frame`] and
//! its parts (the chat, the open menu's kind, a list's timetable, a drop-down).

use super::{InfoBar, Notice};

/// What the chat widget needs from the session each frame.
pub struct ChatView<'a> {
    /// Every line, oldest first ("Name: text" or "* notice").
    pub lines: &'a [String],
    /// The line being typed (the input box is open).
    pub typing: Option<&'a str>,
    /// Why the last line was not sent.
    pub error: Option<&'a str>,
}

/// Which menu is open: its layout follows from it.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum MenuKind {
    /// The game menu itself.
    #[default]
    Game,
    /// A settings window (options, vehicle, world): a sidebar of pages and rows with switches,
    /// sliders and buttons. A row's label is `name\u{1f}kind\u{1f}value\u{1f}description\u{1f}fraction`
    /// (kind: `s` switch, `v` slider, `c` stepper, `o` opens a list, `a` button, `i` information).
    Options,
    /// The lines of the map's timetable.
    Lines,
    /// One line's tours.
    Tours,
    /// Any other list (drivers, fleet numbers, destinations, liveries ...).
    List,
}

/// The timetable beside a list of lines or tours: a title, a line of facts and rows of
/// (what, time).
pub struct Preview {
    pub title: String,
    pub meta: String,
    pub rows: Vec<(String, String)>,
    /// The row chosen, when the rows can be chosen (the stops to start a tour from).
    pub chosen: Option<usize>,
    /// The button under the rows, when there is one.
    pub button: Option<String>,
    /// The time of the timetable the rows are of, with buttons to put it ahead (the trip of
    /// a tour to take on later), when it can be set.
    pub time: Option<String>,
}

/// A drop-down open over a row of a settings window: the entries, the one the keyboard is
/// on, the first one shown and the one in force now.
pub struct DropdownView<'a> {
    pub row: usize,
    pub items: Vec<&'a str>,
    pub sel: usize,
    pub top: usize,
    pub current: Option<usize>,
}

/// Everything the interface draws in a frame.
pub struct Frame<'a> {
    /// Physical pixels per logical one.
    pub scale: f32,
    /// How much larger than designed (`size_factor`): everything below is drawn this much
    /// larger on top of `scale`.
    pub ui_scale: f32,
    /// How strongly the backgrounds are drawn (`backdrop`): the menu's and the timetable's
    /// panels, the chat's box, the plates under the notes, the tooltip and the frame rate.
    pub opacity: f32,
    pub width: f32,
    pub height: f32,
    pub cursor: (f32, f32),
    /// An OpenXR headset is drawing this frame.
    pub vr: bool,
    /// The name of what the cursor points at (a switch, a part), shown next to it.
    pub tooltip: Option<String>,
    /// The chat, when a LAN session runs and the chat is not switched off.
    pub chat: Option<ChatView<'a>>,
    /// The chat's own size on top of `ui_scale` (`Settings::chat_size`).
    pub chat_size: f32,
    /// What the driver has to act on (why the bus does not move, a passenger's wish, the
    /// change due, a service done), top left.
    pub notes: &'a [String],
    /// The frame rate and its 1 % low, top right (the `show_fps` setting).
    pub fps: Option<(f32, Option<f32>)>,
    /// The game stands paused.
    pub paused: bool,
    /// The game menu is open, with this line chosen (labels from `GAME_MENU`).
    pub menu: Option<(usize, &'a [(&'a str, &'a str)])>,
    /// The first line shown when a finger scrolled the menu (`App::menu_top`).
    pub menu_top: Option<f32>,
    /// The timetable window: its title and per stop (name, time, 0 served / 1 next / 2 ahead).
    pub timetable: Option<(String, Vec<(String, String, u8)>)>,
    /// The information bar: its items in groups (a hairline between the groups).
    pub info: Option<InfoBar>,
    /// The room the information bar has along the top: from x to x, from y down (on a
    /// touch screen the gap between its buttons); none: the window's width.
    pub info_room: Option<[f32; 3]>,
    /// A tutorial page: title, text, picture, page number and count.
    pub tutorial: Option<(&'a str, &'a str, Option<&'a std::path::Path>, usize, usize)>,
    /// Name tags: a screen position (the point above a bus), the name and a second line.
    pub tags: Vec<((f32, f32), String, String, f32)>,
    /// The server's notifications, oldest first.
    pub notices: &'a [Notice],
    /// Where the navigator is on the screen (the notifications stand over it, or under it
    /// in a top corner); none: they go to the top middle.
    pub notice_anchor: Option<[f32; 4]>,
    /// What kind of menu the lines belong to.
    pub menu_kind: MenuKind,
    /// The open list's title and the small line above it (the line a tour list is of).
    pub menu_head: Option<(String, String)>,
    /// The timetable of the chosen line or tour, beside the list.
    pub menu_preview: Option<Preview>,
    /// The first stop shown of the timetable beside the tours, when the wheel has scrolled it
    /// (`None`: the stop chosen is kept in view).
    pub pane_first: Option<usize>,
    /// The pages of an open settings window (their titles) and the one shown.
    pub menu_tabs: Option<(Vec<String>, usize)>,
    /// The keyboard chose the menu's line last: that line is shown lit (else only the one
    /// under the mouse is).
    pub menu_kbd: bool,
    /// The drop-down open over a row of the settings window.
    pub dropdown: Option<DropdownView<'a>>,
}
