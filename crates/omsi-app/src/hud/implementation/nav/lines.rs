use crate::map_surface::{GROUP_BRT, GROUP_EXPRESS, GROUP_FEEDER};
use omsi_ui::Color;

/// The texts of the lines panel and of the "you are on" chip. A language without its own gets English.
pub struct LinesText {
    pub lines: &'static str,
    pub line_data: &'static str,
    pub map_lines: &'static str,
    pub all: &'static str,
    pub(super) brt: &'static str,
    pub express: &'static str,
    pub feeders: &'static str,
    pub length: &'static str,
    pub stops: &'static str,
    pub trip: &'static str,
    pub route: &'static str,
    pub vehicles: &'static str,
    pub circular: &'static str,
    pub out_back: &'static str,
    pub minutes: &'static str,
    pub count: &'static str,
    /// The limit of heavy vehicles, when it is not the same.
    pub heavy: &'static str,
    /// The compass: map, north, heading.
    pub orient: [&'static str; 3],
    /// The terrain view's button, and the height in a chip.
    pub terrain: &'static str,
    pub elev: &'static str,
    pub timetable: &'static str,
    /// Said while the timetable is being read.
    pub loading: &'static str,
    /// Tick every line of the tab on show, and clear them.
    /// Said in the card while no line is open: how to open one.
    pub hint: [&'static str; 2],
    /// The lists by kind of vehicle: bus, tram, metro, rail.
    pub modals: [&'static str; 4],
    pub tick_all: &'static str,
    pub tick_none: &'static str,
    pub towards: &'static str,
    /// Monday to Friday, Saturday, Sunday.
    pub days: [&'static str; 3],
    pub predicted: &'static str,
    /// The map's header: what it is, the next stop, the button that follows the bus.
    pub(super) city_map: &'static str,
    pub next_stop: &'static str,
    pub follow: &'static str,
    /// The card of where the bus is, and its rows.
    pub you_are_here: &'static str,
    pub city: &'static str,
    pub street: &'static str,
    /// The trip's card, and its rows (the next stop's above).
    pub(super) trip_card: &'static str,
    pub distance: &'static str,
    pub arrival: &'static str,
    pub stops_left: &'static str,
    /// The legend: the route ahead, the part driven, the traffic, the bus; and the keys.
    pub driven: &'static str,
    pub(super) traffic: &'static str,
    pub you: &'static str,
    pub keys: &'static str,
}

/// The texts as English keys: the painter looks them up in the catalog when it draws or measures them. A text
/// that is put together with others goes through `omsi_ui::tr` first (a sentence made of English pieces is not in
/// the catalog).
pub fn lines_text() -> LinesText {
    LinesText {
        lines: "Lines",
        line_data: "LINE DATA",
        map_lines: "MAP LINES",
        all: "All",
        brt: "BRT",
        express: "Express",
        feeders: "Feeders",
        length: "Length",
        stops: "Stops",
        trip: "Trip time",
        route: "Route",
        vehicles: "Vehicles now",
        circular: "Circular",
        out_back: "Out and back",
        minutes: "min",
        count: "lines",
        timetable: "Timetable",
        loading: "Loading timetable…",
        hint: ["Double-click a line", "for its routes and timetable"],
        modals: ["Bus", "Tram", "Metro", "Rail"],
        tick_all: "Select all",
        tick_none: "Clear",
        towards: "Towards",
        days: ["Mon–Fri", "Sat", "Sun"],
        predicted: "Times predicted from the map's timetable",
        heavy: "heavy",
        terrain: "Terrain",
        orient: ["Map", "North", "Heading"],
        elev: "elev.",
        city_map: "City map",
        next_stop: "Next stop",
        follow: "Follow bus",
        you_are_here: "You are here",
        city: "City",
        street: "Street",
        trip_card: "Trip",
        distance: "Distance",
        arrival: "Arrival",
        stops_left: "Stops left",
        driven: "Driven",
        traffic: "Traffic",
        you: "You",
        keys: "Wheel: zoom · Drag: pan · Esc: close",
    }
}

/// Whether the interface language writes decimals with a comma.
pub(super) fn decimal_comma() -> bool {
    decimal_comma_in(&omsi_ui::i18n::language())
}

pub fn decimal_comma_in(code: &str) -> bool {
    matches!(
        code,
        "be" | "ca"
            | "cs"
            | "de"
            | "es"
            | "fr"
            | "hu"
            | "id"
            | "it"
            | "kk"
            | "nl"
            | "pl"
            | "pt"
            | "pt-pt"
            | "ru"
            | "tr"
            | "uk"
            | "vi"
    )
}

/// The colour a bus line is drawn in on the city map (a fixed palette that keeps clear of the map's own colours).
pub(super) const GROUP_BRT_TAB: u8 = GROUP_BRT;
pub(super) const GROUP_EXPRESS_TAB: u8 = GROUP_EXPRESS;
pub(super) const GROUP_FEEDER_TAB: u8 = GROUP_FEEDER;

pub(super) fn line_color(i: usize) -> Color {
    const P: [u32; 10] = [
        0xe83f9b, 0x17b8c4, 0xff8a1f, 0xff4d4d, 0x8bd13f, 0xd46bff, 0x1fd1a0, 0xff6fb5, 0xf2c14e,
        0x4da3ff,
    ];
    Color::hex(P[i % P.len()])
}
